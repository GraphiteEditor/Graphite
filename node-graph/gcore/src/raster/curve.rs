use dyn_any::{DynAny, StaticType};

use crate::Node;

use super::{Channel, LuminanceMut};

#[derive(Debug, Clone, PartialEq, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Curve {
	pub samples: Vec<CurveSample>,
	pub start_params: [f32; 2],
	pub end_params: [f32; 2],
}

impl Default for Curve {
	fn default() -> Self {
		Self {
			samples: vec![],
			start_params: [0.2; 2],
			end_params: [0.8; 2],
		}
	}
}

impl std::hash::Hash for Curve {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.samples.hash(state);
		[self.start_params, self.end_params].iter().flatten().for_each(|f| f.to_bits().hash(state));
	}
}

#[derive(Debug, Clone, Copy, PartialEq, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CurveSample {
	pub pos: [f32; 2],
	pub params: [[f32; 2]; 2],
}

impl std::hash::Hash for CurveSample {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		for c in self.params.iter().chain([&self.pos]).flatten() {
			c.to_bits().hash(state);
		}
	}
}

// TODO: Propably this is more or less a reimplementation of `CubicSplines`. This code doesn't fail
//       at any asserts. Maybe it is also faster, but that should be tested, as well as the numerical
//       stability. Also bezier_rs functions like `intersections` could be used, but they aren't
//       really made for this case either.
/// This struct stores a bezier curve with auxilary data to be used with the `solve` function.
pub struct CubicBezierCurve {
	qh: f32,
	p: f32,
	h: f32,
	a: f32,
	y: [f32; 4],
}

const TMP1: f32 = 2.598076211353316;
const TMP2: f32 = 1.1547005383792515;
const PI23: f32 = core::f32::consts::PI * 2. / 3.;

impl CubicBezierCurve {
	pub fn new([x0, y0, x1, y1, x2, y2, x3, y3]: [f32; 8]) -> Self {
		let [x03, x13, x23] = [x0 * 3., x1 * 3., x2 * 3.];
		let [a, b, c] = [x13 - x23 + x3 - x0, x03 - 2. * x13 + x23, x13 - x03];
		let [a2, b2] = [a * a, b * b];
		let p = (3. * a * c - b2) / (3. * a2);
		let qh = (2. / 27. * b2 * b - 1. / 3. * a * b * c + a2 * x0) / (a2 * a);
		Self {
			p,
			qh,
			h: 1. / 3. * -b / a,
			a,
			y: [y0, y1, y2, y3],
		}
	}

	/// Get the y-coordinate of the curve given a x-coordinate.
	pub fn solve(&self, x: f32) -> f32 {
		let q = self.qh - x / self.a;
		let t = (if self.p.abs() < -f32::EPSILON {
			0.
		} else if self.p > 0. {
			let psqrt = self.p.sqrt();
			let asinh = (TMP1 * q / (self.p * psqrt)).asinh() / 3.;
			asinh.sinh() * -TMP2 * psqrt
		} else {
			let psqrt = (-self.p).sqrt();
			let acos = (TMP1 * q / (self.p * psqrt)).acos() / 3.;
			(acos - PI23).cos() * TMP2 * psqrt
		}) + self.h;
		let t1 = 1. - t;
		t1 * t1 * t1 * self.y[0] + 3. * t1 * t1 * t * self.y[1] + 3. * t1 * t * t * self.y[2] + t * t * t * self.y[3]
	}
}

#[derive(Debug)]
pub struct CubicSplines {
	pub x: [f32; 4],
	pub y: [f32; 4],
}

impl CubicSplines {
	pub fn solve(&self) -> [f32; 4] {
		let (x, y) = (&self.x, &self.y);

		// Build an augmented matrix to solve the system of equations using Gaussian elimination
		let mut augmented_matrix = [
			[
				2. / (x[1] - x[0]),
				1. / (x[1] - x[0]),
				0.,
				0.,
				// |
				3. * (y[1] - y[0]) / ((x[1] - x[0]) * (x[1] - x[0])),
			],
			[
				1. / (x[1] - x[0]),
				2. * (1. / (x[1] - x[0]) + 1. / (x[2] - x[1])),
				1. / (x[2] - x[1]),
				0.,
				// |
				3. * ((y[1] - y[0]) / ((x[1] - x[0]) * (x[1] - x[0])) + (y[2] - y[1]) / ((x[2] - x[1]) * (x[2] - x[1]))),
			],
			[
				0.,
				1. / (x[2] - x[1]),
				2. * (1. / (x[2] - x[1]) + 1. / (x[3] - x[2])),
				1. / (x[3] - x[2]),
				// |
				3. * ((y[2] - y[1]) / ((x[2] - x[1]) * (x[2] - x[1])) + (y[3] - y[2]) / ((x[3] - x[2]) * (x[3] - x[2]))),
			],
			[
				0.,
				0.,
				1. / (x[3] - x[2]),
				2. / (x[3] - x[2]),
				// |
				3. * (y[3] - y[2]) / ((x[3] - x[2]) * (x[3] - x[2])),
			],
		];

		// Gaussian elimination: forward elimination
		for row in 0..4 {
			let pivot_row_index = (row..4)
				.max_by(|&a_row, &b_row| {
					augmented_matrix[a_row][row]
						.abs()
						.partial_cmp(&augmented_matrix[b_row][row].abs())
						.unwrap_or(core::cmp::Ordering::Equal)
				})
				.unwrap();

			// Swap the current row with the row that has the largest pivot element
			augmented_matrix.swap(row, pivot_row_index);

			// Eliminate the current column in all rows below the current one
			for row_below_current in row + 1..4 {
				assert!(augmented_matrix[row][row].abs() > core::f32::EPSILON);

				let scale_factor = augmented_matrix[row_below_current][row] / augmented_matrix[row][row];
				for col in row..5 {
					augmented_matrix[row_below_current][col] -= augmented_matrix[row][col] * scale_factor
				}
			}
		}

		// Gaussian elimination: back substitution
		let mut solutions = [0.; 4];
		for col in (0..4).rev() {
			assert!(augmented_matrix[col][col].abs() > core::f32::EPSILON);

			solutions[col] = augmented_matrix[col][4] / augmented_matrix[col][col];

			for row in (0..col).rev() {
				augmented_matrix[row][4] -= augmented_matrix[row][col] * solutions[col];
				augmented_matrix[row][col] = 0.;
			}
		}

		solutions
	}

	pub fn interpolate(&self, input: f32, solutions: &[f32]) -> f32 {
		if input <= self.x[0] {
			return self.y[0];
		}
		if input >= self.x[self.x.len() - 1] {
			return self.y[self.x.len() - 1];
		}

		// Find the segment that the input falls between
		let mut segment = 1;
		while self.x[segment] < input {
			segment += 1;
		}
		let segment_start = segment - 1;
		let segment_end = segment;

		// Calculate the output value using quadratic interpolation
		let input_value = self.x[segment_start];
		let input_value_prev = self.x[segment_end];
		let output_value = self.y[segment_start];
		let output_value_prev = self.y[segment_end];
		let solutions_value = solutions[segment_start];
		let solutions_value_prev = solutions[segment_end];

		let output_delta = solutions_value_prev * (input_value - input_value_prev) - (output_value - output_value_prev);
		let solution_delta = (output_value - output_value_prev) - solutions_value * (input_value - input_value_prev);

		let input_ratio = (input - input_value_prev) / (input_value - input_value_prev);
		let prev_output_ratio = (1. - input_ratio) * output_value_prev;
		let output_ratio = input_ratio * output_value;
		let quadratic_ratio = input_ratio * (1. - input_ratio) * (output_delta * (1. - input_ratio) + solution_delta * input_ratio);

		let result = prev_output_ratio + output_ratio + quadratic_ratio;
		result.clamp(0., 1.)
	}
}

pub struct ValueMapperNode<C> {
	lut: Vec<C>,
}

impl<C> ValueMapperNode<C> {
	pub const fn new(lut: Vec<C>) -> Self {
		Self { lut }
	}
}

impl<'i, L: LuminanceMut + 'i> Node<'i, L> for ValueMapperNode<L::LuminanceChannel> {
	type Output = L;

	fn eval(&'i self, mut val: L) -> L {
		let floating_sample_index = val.luminance().to_f32() * (self.lut.len() - 1) as f32;
		let index_in_lut = floating_sample_index.floor() as usize;
		let a = self.lut[index_in_lut];
		let b = self.lut[(index_in_lut + 1).clamp(0, self.lut.len() - 1)];
		let result = a.lerp(b, floating_sample_index.fract());
		val.set_luminance(result);
		val
	}
}
