use core_types::Color;
use core_types::color::SRGBA8;
use core_types::render_complexity::RenderComplexity;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2, Vec4};
use kurbo::{ParamCurve, PathSeg};

use crate::{
	Vector,
	vector::{PointId, SegmentId, StrokeId, misc::point_to_dvec2},
};

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum GradientType {
	#[default]
	Linear,
	Radial,
	Mesh,
}

// TODO: Someday we could switch this to a Box[T] to avoid over-allocation
/// A list of colors (linear, unassociated alpha) associated with positions (in the range 0 to 1) along a gradient.
///
/// Not exposed via Tsify; use [`GradientUI`] at the JS boundary.
#[derive(Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Gradient {
	/// The position of this stop, a factor from 0-1 along the length of the full gradient.
	pub position: Vec<f64>,
	/// The midpoint to the right of this stop, a factor from 0-1 along the distance to the next stop. The final stop's midpoint is ignored.
	pub midpoint: Vec<f64>,
	/// The color at this stop.
	pub color: Vec<Color>,
}

/// JS-boundary version of [`Gradient`] where stop colors are [`SRGBA8`] byte triples instead of linear-light [`Color`].
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(from_wasm_abi))]
#[derive(Debug, Clone, PartialEq, Default, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradientUI {
	pub position: Vec<f64>,
	pub midpoint: Vec<f64>,
	pub color: Vec<SRGBA8>,
}

impl From<&Gradient> for GradientUI {
	fn from(s: &Gradient) -> Self {
		Self {
			position: s.position.clone(),
			midpoint: s.midpoint.clone(),
			color: s.color.iter().map(|c| SRGBA8::from(*c)).collect(),
		}
	}
}

impl From<&GradientUI> for Gradient {
	fn from(s: &GradientUI) -> Self {
		Self {
			position: s.position.clone(),
			midpoint: s.midpoint.clone(),
			color: s.color.iter().map(|c| Color::from(*c)).collect(),
		}
	}
}

impl GradientUI {
	/// CSS `linear-gradient(...)` string. Stops are emitted as `#rrggbbaa` hex (already gamma-encoded bytes).
	pub fn to_css_linear_gradient(&self) -> String {
		if self.position.len() <= 1 {
			let hex = self.color.first().map(|c| c.to_rgba_hex()).unwrap_or_else(|| "000000ff".to_string());
			return format!("linear-gradient(to right, #{hex} 0%, #{hex} 100%)");
		}
		// Sample via the midpoint-aware subdivision used for SVG/Vello stops so browser interpolation matches
		let stops: Gradient = self.into();
		let pieces = stops
			.interpolated_samples()
			.into_iter()
			.map(|(position, color, _)| {
				let percent = ((position * 100.) * 1e2).round() / 1e2;
				let hex = SRGBA8::from(color).to_rgba_hex();
				format!("#{hex} {percent}%")
			})
			.collect::<Vec<_>>()
			.join(", ");
		format!("linear-gradient(to right, {pieces})")
	}
}

// TODO: Eventually remove this migration document upgrade code
impl<'de> serde::Deserialize<'de> for Gradient {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		#[derive(serde::Deserialize)]
		struct NewFormat {
			position: Vec<f64>,
			midpoint: Vec<f64>,
			color: Vec<Color>,
		}

		#[derive(serde::Deserialize)]
		#[cfg_attr(feature = "serde", serde(untagged))]
		enum GradientStopsFormat {
			New(NewFormat),
			Old(Vec<(f64, Color)>),
		}

		Ok(match GradientStopsFormat::deserialize(deserializer)? {
			GradientStopsFormat::New(new) => Self {
				position: new.position,
				midpoint: new.midpoint,
				color: new.color,
			},
			GradientStopsFormat::Old(stops) => {
				let count = stops.len();
				Self {
					position: stops.iter().map(|(p, _)| *p).collect(),
					midpoint: vec![0.5; count],
					color: stops.into_iter().map(|(_, c)| c).collect(),
				}
			}
		})
	}
}

impl Default for Gradient {
	fn default() -> Self {
		Self {
			position: vec![0., 1.],
			midpoint: vec![0.5, 0.5],
			color: vec![Color::BLACK, Color::WHITE],
		}
	}
}

impl RenderComplexity for Gradient {
	fn render_complexity(&self) -> usize {
		1
	}
}

/// Apply the midpoint curve to a normalized parameter `t` (0 to 1) given a `midpoint` (0 to 1, where 0.5 is linear).
fn apply_midpoint(t: f64, midpoint: f64) -> f64 {
	if (midpoint - 0.5).abs() < 1e-6 {
		return t;
	}

	let midpoint = midpoint.clamp(f64::EPSILON, 1. - f64::EPSILON);

	if midpoint < 0.5 {
		let q = -1. / (1. - midpoint).log2();
		1. - (1. - t).powf(q)
	} else {
		let p = -1. / midpoint.log2();
		t.powf(p)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
	pub position: f64,
	pub midpoint: f64,
	pub color: Color,
}

pub struct GradientStopsIter<'a> {
	stops: &'a Gradient,
	index: usize,
}

impl<'a> Iterator for GradientStopsIter<'a> {
	type Item = GradientStop;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index >= self.stops.position.len() {
			return None;
		}

		let stop = GradientStop {
			position: self.stops.position[self.index],
			midpoint: self.stops.midpoint[self.index],
			color: self.stops.color[self.index],
		};
		self.index += 1;
		Some(stop)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remaining = self.stops.position.len() - self.index;
		(remaining, Some(remaining))
	}
}

impl ExactSizeIterator for GradientStopsIter<'_> {}

impl<'a> IntoIterator for &'a Gradient {
	type Item = GradientStop;
	type IntoIter = GradientStopsIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		GradientStopsIter { stops: self, index: 0 }
	}
}

impl IntoIterator for Gradient {
	type Item = GradientStop;
	type IntoIter = std::vec::IntoIter<GradientStop>;

	fn into_iter(self) -> Self::IntoIter {
		self.position
			.into_iter()
			.zip(self.midpoint)
			.zip(self.color)
			.map(|((position, midpoint), color)| GradientStop { position, midpoint, color })
			.collect::<Vec<_>>()
			.into_iter()
	}
}

impl Gradient {
	pub fn new(stops: impl IntoIterator<Item = GradientStop>) -> Self {
		let mut position = Vec::new();
		let mut midpoint = Vec::new();
		let mut color = Vec::new();

		for stop in stops {
			position.push(stop.position);
			midpoint.push(stop.midpoint);
			color.push(stop.color);
		}

		Self { position, midpoint, color }
	}

	pub fn len(&self) -> usize {
		self.position.len()
	}

	pub fn is_empty(&self) -> bool {
		self.position.is_empty()
	}

	pub fn iter(&self) -> GradientStopsIter<'_> {
		self.into_iter()
	}

	/// Remove a stop at the given index.
	pub fn remove(&mut self, index: usize) {
		self.position.remove(index);
		self.midpoint.remove(index);
		self.color.remove(index);
	}

	/// Remove and return the last stop's color, or `None` if empty.
	pub fn pop(&mut self) -> Option<Color> {
		self.position.pop();
		self.midpoint.pop();
		self.color.pop()
	}

	/// Move the stop at `index` to a new position, re-sorting the stops by position. Returns the new index of the moved stop.
	pub fn move_stop(&mut self, index: usize, position: f64) -> usize {
		if index >= self.position.len() {
			return index;
		}
		self.position[index] = position;
		self.sort_returning_new_index(index)
	}

	/// Insert a new stop at the given position, sampling the gradient at that position to determine the new stop's color.
	/// The new stop's midpoint is inherited from the interval it splits (or `0.5` if inserting at the very start).
	/// Returns the index where the new stop was inserted.
	pub fn insert_stop(&mut self, position: f64) -> usize {
		let color = self.evaluate(position);
		let index = self.position.iter().position(|p| *p > position).unwrap_or(self.position.len());
		let midpoint = index.checked_sub(1).and_then(|i| self.midpoint.get(i).copied()).unwrap_or(0.5);
		self.position.insert(index, position);
		self.midpoint.insert(index, midpoint);
		self.color.insert(index, color);
		index
	}

	/// Insert a copy of the stop at `source_index` (same color and midpoint) at `position`, keeping the stops sorted by position.
	/// Returns the index where the copy was inserted, or `None` if `source_index` is out of range.
	pub fn duplicate_stop(&mut self, source_index: usize, position: f64) -> Option<usize> {
		let color = *self.color.get(source_index)?;
		let midpoint = *self.midpoint.get(source_index)?;
		let index = self.position.iter().position(|p| *p > position).unwrap_or(self.position.len());
		self.position.insert(index, position);
		self.midpoint.insert(index, midpoint);
		self.color.insert(index, color);
		Some(index)
	}

	/// Reset the midpoint for the interval starting at `index` to its default `0.5`.
	pub fn reset_midpoint(&mut self, index: usize) {
		if let Some(midpoint) = self.midpoint.get_mut(index) {
			*midpoint = 0.5;
		}
	}

	/// Sort the stops in place by position; returns the new index of the stop that was at `previous_index` before sorting.
	fn sort_returning_new_index(&mut self, previous_index: usize) -> usize {
		let len = self.position.len();
		let mut indices: Vec<usize> = (0..len).collect();
		indices.sort_by(|&a, &b| self.position[a].total_cmp(&self.position[b]));
		let new_index = indices.iter().position(|&i| i == previous_index).unwrap_or(previous_index);
		self.position = indices.iter().map(|&i| self.position[i]).collect();
		self.midpoint = indices.iter().map(|&i| self.midpoint[i]).collect();
		self.color = indices.iter().map(|&i| self.color[i]).collect();
		new_index
	}

	pub fn evaluate(&self, t: f64) -> Color {
		if self.position.is_empty() {
			return Color::BLACK;
		}

		if t <= self.position[0] {
			return self.color[0];
		}
		let last = self.position.len() - 1;
		if t >= self.position[last] {
			return self.color[last];
		}

		for i in 0..self.position.len() - 1 {
			let (t1, c1) = (self.position[i], self.color[i]);
			let (t2, c2) = (self.position[i + 1], self.color[i + 1]);
			if t >= t1 && t <= t2 {
				let normalized_t = (t - t1) / (t2 - t1);
				let adjusted_t = apply_midpoint(normalized_t, self.midpoint[i]);
				return c1.lerp(&c2, adjusted_t as f32);
			}
		}

		Color::BLACK
	}

	pub fn sort(&mut self) {
		let mut indices: Vec<usize> = (0..self.position.len()).collect();
		indices.sort_unstable_by(|&a, &b| self.position[a].total_cmp(&self.position[b]));
		self.position = indices.iter().map(|&i| self.position[i]).collect();
		self.midpoint = indices.iter().map(|&i| self.midpoint[i]).collect();
		self.color = indices.iter().map(|&i| self.color[i]).collect();
	}

	pub fn reversed(&self) -> Self {
		let position: Vec<f64> = self.position.iter().rev().map(|&p| 1. - p).collect();

		let count = self.midpoint.len();
		let midpoint = (0..count).map(|i| if i < count - 1 { 1. - self.midpoint[count - 2 - i] } else { 0.5 }).collect::<Vec<_>>();

		let color: Vec<Color> = self.color.iter().rev().cloned().collect();

		Self { position, midpoint, color }
	}

	pub fn map_colors<F: Fn(&Color) -> Color>(&self, f: F) -> Self {
		Self {
			position: self.position.clone(),
			midpoint: self.midpoint.clone(),
			color: self.color.iter().map(f).collect(),
		}
	}

	/// Build a CSS `linear-gradient(...)` string suitable for use as a `background-image`. Samples the midpoint curves so the rendered gradient matches Graphite's interpolation rather than browser defaults.
	pub fn to_css_linear_gradient(&self) -> String {
		if self.position.len() <= 1 {
			let hex = self.color.first().map(|c| SRGBA8::from(*c).to_rgba_hex()).unwrap_or_else(|| "000000ff".to_string());
			return format!("linear-gradient(to right, #{hex} 0%, #{hex} 100%)");
		}
		let pieces = self
			.interpolated_samples()
			.into_iter()
			.map(|(position, color, _)| {
				let percent = ((position * 100.) * 1e2).round() / 1e2;
				format!("#{} {percent}%", SRGBA8::from(color).to_rgba_hex())
			})
			.collect::<Vec<_>>()
			.join(", ");
		format!("linear-gradient(to right, {pieces})")
	}

	/// Produce a set of linearly-interpolated color samples that approximate the gradient's midpoint curves.
	///
	/// Each sample is `(position, color, original_midpoint)` where `original_midpoint` is `Some(f64)` with the corresponding
	/// midpoint for actual gradient stops, and `None` for interpolated samples added to approximate midpoint curves.
	///
	/// Interpolation is performed in sRGB gamma space (then lifted back to linear-light for output) because the downstream SVG/CSS
	/// renderer interpolates between adjacent `<stop>` colors in gamma space; doing the subdivision math in the same space ensures
	/// the chosen samples actually match the curve the browser will draw.
	pub fn interpolated_samples(&self) -> Vec<(f64, Color, Option<f64>)> {
		/// Controls accuracy vs. number of samples tradeoff.
		/// 2/255 means the linear approximation will deviate by no more than 2 gradations of 8-bit color from the theoretically perfect curve with this midpoint bias.
		const THRESHOLD: f64 = 2. / 255.;

		#[allow(clippy::too_many_arguments)]
		fn subdivide(left: f64, right: f64, midpoint: f64, pos_a: f64, pos_b: f64, color_a_gamma: [f32; 4], color_b_gamma: [f32; 4], result: &mut Vec<(f64, Color, Option<f64>)>, depth: u32) {
			const MAX_DEPTH: u32 = 20;
			if depth >= MAX_DEPTH {
				return;
			}

			let mid = (left + right) / 2.;

			let y_actual = apply_midpoint(mid, midpoint);
			let y_left = apply_midpoint(left, midpoint);
			let y_right = apply_midpoint(right, midpoint);
			let y_linear = (y_left + y_right) / 2.;

			if (y_actual - y_linear).abs() > THRESHOLD {
				subdivide(left, mid, midpoint, pos_a, pos_b, color_a_gamma, color_b_gamma, result, depth + 1);

				let global_pos = pos_a + mid * (pos_b - pos_a);
				let t = y_actual as f32;
				let r = color_a_gamma[0] + (color_b_gamma[0] - color_a_gamma[0]) * t;
				let g = color_a_gamma[1] + (color_b_gamma[1] - color_a_gamma[1]) * t;
				let b = color_a_gamma[2] + (color_b_gamma[2] - color_a_gamma[2]) * t;
				let a = color_a_gamma[3] + (color_b_gamma[3] - color_a_gamma[3]) * t;
				let color = Color::from_gamma_srgb_channels(r, g, b, a);
				result.push((global_pos, color, None));

				subdivide(mid, right, midpoint, pos_a, pos_b, color_a_gamma, color_b_gamma, result, depth + 1);
			}
		}

		if self.position.is_empty() {
			return vec![];
		}

		if self.position.len() == 1 {
			return vec![(self.position[0], self.color[0], Some(self.midpoint[0]))];
		}

		let mut result = Vec::new();

		for i in 0..self.position.len() - 1 {
			let pos_a = self.position[i];
			let pos_b = self.position[i + 1];
			let color_a = self.color[i];
			let color_b = self.color[i + 1];
			let midpoint = self.midpoint[i].clamp(0.01, 0.99);
			let next_midpoint = self.midpoint[i + 1].clamp(0.01, 0.99);

			// Add the start stop (subsequent segments share the previous end stop)
			if i == 0 {
				result.push((pos_a, color_a, Some(midpoint)));
			}

			// Only subdivide if midpoint deviates from linear (0.5)
			if (midpoint - 0.5).abs() >= 1e-6 {
				subdivide(0., 1., midpoint, pos_a, pos_b, color_a.to_gamma_srgb_channels(), color_b.to_gamma_srgb_channels(), &mut result, 0);
			}

			// Add the end stop
			result.push((pos_b, color_b, Some(next_midpoint)));
		}

		// If every midpoint is 0.5 (or within epsilon), turn all midpoints to None
		if result.iter().all(|(_, _, midpoint)| matches!(midpoint, Some(m) if (m - 0.5).abs() < 1e-6)) {
			result.iter_mut().for_each(|(_, _, midpoint)| *midpoint = None);
		}

		result
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		let stops = self.iter().zip(other.iter()).map(|(a, b)| {
			let position = a.position + (b.position - a.position) * time;
			let color = a.color.lerp(&b.color, time as f32);
			GradientStop { position, midpoint: 0.5, color }
		});
		Gradient::new(stops)
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum GradientSpreadMethod {
	#[default]
	Pad,
	Reflect,
	Repeat,
}

impl GradientSpreadMethod {
	pub fn svg_name(&self) -> &'static str {
		match self {
			GradientSpreadMethod::Pad => "pad",
			GradientSpreadMethod::Reflect => "reflect",
			GradientSpreadMethod::Repeat => "repeat",
		}
	}
}

/// Resolved patch of a mesh gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshPatch {
	/// Corner positions. [top-left, top-right, bottom-left, bottom-right]
	pub corners: [DVec2; 4],
	/// Corner colors. [top-left, top-right, bottom-left, bottom-right]
	pub colors: [Color; 4],
	/// Edges defining the patch. [top, bottom, left, right]
	pub edges: [PathSeg; 4],
}

/// Definition of a mesh gradient patch. The entity is stored in the corresponding `MeshGradient` struct.
#[derive(Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct MeshPatchDefinition {
	/// Corner indices in MeshGradient::corner_points/corner_colors. [top-left, top-right, bottom-left, bottom-right]
	corner_indices: [usize; 4],
	/// Segment ids of edges defining the patch. [top, bottom, left, right]
	edges: [SegmentId; 4],
}

/// Mesh gradient defined by multiple coons patches.
#[derive(Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeshGradient {
	mesh_geometry: Vector,
	pub corner_rows: usize,
	pub corner_columns: usize,
	/// Flatten list of corner's point IDs, row-major order.
	corner_points: Vec<PointId>,
	/// Flatten list of corner colors, row-major order.
	corner_colors: Vec<Color>,
	/// Flatten list of mesh patches, row-major order.
	patch_defs: Vec<MeshPatchDefinition>,
}

impl Default for MeshGradient {
	fn default() -> Self {
		// Build 2x2 patches
		let corner_rows = 3;
		let corner_columns = 3;
		let positions: Vec<DVec2> = (0..corner_rows)
			.flat_map(|row| {
				let v = row as f64 / (corner_rows - 1) as f64;
				(0..corner_columns).map(move |column| {
					let u = column as f64 / (corner_columns - 1) as f64;
					DVec2::new(u, v)
				})
			})
			.collect();

		MeshGradient::from_positions(positions.as_slice(), corner_rows, corner_columns).expect("2x2 patches should be valid mesh gradient")
	}
}

impl MeshGradient {
	pub fn from_positions(positions: &[DVec2], corner_rows: usize, corner_columns: usize) -> Option<Self> {
		if corner_rows < 2 || corner_columns < 2 {
			return None;
		}

		let corner_count = corner_rows.checked_mul(corner_columns)?;
		if positions.len() != corner_count {
			return None;
		}

		let mut vector = Vector::default();
		let mut corner_points = Vec::with_capacity(corner_count);

		for &position in positions {
			let point_id = vector.point_domain.next_id();
			vector.point_domain.push(point_id, position);
			corner_points.push(point_id);
		}

		let mut horizontal_edges = Vec::with_capacity(corner_rows * (corner_columns - 1));
		for row in 0..corner_rows {
			for column in 0..(corner_columns - 1) {
				let start_index = row * corner_columns + column;
				let end_index = start_index + 1;

				let segment_id = vector.segment_domain.next_id();
				vector.push(segment_id, corner_points[start_index], corner_points[end_index], (None, None), StrokeId::ZERO);
				horizontal_edges.push(segment_id);
			}
		}

		let mut vertical_edges = Vec::with_capacity((corner_rows - 1) * corner_columns);
		for row in 0..(corner_rows - 1) {
			for column in 0..corner_columns {
				let start_index = row * corner_columns + column;
				let end_index = start_index + corner_columns;

				let segment_id = vector.segment_domain.next_id();
				vector.push(segment_id, corner_points[start_index], corner_points[end_index], (None, None), StrokeId::ZERO);
				vertical_edges.push(segment_id);
			}
		}

		// FIXME: alternative color is only for testing purpose
		let corner_colors = (0..corner_rows)
			.flat_map(|row| {
				(0..corner_columns).map(move |column| {
					let luminance = (row + column).is_multiple_of(2) as u8 as f32;
					Color::from_luminance(luminance)
				})
			})
			.collect();

		let patch_rows = corner_rows - 1;
		let patch_columns = corner_columns - 1;
		let mut patch_defs = Vec::with_capacity((corner_rows - 1) * (corner_columns - 1));
		for row in 0..patch_rows {
			for column in 0..patch_columns {
				let top_edge_index = row * patch_columns + column;
				let bottom_edge_index = (row + 1) * patch_columns + column;
				let left_edge_index = row * corner_columns + column;
				let right_edge_index = left_edge_index + 1;
				let edges = [
					horizontal_edges[top_edge_index],
					horizontal_edges[bottom_edge_index],
					vertical_edges[left_edge_index],
					vertical_edges[right_edge_index],
				];
				let top_left = row * corner_columns + column;
				let top_right = top_left + 1;
				let bottom_left = top_left + corner_columns;
				let bottom_right = bottom_left + 1;
				let corner_indices = [top_left, top_right, bottom_left, bottom_right];
				patch_defs.push(MeshPatchDefinition { corner_indices, edges });
			}
		}

		Some(Self {
			mesh_geometry: vector,
			corner_rows,
			corner_columns,
			corner_points,
			corner_colors,
			patch_defs,
		})
	}

	fn resolve_patch(&self, patch_def: &MeshPatchDefinition) -> Option<MeshPatch> {
		// Normalizes the orientation of the patch edge.
		let resolve_oriented_edge = |segment_id: SegmentId, expected_start: PointId, expected_end: PointId| -> Option<PathSeg> {
			let (actual_start, actual_end, _) = self.mesh_geometry.segment_points_from_id(segment_id)?;
			let segment = self.mesh_geometry.path_segment_from_id(segment_id)?;

			if actual_start == expected_start && actual_end == expected_end {
				Some(segment)
			} else if actual_start == expected_end && actual_end == expected_start {
				Some(segment.reverse())
			} else {
				// Segment is not connected to the expected patch corners.
				None
			}
		};

		let [top_left_index, top_right_index, bottom_left_index, bottom_right_index] = patch_def.corner_indices;

		let top_left_id = *self.corner_points.get(top_left_index)?;
		let top_right_id = *self.corner_points.get(top_right_index)?;
		let bottom_left_id = *self.corner_points.get(bottom_left_index)?;
		let bottom_right_id = *self.corner_points.get(bottom_right_index)?;

		let corners = [
			self.mesh_geometry.point_domain.position_from_id(top_left_id)?,
			self.mesh_geometry.point_domain.position_from_id(top_right_id)?,
			self.mesh_geometry.point_domain.position_from_id(bottom_left_id)?,
			self.mesh_geometry.point_domain.position_from_id(bottom_right_id)?,
		];

		let colors = [
			*self.corner_colors.get(top_left_index)?,
			*self.corner_colors.get(top_right_index)?,
			*self.corner_colors.get(bottom_left_index)?,
			*self.corner_colors.get(bottom_right_index)?,
		];

		let [top_edge_id, bottom_edge_id, left_edge_id, right_edge_id] = patch_def.edges;

		let edges = [
			resolve_oriented_edge(top_edge_id, top_left_id, top_right_id)?,
			resolve_oriented_edge(bottom_edge_id, bottom_left_id, bottom_right_id)?,
			resolve_oriented_edge(left_edge_id, top_left_id, bottom_left_id)?,
			resolve_oriented_edge(right_edge_id, top_right_id, bottom_right_id)?,
		];

		Some(MeshPatch { corners, colors, edges })
	}

	// Returns resolved patch by the provided row/column position, if any.
	fn patch(&self, row: usize, column: usize) -> Option<MeshPatch> {
		let patch_rows = self.corner_rows.checked_sub(1)?;
		let patch_columns = self.corner_columns.checked_sub(1)?;

		if row >= patch_rows || column >= patch_columns {
			return None;
		}

		let patch_columns = self.corner_columns.saturating_sub(1);
		let patch_index = row.checked_mul(patch_columns)?.checked_add(column)?;
		let patch_def = self.patch_defs.get(patch_index)?;

		self.resolve_patch(patch_def)
	}

	pub fn patches(&self) -> impl Iterator<Item = Option<MeshPatch>> + '_ {
		let patch_rows = self.corner_rows.saturating_sub(1);
		let patch_columns = self.corner_columns.saturating_sub(1);
		(0..patch_rows).flat_map(move |row| (0..patch_columns).map(move |column| self.patch(row, column)))
	}

	pub fn evaluator(&self) -> Option<MeshGradientEvaluator> {
		MeshGradientEvaluator::new(self)
	}
}

/// Single vertex of a subpatch. Only for rendering purpose.
#[derive(Clone, Copy)]

pub struct MeshSubpatchVertex {
	pub position: DVec2,
	pub gamma_color: [f32; 4],
}

pub struct MeshSubpatch {
	pub corners: [MeshSubpatchVertex; 4],
}

#[derive(Clone, Copy)]
struct MeshCornerDerivatives {
	u: Vec4,
	v: Vec4,
}

/// A cached mesh patch for subdivision into subpatches in rendering phase.
#[derive(Clone, Copy)]
struct MeshPatchEvaluator {
	/// Corner positions. [top-left, top-right, bottom-left, bottom-right]
	pub corners: [DVec2; 4],
	/// Edges defining the patch. [top, bottom, left, right]
	pub edges: [PathSeg; 4],
	// sRGB gamma space color in 0.-1. [top-left, top-right, bottom-left, bottom-right]
	gamma_colors: [Vec4; 4],
	/// Slopes of corner colors for bicubic hermite interpolation. [top-left, top-right, bottom-left, bottom-right]
	color_slopes: [MeshCornerDerivatives; 4],
	/// Linear length of between each corner. [top, bottom, left, right]
	lengths: [f32; 4],
}

impl MeshPatchEvaluator {
	/// Evaluate interpolated color in a mesh gradient's patch using bicubic hermite interpolation.
	fn eval_color(&self, u: f32, v: f32) -> Option<[f32; 4]> {
		if !(0. ..=1.).contains(&u) || !(0. ..=1.).contains(&v) {
			return None;
		}

		let hermite = |a: f32, ma: f32, b: f32, mb: f32, t: f32| -> f32 {
			let t_power_2 = t * t;
			let t_power_3 = t_power_2 * t;

			let h1 = 2. * t_power_3 - 3. * t_power_2 + 1.;
			let h2 = -2. * t_power_3 + 3. * t_power_2;
			let h3 = t_power_3 - 2. * t_power_2 + t;
			let h4 = t_power_3 - t_power_2;

			ma * h3 + a * h1 + b * h2 + mb * h4
		};

		let [top_left_gamma, top_right_gamma, bottom_left_gamma, bottom_right_gamma] = self.gamma_colors;
		let [top_length, bottom_length, left_length, right_length] = self.lengths;
		let [top_left_color_slope, top_right_color_slope, bottom_left_color_slope, bottom_right_color_slope] = self.color_slopes;

		let interpolated_gamma_color: [f32; 4] = std::array::from_fn(|channel| {
			let top_color_interpolated = hermite(
				top_left_gamma[channel],
				top_left_color_slope.u[channel] * top_length,
				top_right_gamma[channel],
				top_right_color_slope.u[channel] * top_length,
				u,
			);
			let bottom_color_interpolated = hermite(
				bottom_left_gamma[channel],
				bottom_left_color_slope.u[channel] * bottom_length,
				bottom_right_gamma[channel],
				bottom_right_color_slope.u[channel] * bottom_length,
				u,
			);
			let top_slope_interpolated = hermite(top_left_color_slope.v[channel] * left_length, 0., top_right_color_slope.v[channel] * right_length, 0., u);
			let bottom_slope_interpolated = hermite(bottom_left_color_slope.v[channel] * left_length, 0., bottom_right_color_slope.v[channel] * right_length, 0., u);
			hermite(top_color_interpolated, top_slope_interpolated, bottom_color_interpolated, bottom_slope_interpolated, v)
		});

		Some(interpolated_gamma_color)
	}

	fn eval_vertex(&self, u: f64, v: f64, mesh_transform: DAffine2) -> Option<MeshSubpatchVertex> {
		let [top_seg, bottom_seg, left_seg, right_seg] = self.edges;
		let [top_left, top_right, bottom_left, bottom_right] = self.corners;

		let top_u = point_to_dvec2(top_seg.eval(u));
		let bottom_u = point_to_dvec2(bottom_seg.eval(u));
		let left_v = point_to_dvec2(left_seg.eval(v));
		let right_v = point_to_dvec2(right_seg.eval(v));

		let s_c = (1. - v) * top_u + v * bottom_u;
		let s_d = (1. - u) * left_v + u * right_v;
		let s_b = top_left * (1. - u) * (1. - v) + top_right * u * (1. - v) + bottom_left * (1. - u) * v + bottom_right * u * v;

		Some(MeshSubpatchVertex {
			position: mesh_transform.transform_point2(s_c + s_d - s_b),
			gamma_color: self.eval_color(u as f32, v as f32)?,
		})
	}
}

/// Struct for evaluating color for subpatch corners.
/// The main purpose is to prevent duplicated calculation of the slopes for hermite interpolation for each subpatch.
#[derive(Clone)]
pub struct MeshGradientEvaluator {
	/// List of required data for color interpolation, row major order.
	patches: Vec<MeshPatchEvaluator>,
}

impl MeshGradientEvaluator {
	// TODO: probably it is better to use u/v for slope calculation
	pub fn new(mesh_gradient: &MeshGradient) -> Option<Self> {
		let &MeshGradient { corner_columns, corner_rows, .. } = mesh_gradient;
		if corner_rows < 2 || corner_columns < 2 {
			return None;
		}
		let patch_columns = corner_columns - 1;
		let patch_rows = corner_rows - 1;

		if mesh_gradient.patch_defs.len() != patch_rows * patch_columns {
			return None;
		}
		let corner_count = corner_rows.checked_mul(corner_columns)?;
		if mesh_gradient.corner_points.len() != corner_count || mesh_gradient.corner_colors.len() != corner_count {
			return None;
		}

		let corner_positions: Vec<DVec2> = mesh_gradient
			.corner_points
			.iter()
			.map(|&point_id| mesh_gradient.mesh_geometry.point_domain.position_from_id(point_id))
			.collect::<Option<_>>()?;

		// We need to calculate the color derivatives in sRGB since SVG uses sRGB for color interpolation.
		// `color-interpolation="linearRGB"` is part of the SVG2 spec but not yet implemented in major browsers as of Jul. 2026.
		// See also: https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Attribute/color-interpolation
		let gamma_colors: Vec<Vec4> = mesh_gradient.corner_colors.iter().map(|color| Vec4::from_array(color.to_gamma_srgb_channels())).collect();

		// Calculate the slope of the `curr_index` corner by FDM. The slope is derived from the linear distance from the previous/next corners.
		let calculate_color_slope = |prev_index: usize, curr_index: usize, next_index: usize| {
			let prev_color = gamma_colors[prev_index];
			let curr_color = gamma_colors[curr_index];
			let next_color = gamma_colors[next_index];

			let [prev_pos, curr_pos, next_pos] = [prev_index, curr_index, next_index].map(|index| corner_positions[index]);
			let prev_distance = curr_pos.distance(prev_pos) as f32;
			let next_distance = next_pos.distance(curr_pos) as f32;

			if prev_index == curr_index {
				// FIXME: resolve zero-division problem
				(next_color - curr_color) / next_distance
			} else if next_index == curr_index {
				(curr_color - prev_color) / prev_distance
			} else {
				let backward_diff = (curr_color - prev_color) / prev_distance;
				let forward_diff = (next_color - curr_color) / next_distance;
				let central_diff = (backward_diff + forward_diff) / 2.;

				// Prevent overshooting by applying a zero slope at local minimum/maximum
				// TODO: consider clamping slope by a constant value
				Vec4::from_array(std::array::from_fn(
					|channel| {
						if backward_diff[channel] * forward_diff[channel] <= 0. { 0. } else { central_diff[channel] }
					},
				))
			}
		};

		let sample_index = |row: isize, column: isize| -> usize {
			let clamped_column = column.clamp(0, corner_columns as isize - 1) as usize;
			let clamped_row = row.clamp(0, corner_rows as isize - 1) as usize;
			clamped_row * corner_columns + clamped_column
		};

		let mut corner_slopes = Vec::with_capacity(corner_rows * corner_columns);
		for row in 0..corner_rows as isize {
			for col in 0..corner_columns as isize {
				let curr_index = sample_index(row, col);
				let u = calculate_color_slope(sample_index(row, col - 1), curr_index, sample_index(row, col + 1));
				let v = calculate_color_slope(sample_index(row - 1, col), curr_index, sample_index(row + 1, col));
				corner_slopes.push(MeshCornerDerivatives { u, v });
			}
		}

		let mut patch_color_data: Vec<MeshPatchEvaluator> = vec![];
		for patch_def in &mesh_gradient.patch_defs {
			let patch = mesh_gradient.resolve_patch(patch_def)?;
			let gamma_colors = patch_def
				.corner_indices
				.map(|index| gamma_colors.get(index).copied())
				.into_iter()
				.collect::<Option<Vec<_>>>()?
				.try_into()
				.ok()?;

			let color_slopes = patch_def
				.corner_indices
				.map(|index| corner_slopes.get(index).copied())
				.into_iter()
				.collect::<Option<Vec<_>>>()?
				.try_into()
				.ok()?;

			let [top_left_pos, top_right_pos, bottom_left_pos, bottom_right_pos] = patch.corners;
			// [top, bottom, left, right]
			let lengths = [
				top_left_pos.distance(top_right_pos) as f32,
				bottom_left_pos.distance(bottom_right_pos) as f32,
				top_left_pos.distance(bottom_left_pos) as f32,
				top_right_pos.distance(bottom_right_pos) as f32,
			];
			patch_color_data.push(MeshPatchEvaluator {
				corners: patch.corners,
				edges: patch.edges,
				gamma_colors,
				color_slopes,
				lengths,
			});
		}

		Some(Self { patches: patch_color_data })
	}

	/// Subdivide all patches in a mesh into parallelogram subpatches so to renderable by two linear gradients with mask.
	/// Returns subpatchs in row-major.
	pub fn subdivide_patches(&self, subdivisions_per_patch_per_axis: usize, mesh_transform: DAffine2) -> Option<Vec<MeshSubpatch>> {
		let count = subdivisions_per_patch_per_axis;
		if count == 0 {
			return None;
		}

		let capacity = self.patches.len().checked_mul(count)?.checked_mul(count)?;
		let mut subpatches = Vec::with_capacity(capacity);

		for patch in &self.patches {
			let evaluate_row = |row: usize| -> Option<Vec<MeshSubpatchVertex>> {
				let v = row as f64 / count as f64;

				(0..=count)
					.map(|column| {
						let u = column as f64 / count as f64;
						patch.eval_vertex(u, v, mesh_transform)
					})
					.collect()
			};

			// Reusing the previous bottom row as a current top row to prevent duplicated evaluation on the same subpatch vertices.
			let mut top_row = evaluate_row(0)?;
			for row in 0..count {
				let bottom_row = evaluate_row(row + 1)?;
				for column in 0..count {
					subpatches.push(MeshSubpatch {
						corners: [top_row[column], top_row[column + 1], bottom_row[column], bottom_row[column + 1]],
					});
				}

				top_row = bottom_row;
			}
		}

		Some(subpatches)
	}

	/// Recursively subdivide only the regions that do not approximate the source mesh within the given tolerances.
	pub fn subdivide_patches_adaptive(
		&self,
		maximum_subdivisions_per_patch_per_axis: usize,
		mesh_transform: DAffine2,
		position_error_tolerance: f64,
		color_error_tolerance: f32,
	) -> Option<Vec<MeshSubpatch>> {
		if !maximum_subdivisions_per_patch_per_axis.is_power_of_two()
			|| !position_error_tolerance.is_finite()
			|| position_error_tolerance < 0.
			|| !color_error_tolerance.is_finite()
			|| color_error_tolerance < 0.
		{
			return None;
		}

		let samples = [0., 0.25, 0.5, 0.75, 1.];
		let mut subpatches = Vec::new();
		for patch in &self.patches {
			let mut pending = vec![(0., 0., 1., 1_usize)];
			while let Some((u_start, v_start, stride, subdivisions_per_axis)) = pending.pop() {
				let top_left = patch.eval_vertex(u_start, v_start, mesh_transform)?;
				let top_right = patch.eval_vertex(u_start + stride, v_start, mesh_transform)?;
				let bottom_left = patch.eval_vertex(u_start, v_start + stride, mesh_transform)?;
				let bottom_right = patch.eval_vertex(u_start + stride, v_start + stride, mesh_transform)?;
				let corners = [top_left, top_right, bottom_left, bottom_right];
				let [top_left_color, top_right_color, bottom_left_color, bottom_right_color] = corners.map(|vertex| Vec4::from_array(vertex.gamma_color));

				let mut within_tolerance = true;
				'error_samples: for &local_v in &samples {
					for &local_u in &samples {
						let vertex = patch.eval_vertex(u_start + local_u * stride, v_start + local_v * stride, mesh_transform)?;
						let approximated_position = top_left.position + (top_right.position - top_left.position) * local_u + (bottom_left.position - top_left.position) * local_v;
						let top_color = top_left_color.lerp(top_right_color, local_u as f32);
						let bottom_color = bottom_left_color.lerp(bottom_right_color, local_u as f32);
						let approximated_color = top_color.lerp(bottom_color, local_v as f32);

						let position_error = vertex.position.distance(approximated_position);
						let color_error = (Vec4::from_array(vertex.gamma_color) - approximated_color).abs().max_element();
						if !position_error.is_finite() || !color_error.is_finite() || position_error > position_error_tolerance || color_error > color_error_tolerance {
							within_tolerance = false;
							break 'error_samples;
						}
					}
				}

				if within_tolerance || subdivisions_per_axis >= maximum_subdivisions_per_patch_per_axis {
					subpatches.push(MeshSubpatch { corners });
				} else {
					let half_stride = stride / 2.;
					let child_subdivisions_per_axis = subdivisions_per_axis.saturating_mul(2);
					pending.extend([
						(u_start + half_stride, v_start + half_stride, half_stride, child_subdivisions_per_axis),
						(u_start, v_start + half_stride, half_stride, child_subdivisions_per_axis),
						(u_start + half_stride, v_start, half_stride, child_subdivisions_per_axis),
						(u_start, v_start, half_stride, child_subdivisions_per_axis),
					]);
				}
			}
		}

		Some(subpatches)
	}
}

impl RenderComplexity for MeshGradient {
	fn render_complexity(&self) -> usize {
		let patch_rows = self.corner_rows.saturating_sub(1);
		let patch_columns = self.corner_columns.saturating_sub(1);

		// FIXME: probably better to calculate the approximate number of subpatchs
		let subpatch_count_per_patch = 64;
		patch_rows
			.saturating_mul(patch_columns)
			.saturating_mul(subpatch_count_per_patch)
			.saturating_mul(subpatch_count_per_patch)
	}
}

/// Rebuild the y-axis so its (parallel, perpendicular) components in the x-axis-aligned frame stay constant, both
/// rescaled by `|new_x| / |old_x|`. This holds the (x, y) parallelogram's aspect ratio and skew fixed across an endpoint
/// drag, so a radial ellipse stays the same shape (just rotated and resized) instead of distorting as x grows or shrinks.
/// Falls back to a +90° rotation of `new_x` when `old_x` is degenerate.
fn scale_y_axis_to_match_new_x(old_x: DVec2, old_y: DVec2, new_x: DVec2) -> DVec2 {
	let old_x_length = old_x.length();
	if old_x_length < 1e-9 {
		return DVec2::new(-new_x.y, new_x.x);
	}
	let ex_old = old_x / old_x_length;
	let ey_old = DVec2::new(-ex_old.y, ex_old.x);

	let new_x_length = new_x.length();
	if new_x_length < 1e-9 {
		return DVec2::ZERO;
	}
	let ex_new = new_x / new_x_length;
	let ey_new = DVec2::new(-ex_new.y, ex_new.x);

	let parallel = old_y.dot(ex_old);
	let perpendicular = old_y.dot(ey_old);
	let scale = new_x_length / old_x_length;

	scale * (parallel * ex_new + perpendicular * ey_new)
}

/// Build a new affine that maps canonical (0,0) -> (1,0) to (new_start, new_end), preserving the y-axis
/// shape of `old` proportionally to the x-axis length change.
pub fn build_transform_with_y_preservation(old: DAffine2, new_start: DVec2, new_end: DVec2) -> DAffine2 {
	let new_x_axis = new_end - new_start;
	let preserved_y_axis = scale_y_axis_to_match_new_x(old.matrix2.x_axis, old.matrix2.y_axis, new_x_axis);
	DAffine2 {
		matrix2: glam::DMat2::from_cols(new_x_axis, preserved_y_axis),
		translation: new_start,
	}
}

/// Build the default transform for a gradient not yet given one: a horizontal gradient spanning the
/// bounding box's width, running through its vertical middle.
pub fn initial_gradient_transform_for_bounding_box(bounds: [DVec2; 2]) -> DAffine2 {
	let [min, max] = bounds;
	let x_axis = DVec2::new(max.x - min.x, 0.);
	DAffine2 {
		matrix2: glam::DMat2::from_cols(x_axis, x_axis.perp()),
		translation: DVec2::new(min.x, (min.y + max.y) / 2.),
	}
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_to_gradient<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Gradient, D::Error> {
	use serde::Deserialize;

	#[derive(serde::Deserialize)]
	struct LegacyTable {
		#[serde(alias = "instances", alias = "instance")]
		element: Vec<Gradient>,
	}

	#[derive(serde::Deserialize)]
	#[cfg_attr(feature = "serde", serde(untagged))]
	enum GradientStopsFormat {
		Stops(Gradient),
		List(LegacyTable),
	}

	Ok(match GradientStopsFormat::deserialize(deserializer)? {
		GradientStopsFormat::Stops(stops) => stops,
		GradientStopsFormat::List(list) => list.element.into_iter().next().unwrap_or_default(),
	})
}

impl core_types::bounds::BoundingBox for Gradient {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		core_types::bounds::RenderBoundingBox::Infinite
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		// AABB of the gradient line itself, leaving aspect padding and sub-pixel fallbacks to the runtime so this stays
		// a clean per-item geometric bound that combines naturally with siblings
		let start = transform.transform_point2(DVec2::ZERO);
		let end = transform.transform_point2(DVec2::X);
		core_types::bounds::RenderBoundingBox::Rectangle([start.min(end), start.max(end)])
	}
}

impl core_types::bounds::BoundingBox for MeshGradient {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		// FIXME: infinite? finite?
		core_types::bounds::RenderBoundingBox::Infinite
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		// FIXME: implement actual check of the bounding box
		let start = transform.transform_point2(DVec2::ZERO);
		let end = transform.transform_point2(DVec2::X);
		core_types::bounds::RenderBoundingBox::Rectangle([start.min(end), start.max(end)])
	}
}
