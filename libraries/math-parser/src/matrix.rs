use crate::constants::mean_of;
use crate::quaternion::Quaternion;
use crate::value::{Number, part_product, power_of_two_scale};
use std::fmt;
use std::ops::{Add, Neg, Sub};

/// An affine map of quaternion space, `p -> M p + c`: four rows over the parts `w, x, y, z` and a translation. Matrices are the
/// language's second sort beside values, holding every map from a swizzle to a Graphite transform, a Linear one with zero translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
	/// Each row weights the input's parts into one output part, so the row `i` in the `x` slot passes the input's `x` through.
	pub rows: [Quaternion; 4],
	pub translation: Quaternion,
	/// The parameter parts the map spans as a region, which `inside` and `clamp` bound by `0..1` while holding the rest at 0: a range's
	/// rung, a literal's count, a composition's inner map's, or every part a builtin touches.
	pub axes: [bool; 4],
}

const ZERO: Quaternion = Quaternion::ZERO;
const ALL_AXES: [bool; 4] = [true; 4];

impl Matrix {
	pub const IDENTITY: Self = Self::linear([Quaternion::ONE, Quaternion::I, Quaternion::J, Quaternion::K]);
	pub const ZERO: Self = Self::linear([ZERO; 4]);

	pub const fn linear(rows: [Quaternion; 4]) -> Self {
		Self {
			rows,
			translation: ZERO,
			axes: ALL_AXES,
		}
	}

	/// The row literal `[a;b;c]`, whose rows land on the parts of the rung their count names: `w` alone, `x, y`, `x, y, z`, or all four.
	pub fn from_rows(rows: &[Quaternion]) -> Option<Self> {
		let (rows, axes) = match *rows {
			[a] => ([a, ZERO, ZERO, ZERO], [true, false, false, false]),
			[a, b] => ([ZERO, a, b, ZERO], [false, true, true, false]),
			[a, b, c] => ([ZERO, a, b, c], [false, true, true, true]),
			[a, b, c, d] => ([a, b, c, d], ALL_AXES),
			_ => return None,
		};
		Some(Self { axes, ..Self::linear(rows) })
	}

	/// The column literal `[a,b,c]`, whose columns are the images of the basis directions of the rung their count names.
	pub fn from_columns(columns: &[Quaternion]) -> Option<Self> {
		Some(Self::from_rows(columns)?.transposed())
	}

	/// The range `a..b`: on each part of the corners' join rung, the map sending parameter `0` to `a` and `1` to `b`, so vector corners
	/// make a box, with the other parts untouched. A part the corners share is flat, so `0..(i + k)` is the square where `j` is 0.
	pub fn range(a: Quaternion, b: Quaternion) -> Self {
		let axes = Self::range_axes(a, b);
		let (a, b) = (a.parts(), b.parts());

		let mut range = Self { axes, ..Self::IDENTITY };
		let mut translation = [0.; 4];
		for (axis, spanned) in axes.into_iter().enumerate() {
			if spanned {
				let mut row = [0.; 4];
				row[axis] = b[axis] - a[axis];
				range.rows[axis] = Quaternion::from_parts(row);
				translation[axis] = a[axis];
			}
		}
		range.translation = Quaternion::from_parts(translation);
		range
	}

	/// The axes a range spans, its corners' join rung: the weight if either corner has one, and the vector parts up to the last either
	/// has, with `0..0` a range of the weight.
	fn range_axes(a: Quaternion, b: Quaternion) -> [bool; 4] {
		let (a, b) = (a.parts(), b.parts());
		let has_part = |axis: usize| a[axis] != 0. || b[axis] != 0.;
		let last_vector_axis = (1..4).rev().find(|&axis| has_part(axis));
		std::array::from_fn(|axis| match axis {
			0 => has_part(0) || last_vector_axis.is_none(),
			_ => last_vector_axis.is_some_and(|last| axis <= last),
		})
	}

	/// The image of one parameter axis, a column of the linear part.
	fn column(self, axis: usize) -> [f64; 4] {
		self.rows.map(|row| row.parts()[axis])
	}

	fn with_column(mut self, axis: usize, column: [f64; 4]) -> Self {
		for (row, entry) in self.rows.iter_mut().zip(column) {
			let mut parts = row.parts();
			parts[axis] = entry;
			*row = Quaternion::from_parts(parts);
		}
		self
	}

	/// The map as a region, where each parameter axis it leaves unspanned and unused passes its own part through, so a padded literal
	/// like `[u, v]` leaves the parts off its parallelogram alone.
	pub fn region(self) -> Self {
		(0..4)
			.filter(|&axis| !self.axes[axis] && self.column(axis) == [0.; 4])
			.fold(self, |region, axis| region.with_column(axis, unit(axis)))
	}

	/// The region with every parameter axis it leaves unused given a direction off the others, its own where it can, so it inverts
	/// unless singular for another reason, and which spanned axes were flat. A point lies on the region only at a parameter of 0 there.
	pub fn invertible_region(self) -> (Self, [bool; 4]) {
		let used = |axis: usize| self.column(axis) != [0.; 4];
		let flat = std::array::from_fn(|axis| self.axes[axis] && !used(axis));

		// An orthonormal basis of the region's extent, which each unused axis extends with the basis direction furthest from it
		let mut extent = (0..4).filter(|&axis| used(axis)).fold(Vec::new(), |mut extent, axis| {
			extent.extend(orthogonal_part(self.column(axis), &extent).map(|(direction, _)| direction));
			extent
		});
		let mut region = self;
		for axis in (0..4).filter(|&axis| !used(axis)) {
			let candidates = std::iter::once(axis).chain((0..4).filter(|&index| index != axis));
			let furthest = candidates
				.filter_map(|index| orthogonal_part(unit(index), &extent))
				.reduce(|furthest, candidate| if candidate.1 > furthest.1 { candidate } else { furthest });
			if let Some((direction, _)) = furthest {
				region = region.with_column(axis, direction);
				extent.push(direction);
			}
		}

		(region, flat)
	}

	/// Whether the translation is zero, leaving a linear map.
	pub fn is_linear(self) -> bool {
		self.translation == ZERO
	}

	/// Whether the two maps agree entry for entry, whatever axes each was built to act on.
	pub fn same_entries(self, other: Self) -> bool {
		self.rows == other.rows && self.translation == other.translation
	}

	/// The linear part with its rows and columns swapped, keeping the translation.
	pub fn transposed(self) -> Self {
		let entries = self.entries();
		Self {
			rows: std::array::from_fn(|row| Quaternion::from_parts(std::array::from_fn(|column| entries[column][row]))),
			..self
		}
	}

	/// The map with the translation `t` added.
	pub fn translated(self, t: Quaternion) -> Self {
		Self {
			translation: self.translation + t,
			..self
		}
	}

	/// The parts a value has, as the axes a map built from it acts on.
	fn axes_of(q: Quaternion) -> [bool; 4] {
		q.parts().map(|part| part != 0.)
	}

	fn entries(self) -> [[f64; 4]; 4] {
		self.rows.map(Quaternion::parts)
	}

	/// The image of `p`, `M p + c`.
	pub fn apply(self, p: Quaternion) -> Quaternion {
		Quaternion::from_parts(self.rows.map(|row| inner(row, p))) + self.translation
	}

	/// The composition `self ∘ other`, applying `other` first: `(M₁ M₂, M₁ c₂ + c₁)`. Its parameters are those of `other`, so it spans
	/// `other`'s axes, as a rotated box spans the box's.
	pub fn compose(self, other: Self) -> Self {
		let rows = self.rows.map(|row| {
			let weights = row.parts();
			let mut combined = ZERO;
			for (weight, other_row) in weights.into_iter().zip(other.rows) {
				combined = combined + other_row.map(|entry| part_product(weight, entry, false));
			}
			combined
		});
		let translation = Quaternion::from_parts(self.rows.map(|row| inner(row, other.translation))) + self.translation;
		Self { rows, translation, axes: other.axes }
	}

	/// The determinant of the linear part.
	pub fn determinant(self) -> f64 {
		let entries = self.entries();
		(0..4).map(|column| cofactor(entries, 0, column) * entries[0][column]).sum()
	}

	/// The inverse map, or `None` for a singular one: `(M⁻¹, -M⁻¹ c)`.
	pub fn inverse(self) -> Option<Self> {
		// Rows then columns are divided by powers of two near their largest entries, which is exact, so the cofactors of huge or tiny
		// entries stay in range, then the scales are divided back out
		let mut entries = self.entries();
		let row_scales = entries.map(|row| power_of_two_scale(row.into_iter()));
		for (row, scale) in entries.iter_mut().zip(row_scales) {
			*row = row.map(|entry| entry / scale);
		}
		let column_scales: [f64; 4] = std::array::from_fn(|column| power_of_two_scale(entries.iter().map(|row| row[column])));
		for row in entries.iter_mut() {
			*row = std::array::from_fn(|column| row[column] / column_scales[column]);
		}

		let determinant: f64 = (0..4).map(|column| cofactor(entries, 0, column) * entries[0][column]).sum();
		if determinant == 0. {
			return None;
		}

		// The adjugate over the determinant, whose entries are the transposed cofactors
		let rows = std::array::from_fn(|row| Quaternion::from_parts(std::array::from_fn(|column| cofactor(entries, column, row) / determinant / column_scales[row] / row_scales[column])));
		let linear = Self::linear(rows);
		Some(Self {
			rows,
			translation: -Quaternion::from_parts(linear.rows.map(|row| inner(row, self.translation))),
			..self
		})
	}

	/// The composition power `A^n`, with a negative power composing the inverse, or `None` for a negative power of a singular map.
	pub fn power(self, exponent: i64) -> Option<Self> {
		let mut base = if exponent < 0 { self.inverse()? } else { self };
		let mut remaining = exponent.unsigned_abs();
		let mut result = Self { axes: self.axes, ..Self::IDENTITY };
		while remaining > 0 {
			if remaining & 1 == 1 {
				result = result.compose(base);
			}
			remaining >>= 1;
			if remaining > 0 {
				base = base.compose(base);
			}
		}
		Some(result)
	}

	/// Left multiplication by `q` as a matrix, `L_q p = q p`, the value's own action in the matrix sort.
	pub fn left_multiplication(q: Quaternion) -> Self {
		let Quaternion { w, x, y, z } = q;
		Self::linear([Quaternion::new(w, -x, -y, -z), Quaternion::new(x, w, -z, y), Quaternion::new(y, z, w, -x), Quaternion::new(z, -y, x, w)])
	}

	/// The rotation a unit rotor performs about `axis`, leaving the weight alone, with the axes it does not turn kept exact.
	pub fn rotation(rotor: Quaternion, axis: Quaternion) -> Self {
		let Quaternion { w, x, y, z } = rotor;
		let [_, along_x, along_y, along_z] = Self::axes_of(axis);
		Self {
			// The plane of rotation, all of space unless the axis is a basis direction, whose own part stays put
			axes: [false, along_y || along_z, along_x || along_z, along_x || along_y],
			..Self::linear([
				Quaternion::ONE,
				Quaternion::new(0., 1. - 2. * (y * y + z * z), 2. * (x * y - w * z), 2. * (x * z + w * y)),
				Quaternion::new(0., 2. * (x * y + w * z), 1. - 2. * (x * x + z * z), 2. * (y * z - w * x)),
				Quaternion::new(0., 2. * (x * z - w * y), 2. * (y * z + w * x), 1. - 2. * (x * x + y * y)),
			])
		}
	}

	/// Scales each axis by the weight plus that axis's part of `q`, so a real scales uniformly, leaving the weight alone.
	pub fn scale(q: Quaternion) -> Self {
		Self {
			axes: [false, true, true, true],
			..Self::linear([
				Quaternion::ONE,
				Quaternion::new(0., q.w + q.x, 0., 0.),
				Quaternion::new(0., 0., q.w + q.y, 0.),
				Quaternion::new(0., 0., 0., q.w + q.z),
			])
		}
	}

	/// Displaces the `along` coordinate by `factor` times the `by` coordinate: the identity plus `factor` times their outer product.
	pub fn shear(along: Quaternion, by: Quaternion, factor: f64) -> Self {
		let mut matrix = Self {
			axes: joined(Self::axes_of(along), Self::axes_of(by)),
			..Self::IDENTITY
		};
		for (row, along) in matrix.rows.iter_mut().zip(along.parts()) {
			*row = *row + by.map(|by| part_product(part_product(factor, along, false), by, false));
		}
		matrix
	}

	/// The pointwise mean, each entry averaged on its own scale as the mean of values is, so huge entries cannot overflow the sum.
	pub fn mean(matrices: &[Self]) -> Option<Self> {
		let count = matrices.len();
		if count == 0 {
			return None;
		}

		Some(Self {
			rows: std::array::from_fn(|row| Quaternion::from_parts(mean_of(matrices.iter().map(|matrix| matrix.rows[row].parts()), count))),
			translation: Quaternion::from_parts(mean_of(matrices.iter().map(|matrix| matrix.translation.parts()), count)),
			axes: matrices.iter().fold([false; 4], |axes, matrix| joined(axes, matrix.axes)),
		})
	}

	/// Applies a function to every entry, the translation included.
	pub fn map(self, function: impl Fn(f64) -> f64) -> Self {
		Self {
			rows: self.rows.map(|row| row.map(&function)),
			translation: self.translation.map(&function),
			..self
		}
	}

	/// Whether any entry is NaN, which no operation may produce.
	pub fn is_nan(self) -> bool {
		self.rows.iter().chain([&self.translation]).any(|row| row.parts().iter().any(|part| part.is_nan()))
	}

	/// Whether every entry is finite, as an inverse needs to be exact rather than dividing by infinity into zeros.
	pub fn is_finite(self) -> bool {
		self.rows.iter().chain([&self.translation]).all(|row| row.parts().iter().all(|part| part.is_finite()))
	}

	/// Whether the weight passes through untouched and never leaks into position, as in every Graphite transform.
	fn is_geometric(self) -> bool {
		self.rows[0] == Quaternion::ONE && self.rows[1..].iter().all(|row| row.w == 0.) && self.translation.w == 0.
	}

	/// Whether `z` passes through untouched, as in a map of the plane.
	fn is_planar(self) -> bool {
		self.rows[3] == Quaternion::K && self.rows[..3].iter().all(|row| row.z == 0.) && self.translation.z == 0.
	}

	/// Reads the matrix as a map of space, or `None` if it touches the weight.
	pub fn as_affine3(self) -> Option<Affine3> {
		let [_, x, y, z] = self.rows;
		self.is_geometric().then_some(Affine3 {
			linear: Linear3([[x.x, y.x, z.x], [x.y, y.y, z.y], [x.z, y.z, z.z]]),
			translation: [self.translation.x, self.translation.y, self.translation.z],
		})
	}

	/// Reads the matrix as a linear map of space, or `None` if it touches the weight or has a translation.
	pub fn as_linear3(self) -> Option<Linear3> {
		self.is_linear().then(|| self.as_affine3()).flatten().map(|affine| affine.linear)
	}

	/// Reads the matrix as a map of the plane, or `None` if it touches the weight or `z`.
	pub fn as_affine2(self) -> Option<Affine2> {
		let Affine3 {
			linear: Linear3([[a, c, _], [b, d, _], _]),
			translation: [x, y, _],
		} = self.is_planar().then(|| self.as_affine3()).flatten()?;
		Some(Affine2 {
			linear: Linear2([[a, c], [b, d]]),
			translation: [x, y],
		})
	}

	/// Reads the matrix as a linear map of the plane, or `None` if it touches the weight or `z` or has a translation.
	pub fn as_linear2(self) -> Option<Linear2> {
		self.is_linear().then(|| self.as_affine2()).flatten().map(|affine| affine.linear)
	}
}

/// The inner product of a row with a value over all four parts, where a zero entry contributes nothing even beside an infinite part.
fn inner(row: Quaternion, p: Quaternion) -> f64 {
	row.parts().into_iter().zip(p.parts()).map(|(weight, part)| part_product(weight, part, false)).sum()
}

/// The axes either map acts on, as a map built from both does.
fn joined(a: [bool; 4], b: [bool; 4]) -> [bool; 4] {
	std::array::from_fn(|axis| a[axis] || b[axis])
}

/// The basis direction of one part.
fn unit(axis: usize) -> [f64; 4] {
	std::array::from_fn(|part| if part == axis { 1. } else { 0. })
}

/// The part of `vector` orthogonal to the orthonormal `basis`, normalized, and its length relative to the vector's scale, or `None`
/// when nothing remains.
fn orthogonal_part(vector: [f64; 4], basis: &[[f64; 4]]) -> Option<([f64; 4], f64)> {
	let scale = power_of_two_scale(vector.into_iter());
	let mut remainder = vector.map(|part| part / scale);
	for direction in basis {
		let projection: f64 = remainder.iter().zip(direction).map(|(part, direction)| part * direction).sum();
		remainder = std::array::from_fn(|part| remainder[part] - projection * direction[part]);
	}

	let length = remainder.iter().map(|part| part * part).sum::<f64>().sqrt();
	(length > 0.).then(|| (remainder.map(|part| part / length), length))
}

/// The signed minor of the entry at `row`, `column`.
fn cofactor(entries: [[f64; 4]; 4], row: usize, column: usize) -> f64 {
	let mut minor = [[0.; 3]; 3];
	for (minor_row, source_row) in (0..4).filter(|&index| index != row).enumerate() {
		for (minor_column, source_column) in (0..4).filter(|&index| index != column).enumerate() {
			minor[minor_row][minor_column] = entries[source_row][source_column];
		}
	}
	let [[a, b, c], [d, e, f], [g, h, i]] = minor;
	let determinant = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
	if (row + column).is_multiple_of(2) { determinant } else { -determinant }
}

impl Add for Matrix {
	type Output = Self;
	fn add(self, other: Self) -> Self {
		Self {
			rows: std::array::from_fn(|index| self.rows[index] + other.rows[index]),
			translation: self.translation + other.translation,
			axes: joined(self.axes, other.axes),
		}
	}
}

impl Sub for Matrix {
	type Output = Self;
	fn sub(self, other: Self) -> Self {
		self + -other
	}
}

impl Neg for Matrix {
	type Output = Self;
	fn neg(self) -> Self {
		self.map(|entry| -entry)
	}
}

impl fmt::Display for Matrix {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// The shortest row literal naming the matrix, then its translation, parenthesized unless it is one positive part
		let [w, x, y, z] = self.rows;
		let rows: &[Quaternion] = if [x, y, z] == [ZERO; 3] {
			&[w]
		} else if w == ZERO && z == ZERO {
			&[x, y]
		} else if w == ZERO {
			&[x, y, z]
		} else {
			&self.rows
		};

		f.write_str("[")?;
		for (index, row) in rows.iter().enumerate() {
			if index > 0 {
				f.write_str(";")?;
			}
			Number::Quaternion(*row).canonical().fmt(f)?;
		}
		f.write_str("]")?;

		if self.translation != ZERO {
			let parts = self.translation.parts();
			let translation = Number::Quaternion(self.translation).canonical();
			if parts.iter().filter(|part| **part != 0.).count() == 1 && parts.iter().any(|part| *part > 0.) {
				write!(f, " + {translation}")
			} else {
				write!(f, " + ({translation})")
			}
		} else {
			Ok(())
		}
	}
}

/// A region for `inside`, `clamp`, and `remap`: a range literal kept by its corners, which may be infinite where no matrix can hold
/// them, or any other map.
#[derive(Debug, Clone, Copy)]
pub enum Region {
	Range(Quaternion, Quaternion),
	Map(Matrix),
}

impl Region {
	/// The region as a map, which for a range with an infinite corner holds infinite or indeterminate entries.
	pub fn matrix(self) -> Matrix {
		match self {
			Region::Range(a, b) => Matrix::range(a, b),
			Region::Map(matrix) => matrix,
		}
	}
}

/// A linear map of the plane as its columns, the images of `i` and `j`, like glam's `from_cols`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Linear2(pub [[f64; 2]; 2]);

/// A map of the plane: a linear part and a translation, like glam's `DAffine2`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Affine2 {
	pub linear: Linear2,
	pub translation: [f64; 2],
}

/// A linear map of space as its columns, the images of `i`, `j`, and `k`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Linear3(pub [[f64; 3]; 3]);

/// A map of space: a linear part and a translation, like glam's `DAffine3`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Affine3 {
	pub linear: Linear3,
	pub translation: [f64; 3],
}

impl From<Affine3> for Matrix {
	fn from(
		Affine3 {
			linear: Linear3([i, j, k]),
			translation: [x, y, z],
		}: Affine3,
	) -> Self {
		Self {
			rows: [
				Quaternion::ONE,
				Quaternion::new(0., i[0], j[0], k[0]),
				Quaternion::new(0., i[1], j[1], k[1]),
				Quaternion::new(0., i[2], j[2], k[2]),
			],
			translation: Quaternion::new(0., x, y, z),
			axes: [false, true, true, true],
		}
	}
}

impl From<Linear3> for Matrix {
	fn from(linear: Linear3) -> Self {
		Self::from(Affine3 { linear, translation: [0.; 3] })
	}
}

impl From<Affine2> for Matrix {
	fn from(
		Affine2 {
			linear: Linear2([i, j]),
			translation: [x, y],
		}: Affine2,
	) -> Self {
		Self {
			axes: [false, true, true, false],
			..Self::from(Affine3 {
				linear: Linear3([[i[0], i[1], 0.], [j[0], j[1], 0.], [0., 0., 1.]]),
				translation: [x, y, 0.],
			})
		}
	}
}

impl From<Linear2> for Matrix {
	fn from(linear: Linear2) -> Self {
		Self::from(Affine2 { linear, translation: [0.; 2] })
	}
}
