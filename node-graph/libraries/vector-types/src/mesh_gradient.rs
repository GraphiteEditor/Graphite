use core_types::list::{ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPACE, Item};
use core_types::{Color, render_complexity::RenderComplexity};
use dyn_any::DynAny;
use glam::{DAffine2, DMat2, DVec2, Mat4, Vec4};
use kurbo::{BezPath, ParamCurve, PathSeg};

use crate::{
	Vector,
	gradient::{GradientInterpolation, GradientSpace, color_from_gradient_space_channels, gradient_space_channels},
	subpath::{BezierHandles, pathseg_points},
	vector::{
		PointId, SegmentId, StrokeId,
		algorithms::util::pathseg_tangent,
		misc::{HandleId, HandleType, point_to_dvec2},
	},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshGradientCorner {
	pub index: usize,
	pub point_id: PointId,
	pub position: DVec2,
	pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshGradientEdge {
	pub segment_id: SegmentId,
	pub segment: PathSeg,
	pub start: PointId,
	pub end: PointId,
}

/// Resolved patch of a mesh gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshPatch {
	/// Patch index in row-major order.
	pub index: usize,
	/// Corner positions. [top-left, top-right, bottom-left, bottom-right]
	pub corners: [DVec2; 4],
	/// Corner colors. [top-left, top-right, bottom-left, bottom-right]
	pub colors: [Color; 4],
	/// Edges defining the patch. [top, bottom, left, right]
	pub edges: [PathSeg; 4],
}

impl MeshPatch {
	/// The patch outline as one closed subpath, in mesh-local coordinates.
	/// Walks `top`, `right`, then `bottom` and `left` reversed, which is the only traversal of [`Self::edges`]'s
	/// `[top, bottom, left, right]` order that stays connected end-to-end.
	pub fn boundary_path(&self) -> BezPath {
		let [top, bottom, left, right] = self.edges;
		let mut boundary = BezPath::from_path_segments([top, right, bottom.reverse(), left.reverse()].into_iter());
		boundary.close_path();
		boundary
	}

	/// Checks for foldovers by sampling the position Jacobian over the patch.
	pub fn sampled_no_foldover(&self) -> bool {
		const SUBDIVISIONS: usize = 64;
		const RELATIVE_EPSILON: f64 = 1e-6;
		const FOLDOVER_SAFETY_ANGLE_DEGREES: f64 = 5.;
		let minimum_normalized_jacobian = FOLDOVER_SAFETY_ANGLE_DEGREES.to_radians().sin();

		for row in 0..=SUBDIVISIONS {
			let v = row as f64 / SUBDIVISIONS as f64;
			for column in 0..=SUBDIVISIONS {
				let u = column as f64 / SUBDIVISIONS as f64;
				let jacobian = position_jacobian(self.corners, self.edges, u, v);
				let derivative_u = jacobian.x_axis;
				let derivative_v = jacobian.y_axis;
				let scale = derivative_u.length() * derivative_v.length();
				let determinant = derivative_u.perp_dot(derivative_v);

				if !scale.is_finite() || !determinant.is_finite() || determinant <= (RELATIVE_EPSILON + minimum_normalized_jacobian) * scale {
					return false;
				}
			}
		}

		true
	}
}

/// Row-major storage for values arranged in a rectangular mesh grid.
#[derive(Debug, Clone, PartialEq, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct MeshGrid<T> {
	rows: usize,
	columns: usize,
	values: Vec<T>,
}

impl<T> MeshGrid<T> {
	fn new(values: Vec<T>, rows: usize, columns: usize) -> Option<Self> {
		(values.len() == rows.checked_mul(columns)?).then_some(Self { rows, columns, values })
	}

	fn index(&self, row: usize, column: usize) -> Option<usize> {
		if row >= self.rows || column >= self.columns {
			return None;
		}
		row.checked_mul(self.columns)?.checked_add(column)
	}

	fn get(&self, row: usize, column: usize) -> Option<&T> {
		self.values.get(self.index(row, column)?)
	}

	fn get_flat(&self, index: usize) -> Option<&T> {
		self.values.get(index)
	}

	fn get_flat_mut(&mut self, index: usize) -> Option<&mut T> {
		self.values.get_mut(index)
	}

	fn dimensions(&self) -> [usize; 2] {
		[self.rows, self.columns]
	}

	fn splice_lines(&mut self, axis: MeshGridLineAxis, removed: std::ops::Range<usize>, inserted_lines: &[&[T]]) -> Option<()>
	where
		T: Copy,
	{
		let [across_count, along_count] = axis.logical_indices(self.rows, self.columns);
		if removed.start > removed.end || removed.end > along_count || inserted_lines.iter().any(|line| line.len() != across_count) {
			return None;
		}

		let removed_count = removed.end - removed.start;
		let inserted_count = inserted_lines.len();
		let new_along_count = along_count - removed_count + inserted_count;
		let [new_rows, new_columns] = axis.physical_indices(across_count, new_along_count);
		let mut new_values = Vec::with_capacity(new_rows.checked_mul(new_columns)?);

		for new_row in 0..new_rows {
			for new_column in 0..new_columns {
				let [across, along] = axis.logical_indices(new_row, new_column);
				if along >= removed.start && along < removed.start + inserted_count {
					new_values.push(inserted_lines[along - removed.start][across]);
				} else {
					let original_along = if along < removed.start { along } else { along - inserted_count + removed_count };
					let [original_row, original_column] = axis.physical_indices(across, original_along);
					new_values.push(self.values[original_row * self.columns + original_column]);
				}
			}
		}

		self.rows = new_rows;
		self.columns = new_columns;
		self.values = new_values;
		Some(())
	}
}

/// Maps row and column insertion onto one operation that splits edges along an axis and connects them across the other axis.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MeshGridLineAxis {
	Row,
	Column,
}

impl MeshGridLineAxis {
	fn physical_indices(self, across: usize, along: usize) -> [usize; 2] {
		match self {
			Self::Column => [across, along],
			Self::Row => [along, across],
		}
	}

	fn logical_indices(self, row: usize, column: usize) -> [usize; 2] {
		match self {
			Self::Column => [row, column],
			Self::Row => [column, row],
		}
	}

	fn uv(self, along: f32, across: f32) -> [f32; 2] {
		match self {
			Self::Column => [along, across],
			Self::Row => [across, along],
		}
	}

	fn edge_grids<'a, T>(self, horizontal: &'a MeshGrid<T>, vertical: &'a MeshGrid<T>) -> (&'a MeshGrid<T>, &'a MeshGrid<T>) {
		match self {
			Self::Column => (horizontal, vertical),
			Self::Row => (vertical, horizontal),
		}
	}

	fn edge_grids_mut<'a, T>(self, horizontal: &'a mut MeshGrid<T>, vertical: &'a mut MeshGrid<T>) -> (&'a mut MeshGrid<T>, &'a mut MeshGrid<T>) {
		match self {
			Self::Column => (horizontal, vertical),
			Self::Row => (vertical, horizontal),
		}
	}
}

/// The serialized exchange form of a mesh gradient: its patches, with whole-mesh settings as sibling fields
/// serialized only when non-default.
#[derive(Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeshGradientSurface {
	pub mesh: MeshGradient,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "GradientSpace::is_default"))]
	pub gradient_space: GradientSpace,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "GradientInterpolation::is_default"))]
	pub gradient_interpolation: GradientInterpolation,
}

impl Default for MeshGradientSurface {
	fn default() -> Self {
		Self {
			mesh: MeshGradient::default(),
			gradient_space: GradientSpace::default(),
			gradient_interpolation: GradientInterpolation::Smooth,
		}
	}
}

impl From<MeshGradient> for MeshGradientSurface {
	fn from(mesh: MeshGradient) -> Self {
		Self { mesh, ..Default::default() }
	}
}

// The runtime wire form: whole-mesh settings ride as the mesh gradient item's attributes in its containing list,
// where the Fill kernel, chain setter nodes, and renderers read and write them
impl From<MeshGradientSurface> for Item<MeshGradient> {
	fn from(surface: MeshGradientSurface) -> Self {
		let mut item = Item::new_from_element(surface.mesh);
		if !surface.gradient_space.is_default() {
			item.set_attribute(ATTR_GRADIENT_SPACE, surface.gradient_space);
		}
		if !surface.gradient_interpolation.is_default() {
			item.set_attribute(ATTR_GRADIENT_INTERPOLATION, surface.gradient_interpolation);
		}
		item
	}
}

impl From<&Item<MeshGradient>> for MeshGradientSurface {
	fn from(item: &Item<MeshGradient>) -> Self {
		Self {
			mesh: item.element().clone(),
			gradient_space: item.attribute_cloned_or_default(ATTR_GRADIENT_SPACE),
			gradient_interpolation: item.attribute_cloned_or_default(ATTR_GRADIENT_INTERPOLATION),
		}
	}
}

/// Returns the affine that fits the mesh gradient geometry to the provided bounds.
pub fn initial_mesh_gradient_transform_for_bounding_box(bounds: [DVec2; 2]) -> DAffine2 {
	let [min, max] = bounds;
	let size = max - min;
	DAffine2::from_cols(DVec2::new(size.x, 0.), DVec2::new(0., size.y), min)
}

/// Mesh gradient defined by multiple coons patches.
#[derive(Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeshGradient {
	mesh_geometry: Vector,
	corner_points: MeshGrid<PointId>,
	corner_colors: MeshGrid<Color>,
	horizontal_edges: MeshGrid<SegmentId>,
	vertical_edges: MeshGrid<SegmentId>,
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
	/// Create a new mesh gradient alternates black and white from the provided row-major corner positions.
	pub fn from_positions(positions: &[DVec2], corner_rows: usize, corner_columns: usize) -> Option<Self> {
		if corner_rows < 2 || corner_columns < 2 {
			return None;
		}

		let corner_count = corner_rows.checked_mul(corner_columns)?;
		if positions.len() != corner_count {
			return None;
		}

		let mut mesh_geometry = Vector::default();
		let mut corner_points = Vec::with_capacity(corner_count);

		for &position in positions {
			let point_id = mesh_geometry.point_domain.next_id();
			mesh_geometry.point_domain.push(point_id, position);
			corner_points.push(point_id);
		}

		let mut horizontal_edges = Vec::with_capacity(corner_rows * (corner_columns - 1));
		for row in 0..corner_rows {
			for column in 0..(corner_columns - 1) {
				let start_index = row * corner_columns + column;
				let end_index = start_index + 1;

				let segment_id = mesh_geometry.segment_domain.next_id();
				mesh_geometry.push(
					segment_id,
					corner_points[start_index],
					corner_points[end_index],
					line_to_cubic_bezier_handles(positions[start_index], positions[end_index]),
					StrokeId::ZERO,
				);
				horizontal_edges.push(segment_id);
			}
		}

		let mut vertical_edges = Vec::with_capacity((corner_rows - 1) * corner_columns);
		for row in 0..(corner_rows - 1) {
			for column in 0..corner_columns {
				let start_index = row * corner_columns + column;
				let end_index = start_index + corner_columns;

				let segment_id = mesh_geometry.segment_domain.next_id();
				mesh_geometry.push(
					segment_id,
					corner_points[start_index],
					corner_points[end_index],
					line_to_cubic_bezier_handles(positions[start_index], positions[end_index]),
					StrokeId::ZERO,
				);
				vertical_edges.push(segment_id);
			}
		}

		let corner_colors = (0..corner_rows)
			.flat_map(|row| {
				(0..corner_columns).map(move |column| {
					let luminance = (row + column).is_multiple_of(2) as u8 as f32;
					Color::from_luminance(luminance)
				})
			})
			.collect();

		Some(Self {
			mesh_geometry,
			corner_points: MeshGrid::new(corner_points, corner_rows, corner_columns)?,
			corner_colors: MeshGrid::new(corner_colors, corner_rows, corner_columns)?,
			horizontal_edges: MeshGrid::new(horizontal_edges, corner_rows, corner_columns - 1)?,
			vertical_edges: MeshGrid::new(vertical_edges, corner_rows - 1, corner_columns)?,
		})
	}

	/// Returns the number of corners.
	pub fn size(&self) -> usize {
		self.corner_points.rows * self.corner_points.columns
	}

	/// Returns resolved patch by the provided row/column position, if any.
	fn patch(&self, row: usize, column: usize) -> Option<MeshPatch> {
		let patch_columns = self.corner_points.columns.saturating_sub(1);
		let index = row * patch_columns + column;

		let top_left_id = *self.corner_points.get(row, column)?;
		let top_right_id = *self.corner_points.get(row, column + 1)?;
		let bottom_left_id = *self.corner_points.get(row + 1, column)?;
		let bottom_right_id = *self.corner_points.get(row + 1, column + 1)?;

		let corners = [
			self.mesh_geometry.point_domain.position_from_id(top_left_id)?,
			self.mesh_geometry.point_domain.position_from_id(top_right_id)?,
			self.mesh_geometry.point_domain.position_from_id(bottom_left_id)?,
			self.mesh_geometry.point_domain.position_from_id(bottom_right_id)?,
		];

		let colors = [
			*self.corner_colors.get(row, column)?,
			*self.corner_colors.get(row, column + 1)?,
			*self.corner_colors.get(row + 1, column)?,
			*self.corner_colors.get(row + 1, column + 1)?,
		];

		let top_edge_id = *self.horizontal_edges.get(row, column)?;
		let bottom_edge_id = *self.horizontal_edges.get(row + 1, column)?;
		let left_edge_id = *self.vertical_edges.get(row, column)?;
		let right_edge_id = *self.vertical_edges.get(row, column + 1)?;

		let edges = [
			self.mesh_geometry.path_segment_from_id(top_edge_id)?,
			self.mesh_geometry.path_segment_from_id(bottom_edge_id)?,
			self.mesh_geometry.path_segment_from_id(left_edge_id)?,
			self.mesh_geometry.path_segment_from_id(right_edge_id)?,
		];

		Some(MeshPatch { index, corners, colors, edges })
	}

	/// The union of every resolvable patch outline, in mesh-local coordinates.
	pub fn boundary_path(&self) -> BezPath {
		let mut boundary = BezPath::new();
		for patch in self.patches().flatten() {
			boundary.extend(patch.boundary_path());
		}
		boundary
	}

	/// Iterator over all of the mesh gradient patches by row-major order, `None` if the patch is defined in unexpected structure.
	pub fn patches(&self) -> impl Iterator<Item = Option<MeshPatch>> + '_ {
		let patch_rows = self.corner_points.rows.saturating_sub(1);
		let patch_columns = self.corner_points.columns.saturating_sub(1);
		(0..patch_rows).flat_map(move |row| (0..patch_columns).map(move |column| self.patch(row, column)))
	}

	// TODO: Research the way to handle polar color spaces for mesh gradient
	/// Returns a new `MeshGradientEvaluator` whose Hermite color field is expressed in `space`.
	pub fn evaluator(&self, space: GradientSpace, interpolation: GradientInterpolation) -> Option<MeshGradientEvaluator> {
		if space.is_polar() {
			return None;
		}
		MeshGradientEvaluator::new(self, space, interpolation)
	}

	/// Returns the read only mesh gradient's geometry.
	pub fn geometry(&self) -> &Vector {
		&self.mesh_geometry
	}

	/// Returns an iterator of all corners data by row-major order.
	pub fn corners(&self) -> impl Iterator<Item = MeshGradientCorner> + '_ {
		self.corner_points
			.values
			.iter()
			.copied()
			.zip(self.corner_colors.values.iter().copied())
			.enumerate()
			.filter_map(|(index, (point_id, color))| {
				let position = self.mesh_geometry.point_domain.position_from_id(point_id)?;
				Some(MeshGradientCorner { index, point_id, position, color })
			})
	}

	/// Returns an iterator of all edges data by row-major order.
	pub fn edges(&self) -> impl Iterator<Item = MeshGradientEdge> + '_ {
		self.mesh_geometry
			.segment_iter()
			.map(|(segment_id, segment, start, end)| MeshGradientEdge { segment_id, segment, start, end })
	}

	/// Set the corner position by flat corner index. The corresponding handles are also moved same amount.
	pub fn set_corner_position(&mut self, corner_index: usize, position: DVec2) -> Option<()> {
		let point_id = *self.corner_points.get_flat(corner_index)?;
		let point_index = self.mesh_geometry.point_domain.resolve_id(point_id)?;
		let previous_position = *self.mesh_geometry.point_domain.positions().get(point_index)?;
		let delta = position - previous_position;

		for (_, handles, start, end) in self.mesh_geometry.handles_mut() {
			if start == point_id {
				handles.move_start(delta);
			}
			if end == point_id {
				handles.move_end(delta);
			}
		}

		self.mesh_geometry.point_domain.set_position(point_index, position);

		Some(())
	}

	/// Set the corner color by flat corner index.
	pub fn set_corner_color(&mut self, corner_index: usize, color: Color) -> Option<()> {
		*self.corner_colors.get_flat_mut(corner_index)? = color;
		Some(())
	}

	pub fn set_edge_handles(&mut self, segment_id: SegmentId, new_handles: BezierHandles) -> Option<()> {
		let (_, handles, _, _) = self.mesh_geometry.handles_mut().find(|(id, _, _, _)| *id == segment_id)?;
		*handles = new_handles;
		Some(())
	}

	pub fn set_handle_position(&mut self, handle_id: HandleId, new_position: DVec2) -> Option<()> {
		let (_, handles, _, _) = self.mesh_geometry.handles_mut().find(|(segment_id, _, _, _)| *segment_id == handle_id.segment)?;

		match (handle_id.ty, handles) {
			(HandleType::Primary, BezierHandles::Quadratic { handle }) => {
				*handle = new_position;
			}
			(HandleType::Primary, BezierHandles::Cubic { handle_start, .. }) => {
				*handle_start = new_position;
			}
			(HandleType::End, BezierHandles::Cubic { handle_end, .. }) => {
				*handle_end = new_position;
			}
			_ => return None,
		}

		Some(())
	}

	/// Finds which grid axis contains the segment and its patch index along that axis.
	fn grid_line_axis(&self, segment_id: SegmentId) -> Option<(MeshGridLineAxis, usize)> {
		let (axis, split_patch_index) = if let Some(index) = self.horizontal_edges.values.iter().position(|&id| id == segment_id) {
			(MeshGridLineAxis::Column, index % self.horizontal_edges.columns)
		} else {
			let index = self.vertical_edges.values.iter().position(|&id| id == segment_id)?;
			(MeshGridLineAxis::Row, index / self.vertical_edges.columns)
		};

		Some((axis, split_patch_index))
	}

	/// Inserts a new grid line through the provided segment at the given parameter. The time has to be within (0, 1).
	pub fn insert_grid_line(&mut self, segment_id: SegmentId, space: GradientSpace, interpolation: GradientInterpolation, time: f64) -> Option<()> {
		#[derive(Clone, Copy)]
		struct SegmentToSplit {
			segment_id: SegmentId,
			start_point_id: PointId,
			end_point_id: PointId,
			segment: PathSeg,
		}

		if !(0. < time && time < 1.) {
			return None;
		}

		let evaluator = self.evaluator(space, interpolation)?;
		let (axis, split_patch_index) = self.grid_line_axis(segment_id)?;
		let (split_edge_grid, _) = axis.edge_grids(&self.horizontal_edges, &self.vertical_edges);
		let [across_corner_count, _] = axis.logical_indices(split_edge_grid.rows, split_edge_grid.columns);
		let grid_line_insertion_index = split_patch_index + 1;
		let across_patch_count = across_corner_count - 1;
		let patch_columns = self.corner_points.columns - 1;

		// Collect the existing segments that will be split by inserting new corners
		let segments_to_split: Vec<SegmentToSplit> = (0..across_corner_count)
			.map(|across| {
				let [edge_row, edge_column] = axis.physical_indices(across, split_patch_index);
				let segment_id = *split_edge_grid.get(edge_row, edge_column)?;
				let [start_point_id, end_point_id] = self.mesh_geometry.points_from_id(segment_id)?;
				let segment = self.mesh_geometry.path_segment_from_id(segment_id)?;
				Some(SegmentToSplit {
					segment_id,
					start_point_id,
					end_point_id,
					segment,
				})
			})
			.collect::<Option<_>>()?;

		// Calculate the new corners' information
		let new_corner_positions: Vec<DVec2> = segments_to_split.iter().map(|source| point_to_dvec2(source.segment.eval(time))).collect();
		let new_corner_colors: Vec<Color> = (0..across_corner_count)
			.map(|across| {
				let (patch_across, across_t) = if across < across_patch_count { (across, 0.) } else { (across - 1, 1.) };
				let [patch_row, patch_column] = axis.physical_indices(patch_across, split_patch_index);
				let patch_index = patch_row * patch_columns + patch_column;
				let [u, v] = axis.uv(time as f32, across_t);
				let [r, g, b, a] = evaluator.evaluate_color(patch_index, u, v);
				Color::from_gamma_srgb_channels(r, g, b, a)
			})
			.collect();

		let mut new_corner_ids = Vec::with_capacity(across_corner_count);
		for &position in &new_corner_positions {
			let point_id = self.mesh_geometry.point_domain.next_id();
			self.mesh_geometry.point_domain.push(point_id, position);
			new_corner_ids.push(point_id);
		}

		// Split the existing segments by the new corners
		let mut first_split_edges = Vec::with_capacity(across_corner_count);
		let mut second_split_edges = Vec::with_capacity(across_corner_count);
		for (source, &inserted_corner) in segments_to_split.iter().zip(&new_corner_ids) {
			let first_half = pathseg_points(source.segment.subsegment(0. ..time));
			let second_half = pathseg_points(source.segment.subsegment(time..1.));

			let first_segment_id = self.mesh_geometry.segment_domain.next_id();
			self.mesh_geometry
				.push(first_segment_id, source.start_point_id, inserted_corner, (first_half.p1, first_half.p2), StrokeId::ZERO);
			first_split_edges.push(first_segment_id);

			let second_segment_id = self.mesh_geometry.segment_domain.next_id();
			self.mesh_geometry
				.push(second_segment_id, inserted_corner, source.end_point_id, (second_half.p1, second_half.p2), StrokeId::ZERO);
			second_split_edges.push(second_segment_id);
		}

		// Create new segments along the axis
		let mut connecting_edges = Vec::with_capacity(across_patch_count);
		for (corner_pair, position_pair) in new_corner_ids.windows(2).zip(new_corner_positions.windows(2)) {
			let &[start, end] = corner_pair else { unreachable!() };
			let &[start_position, end_position] = position_pair else { unreachable!() };
			let connecting_segment_id = self.mesh_geometry.segment_domain.next_id();
			self.mesh_geometry
				.push(connecting_segment_id, start, end, line_to_cubic_bezier_handles(start_position, end_position), StrokeId::ZERO);
			connecting_edges.push(connecting_segment_id);
		}

		self.corner_points.splice_lines(axis, grid_line_insertion_index..grid_line_insertion_index, &[&new_corner_ids])?;
		self.corner_colors.splice_lines(axis, grid_line_insertion_index..grid_line_insertion_index, &[&new_corner_colors])?;
		let (split_edge_grid, connecting_edge_grid) = axis.edge_grids_mut(&mut self.horizontal_edges, &mut self.vertical_edges);
		split_edge_grid.splice_lines(axis, split_patch_index..grid_line_insertion_index, &[&first_split_edges, &second_split_edges])?;
		connecting_edge_grid.splice_lines(axis, grid_line_insertion_index..grid_line_insertion_index, &[&connecting_edges])?;

		let replaced_edges: Vec<_> = segments_to_split.iter().map(|source| source.segment_id).collect();
		let point_count = self.mesh_geometry.point_domain.ids().len();
		self.mesh_geometry.segment_domain.retain(|id| !replaced_edges.contains(id), point_count);

		Some(())
	}

	/// Removes the interior grid line containing the provided segment.
	pub fn remove_edge(&mut self, segment_id: SegmentId) -> Option<()> {
		let (axis, grid_line_index) = if let Some(index) = self.horizontal_edges.values.iter().position(|&id| id == segment_id) {
			(MeshGridLineAxis::Row, index / self.horizontal_edges.columns)
		} else {
			let index = self.vertical_edges.values.iter().position(|&id| id == segment_id)?;
			(MeshGridLineAxis::Column, index % self.vertical_edges.columns)
		};

		let [across_corner_count, grid_line_count] = axis.logical_indices(self.corner_points.rows, self.corner_points.columns);
		if grid_line_index == 0 || grid_line_index + 1 >= grid_line_count {
			return None;
		}

		let (split_edge_grid, connecting_edge_grid) = axis.edge_grids(&self.horizontal_edges, &self.vertical_edges);
		let removed_corner_ids: Vec<PointId> = (0..across_corner_count)
			.map(|across| {
				let [row, column] = axis.physical_indices(across, grid_line_index);
				self.corner_points.get(row, column).copied()
			})
			.collect::<Option<_>>()?;

		let mut merged_edges = Vec::with_capacity(across_corner_count);
		let mut removed_edge_ids = Vec::with_capacity(across_corner_count * 2 + across_corner_count - 1);
		for across in 0..across_corner_count {
			let [first_row, first_column] = axis.physical_indices(across, grid_line_index - 1);
			let [second_row, second_column] = axis.physical_indices(across, grid_line_index);
			let first_segment_id = *split_edge_grid.get(first_row, first_column)?;
			let second_segment_id = *split_edge_grid.get(second_row, second_column)?;
			let first_segment = self.mesh_geometry.path_segment_from_id(first_segment_id)?.to_cubic();
			let second_segment = self.mesh_geometry.path_segment_from_id(second_segment_id)?.to_cubic();
			let [start_point_id, _] = self.mesh_geometry.points_from_id(first_segment_id)?;
			let [_, end_point_id] = self.mesh_geometry.points_from_id(second_segment_id)?;

			// Each half's control point was shortened by the split that produced it,
			// so scale it back out by the share of the merged parameter range that half covers.
			let merged_handles = {
				let [first_start, first_end] = [first_segment.p0, first_segment.p3].map(point_to_dvec2);
				let [second_start, second_end] = [second_segment.p0, second_segment.p3].map(point_to_dvec2);
				let first_chord = first_start.distance(first_end);
				let second_chord = second_start.distance(second_end);
				let total_chord = first_chord + second_chord;
				let split = if total_chord > 0. { (first_chord / total_chord).clamp(0.1, 0.9) } else { 0.5 };

				let handle_start = first_start + (point_to_dvec2(first_segment.p1) - first_start) / split;
				let handle_end = second_end + (point_to_dvec2(second_segment.p2) - second_end) / (1. - split);
				(Some(handle_start), Some(handle_end))
			};

			let merged_segment_id = self.mesh_geometry.segment_domain.next_id();
			self.mesh_geometry.push(merged_segment_id, start_point_id, end_point_id, merged_handles, StrokeId::ZERO);
			merged_edges.push(merged_segment_id);
			removed_edge_ids.extend([first_segment_id, second_segment_id]);
		}

		for across in 0..across_corner_count - 1 {
			let [row, column] = axis.physical_indices(across, grid_line_index);
			removed_edge_ids.push(*connecting_edge_grid.get(row, column)?);
		}

		self.corner_points.splice_lines(axis, grid_line_index..grid_line_index + 1, &[])?;
		self.corner_colors.splice_lines(axis, grid_line_index..grid_line_index + 1, &[])?;
		let (split_edge_grid, connecting_edge_grid) = axis.edge_grids_mut(&mut self.horizontal_edges, &mut self.vertical_edges);
		split_edge_grid.splice_lines(axis, grid_line_index - 1..grid_line_index + 1, &[&merged_edges])?;
		connecting_edge_grid.splice_lines(axis, grid_line_index..grid_line_index + 1, &[])?;

		let point_count = self.mesh_geometry.point_domain.ids().len();
		self.mesh_geometry.segment_domain.retain(|id| !removed_edge_ids.contains(id), point_count);
		let Vector { point_domain, segment_domain, .. } = &mut self.mesh_geometry;
		point_domain.retain(segment_domain, |id| !removed_corner_ids.contains(id));

		Some(())
	}
}

#[derive(Clone, Copy)]
struct MeshCornerDerivatives {
	u: Vec4,
	v: Vec4,
}

#[derive(Clone, Copy)]
enum MeshPatchInterpolation {
	Stepped,
	Linear,
	Smooth {
		/// Slopes of corner colors for bicubic hermite interpolation. [top-left, top-right, bottom-left, bottom-right]
		color_slopes: [MeshCornerDerivatives; 4],
		/// Linear length of between each corner. [top, bottom, left, right]
		lengths: [f32; 4],
		/// The Bezier restatement of the Hermite color data, built alongside it so the two cannot drift apart.
		bezier_control_points: [[Vec4; 4]; 4],
	},
}

/// A cached mesh patch for subdivision into subpatches in rendering phase.
#[derive(Clone, Copy)]
pub struct MeshPatchEvaluator {
	/// Corner positions. [top-left, top-right, bottom-left, bottom-right]
	pub corners: [DVec2; 4],
	/// Edges defining the patch. [top, bottom, left, right]
	pub edges: [PathSeg; 4],
	/// Color-space channels and straight alpha. [top-left, top-right, bottom-left, bottom-right]
	colors: [Vec4; 4],
	/// Color space used by `colors` and `color_slopes`.
	space: GradientSpace,
	/// Color interpolation method.
	interpolation: MeshPatchInterpolation,
}

impl MeshPatchEvaluator {
	/// Evaluates the raw interpolated color-space channels using the selected interpolation method.
	fn evaluate_channels(&self, u: f32, v: f32) -> [f32; 4] {
		let [top_left_color, top_right_color, bottom_left_color, bottom_right_color] = self.colors;

		match &self.interpolation {
			MeshPatchInterpolation::Stepped => top_left_color.to_array(),
			MeshPatchInterpolation::Linear => {
				let top = top_left_color.lerp(top_right_color, u);
				let bottom = bottom_left_color.lerp(bottom_right_color, u);
				top.lerp(bottom, v).to_array()
			}
			MeshPatchInterpolation::Smooth { color_slopes, lengths, .. } => {
				let hermite = |a: f32, ma: f32, b: f32, mb: f32, t: f32| -> f32 {
					let t_power_2 = t * t;
					let t_power_3 = t_power_2 * t;

					let h1 = 2. * t_power_3 - 3. * t_power_2 + 1.;
					let h2 = -2. * t_power_3 + 3. * t_power_2;
					let h3 = t_power_3 - 2. * t_power_2 + t;
					let h4 = t_power_3 - t_power_2;

					ma * h3 + a * h1 + b * h2 + mb * h4
				};

				let [top_length, bottom_length, left_length, right_length] = lengths;
				let [top_left_color_slope, top_right_color_slope, bottom_left_color_slope, bottom_right_color_slope] = color_slopes;

				std::array::from_fn(|channel| {
					let top_color_interpolated = hermite(
						top_left_color[channel],
						top_left_color_slope.u[channel] * top_length,
						top_right_color[channel],
						top_right_color_slope.u[channel] * top_length,
						u,
					);
					let bottom_color_interpolated = hermite(
						bottom_left_color[channel],
						bottom_left_color_slope.u[channel] * bottom_length,
						bottom_right_color[channel],
						bottom_right_color_slope.u[channel] * bottom_length,
						u,
					);
					let top_slope_interpolated = hermite(top_left_color_slope.v[channel] * left_length, 0., top_right_color_slope.v[channel] * right_length, 0., u);
					let bottom_slope_interpolated = hermite(bottom_left_color_slope.v[channel] * left_length, 0., bottom_right_color_slope.v[channel] * right_length, 0., u);
					hermite(top_color_interpolated, top_slope_interpolated, bottom_color_interpolated, bottom_slope_interpolated, v)
				})
			}
		}
	}

	/// Evaluates the interpolated color and returns gamma-sRGB channels for rendering.
	pub fn evaluate_color(&self, u: f32, v: f32) -> [f32; 4] {
		let channels = self.evaluate_channels(u, v);
		if self.space == GradientSpace::RgbGamma {
			channels
		} else {
			color_from_gradient_space_channels(channels, self.space).to_gamma_srgb_channels()
		}
	}

	/// Evaluates the interpolated position using a bilinearly blended Coons patch.
	pub fn evaluate_position(&self, u: f64, v: f64) -> DVec2 {
		let [top_seg, bottom_seg, left_seg, right_seg] = self.edges;
		let [top_left, top_right, bottom_left, bottom_right] = self.corners;

		let top_u_pos = point_to_dvec2(top_seg.eval(u));
		let bottom_u_pos = point_to_dvec2(bottom_seg.eval(u));
		let left_v_pos = point_to_dvec2(left_seg.eval(v));
		let right_v_pos = point_to_dvec2(right_seg.eval(v));

		let s_c = (1. - v) * top_u_pos + v * bottom_u_pos;
		let s_d = (1. - u) * left_v_pos + u * right_v_pos;
		let s_b = top_left * (1. - u) * (1. - v) + top_right * u * (1. - v) + bottom_left * (1. - u) * v + bottom_right * u * v;

		s_c + s_d - s_b
	}

	/// Returns [0,1] approximated uv by calculating the inverse of the bilinearly-blended Coons patch using Newton's method.
	pub fn inverse_patch_position(&self, target_position: DVec2, initial_uv: DVec2) -> DVec2 {
		let (uv, _) = self.inverse_patch_position_impl(target_position, initial_uv);
		uv.clamp(DVec2::ZERO, DVec2::ONE)
	}

	/// Returns the unbounded UV when Newton's method converges, allowing neighboring positions to continue the same inverse branch.
	pub fn try_inverse_patch_position(&self, target_position: DVec2, initial_uv: DVec2) -> Option<DVec2> {
		let (uv, converged) = self.inverse_patch_position_impl(target_position, initial_uv);
		converged.then_some(uv)
	}

	fn inverse_patch_position_impl(&self, target_position: DVec2, initial_uv: DVec2) -> (DVec2, bool) {
		const MAX_ITERATION: usize = 16;
		const POSITION_TOLERANCE: f64 = 1e-6;
		const JACOBIAN_EPSILON: f64 = 1e-12;
		const LINE_SEARCH_STEPS: usize = 8;

		let mut uv = initial_uv;

		for _ in 0..MAX_ITERATION {
			let DVec2 { x: u, y: v } = uv;
			// Check if the current uv position is already within the tolerance
			let position = self.evaluate_position(u, v);
			let error = position - target_position;
			let error_squared = error.length_squared();

			if !error_squared.is_finite() {
				break;
			}

			if error_squared <= POSITION_TOLERANCE * POSITION_TOLERANCE {
				return (uv, true);
			}

			// If not, calculate the next uv by subtracting the inverse Jacobian multiplied by the error
			let jacobian = position_jacobian(self.corners, self.edges, u, v);
			let determinant = jacobian.determinant();
			if !determinant.is_finite() || determinant.abs() <= JACOBIAN_EPSILON {
				break;
			}

			let delta = jacobian.inverse() * error;
			if !delta.is_finite() {
				break;
			}

			// Try progressively smaller Newton steps until the error decreases
			let mut step = 1.;
			let mut next_uv = None;
			for _ in 0..LINE_SEARCH_STEPS {
				let candidate = uv - delta * step;
				let candidate_error_squared = self.evaluate_position(candidate.x, candidate.y).distance_squared(target_position);

				if candidate_error_squared.is_finite() && candidate_error_squared < error_squared {
					next_uv = Some(candidate);
					break;
				}

				step *= 0.5;
			}

			let Some(next_uv) = next_uv else {
				break;
			};
			uv = next_uv;
		}

		let error_squared = self.evaluate_position(uv.x, uv.y).distance_squared(target_position);
		(uv, error_squared.is_finite() && error_squared <= POSITION_TOLERANCE * POSITION_TOLERANCE)
	}

	/// Evaluates one horizontal Bezier control row of a smooth patch.
	pub fn evaluate_bicubic_bezier_row(&self, row: usize, u: f32) -> Option<Vec4> {
		let MeshPatchInterpolation::Smooth { bezier_control_points, .. } = &self.interpolation else {
			return None;
		};
		let [a, b, c, d] = bezier_control_points[row];
		let one_minus_u = 1. - u;
		Some(a * one_minus_u.powi(3) + b * (3. * u * one_minus_u.powi(2)) + c * (3. * u.powi(2) * one_minus_u) + d * u.powi(3))
	}
}

/// Restates a patch's Hermite color data as the control net of the equivalent bicubic Bezier surface.
fn bicubic_bezier_control_net(colors: &[Vec4; 4], color_slopes: &[MeshCornerDerivatives; 4], lengths: &[f32; 4]) -> [[Vec4; 4]; 4] {
	let [top_length, bottom_length, left_length, right_length] = *lengths;
	let [top_left_color, top_right_color, bottom_left_color, bottom_right_color] = *colors;
	let [top_left_color_slope, top_right_color_slope, bottom_left_color_slope, bottom_right_color_slope] = *color_slopes;

	let hermite_channels: [Mat4; 4] = std::array::from_fn(|channel| {
		Mat4::from_cols(
			Vec4::new(
				top_left_color[channel],
				top_left_color_slope.v[channel] * left_length,
				bottom_left_color[channel],
				bottom_left_color_slope.v[channel] * left_length,
			),
			Vec4::new(top_left_color_slope.u[channel] * top_length, 0., bottom_left_color_slope.u[channel] * bottom_length, 0.),
			Vec4::new(
				top_right_color[channel],
				top_right_color_slope.v[channel] * right_length,
				bottom_right_color[channel],
				bottom_right_color_slope.v[channel] * right_length,
			),
			Vec4::new(top_right_color_slope.u[channel] * top_length, 0., bottom_right_color_slope.u[channel] * bottom_length, 0.),
		)
	});

	let hermite_to_bezier_axis = Mat4::from_cols(Vec4::new(1., 1., 0., 0.), Vec4::new(0., 1. / 3., 0., 0.), Vec4::new(0., 0., 1., 1.), Vec4::new(0., 0., -1. / 3., 0.));
	let hermite_to_bezier_axis_transpose = hermite_to_bezier_axis.transpose();

	let points_mat = hermite_channels.map(|hermite| hermite_to_bezier_axis * hermite * hermite_to_bezier_axis_transpose);

	std::array::from_fn(|v| std::array::from_fn(|u| Vec4::new(points_mat[0].col(u)[v], points_mat[1].col(u)[v], points_mat[2].col(u)[v], points_mat[3].col(u)[v])))
}

/// Struct for evaluating color for subpatch corners.
/// The main purpose is to prevent duplicated calculation of the slopes for hermite interpolation for each subpatch.
#[derive(Clone)]
pub struct MeshGradientEvaluator {
	/// List of required data for color interpolation, row major order.
	patches: Vec<MeshPatchEvaluator>,
	space: GradientSpace,
	interpolation: GradientInterpolation,
}

impl MeshGradientEvaluator {
	pub fn new(mesh_gradient: &MeshGradient, space: GradientSpace, interpolation: GradientInterpolation) -> Option<Self> {
		let [corner_rows, corner_columns] = mesh_gradient.corner_points.dimensions();
		if corner_rows < 2 || corner_columns < 2 {
			return None;
		}
		let patch_columns = corner_columns - 1;
		let patch_rows = corner_rows - 1;

		if mesh_gradient.corner_colors.dimensions() != [corner_rows, corner_columns]
			|| mesh_gradient.horizontal_edges.dimensions() != [corner_rows, patch_columns]
			|| mesh_gradient.vertical_edges.dimensions() != [patch_rows, corner_columns]
		{
			return None;
		}

		let corner_positions: Vec<DVec2> = mesh_gradient
			.corner_points
			.values
			.iter()
			.map(|&point_id| mesh_gradient.mesh_geometry.point_domain.position_from_id(point_id))
			.collect::<Option<_>>()?;

		let colors: Vec<Vec4> = mesh_gradient
			.corner_colors
			.values
			.iter()
			.map(|&color| Vec4::from_array(gradient_space_channels(color, space)))
			.collect();

		// Calculate the slope of the `curr_index` corner by FDM. The slope is derived from the linear distance from the previous/next corners.
		let calculate_color_slope = |prev_index: usize, curr_index: usize, next_index: usize| {
			let prev_color = colors[prev_index];
			let curr_color = colors[curr_index];
			let next_color = colors[next_index];

			let [prev_pos, curr_pos, next_pos] = [prev_index, curr_index, next_index].map(|index| corner_positions[index]);
			let prev_distance = curr_pos.distance(prev_pos) as f32;
			let next_distance = next_pos.distance(curr_pos) as f32;

			let backward_diff = (prev_distance > f32::EPSILON).then(|| (curr_color - prev_color) / prev_distance);
			let forward_diff = (next_distance > f32::EPSILON).then(|| (next_color - curr_color) / next_distance);

			match (backward_diff, forward_diff) {
				(Some(backward), Some(forward)) => {
					let central = (backward + forward) / 2.;

					// Prevent overshooting by using a zero slope at a local extremum.
					Vec4::from_array(std::array::from_fn(|channel| if backward[channel] * forward[channel] <= 0. { 0. } else { central[channel] }))
				}
				(Some(backward), None) => backward,
				(None, Some(forward)) => forward,
				(None, None) => Vec4::ZERO,
			}
		};

		let sample_index = |row: isize, column: isize| -> usize {
			let clamped_column = column.clamp(0, corner_columns as isize - 1) as usize;
			let clamped_row = row.clamp(0, corner_rows as isize - 1) as usize;
			clamped_row * corner_columns + clamped_column
		};

		let corner_slopes = (interpolation == GradientInterpolation::Smooth).then(|| {
			let mut slopes = Vec::with_capacity(corner_rows * corner_columns);
			for row in 0..corner_rows as isize {
				for col in 0..corner_columns as isize {
					let curr_index = sample_index(row, col);
					let u = calculate_color_slope(sample_index(row, col - 1), curr_index, sample_index(row, col + 1));
					let v = calculate_color_slope(sample_index(row - 1, col), curr_index, sample_index(row + 1, col));
					slopes.push(MeshCornerDerivatives { u, v });
				}
			}
			slopes
		});

		let mut patch_color_data = Vec::with_capacity(patch_rows.checked_mul(patch_columns)?);
		for row in 0..patch_rows {
			for column in 0..patch_columns {
				let patch = mesh_gradient.patch(row, column)?;
				let top_left_index = row * corner_columns + column;
				let corner_indices = [top_left_index, top_left_index + 1, top_left_index + corner_columns, top_left_index + corner_columns + 1];
				let patch_colors = corner_indices.map(|index| colors[index]);

				let [top_left_pos, top_right_pos, bottom_left_pos, bottom_right_pos] = patch.corners;

				let interpolation = match interpolation {
					GradientInterpolation::Stepped => MeshPatchInterpolation::Stepped,
					GradientInterpolation::Linear => MeshPatchInterpolation::Linear,
					GradientInterpolation::Smooth => {
						let corner_slopes = corner_slopes.as_ref().expect("Smooth interpolation must have color slopes");
						let color_slopes = corner_indices.map(|index| corner_slopes[index]);
						let lengths = [
							top_left_pos.distance(top_right_pos) as f32,
							bottom_left_pos.distance(bottom_right_pos) as f32,
							top_left_pos.distance(bottom_left_pos) as f32,
							top_right_pos.distance(bottom_right_pos) as f32,
						];
						let bezier_control_points = bicubic_bezier_control_net(&patch_colors, &color_slopes, &lengths);
						MeshPatchInterpolation::Smooth {
							color_slopes,
							lengths,
							bezier_control_points,
						}
					}
				};

				patch_color_data.push(MeshPatchEvaluator {
					corners: patch.corners,
					edges: patch.edges,
					colors: patch_colors,
					space,
					interpolation,
				});
			}
		}

		Some(Self {
			patches: patch_color_data,
			space,
			interpolation,
		})
	}

	pub fn interpolation_method(&self) -> GradientInterpolation {
		self.interpolation
	}

	pub fn space(&self) -> GradientSpace {
		self.space
	}

	fn evaluate_color(&self, patch_index: usize, u: f32, v: f32) -> [f32; 4] {
		self.patches[patch_index].evaluate_color(u, v)
	}

	/// Returns the cached evaluators in row-major patch order.
	pub fn patch_evaluators(&self) -> impl Iterator<Item = &MeshPatchEvaluator> {
		self.patches.iter()
	}

	pub fn patch_evaluator(&self, patch_index: usize) -> Option<&MeshPatchEvaluator> {
		self.patches.get(patch_index)
	}
}

impl RenderComplexity for MeshGradient {
	fn render_complexity(&self) -> usize {
		usize::MAX
	}
}

impl core_types::bounds::BoundingBox for MeshGradient {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		core_types::bounds::BoundingBox::bounding_box(&self.mesh_geometry, transform, include_stroke)
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		core_types::bounds::BoundingBox::thumbnail_bounding_box(&self.mesh_geometry, transform, include_stroke)
	}
}

/// Helper to create initial handles.
fn line_to_cubic_bezier_handles(start: DVec2, end: DVec2) -> (Option<DVec2>, Option<DVec2>) {
	(Some(start + (end - start) / 3.), Some(end + (start - end) / 3.))
}

/// Returns Jacobian matrix of the UV position in a single Coons patch.
fn position_jacobian(corners: [DVec2; 4], edges: [PathSeg; 4], u: f64, v: f64) -> DMat2 {
	let [top, bottom, left, right] = edges;
	let [top_left, top_right, bottom_left, bottom_right] = corners;

	let top_u_pos = point_to_dvec2(top.eval(u));
	let bottom_u_pos = point_to_dvec2(bottom.eval(u));
	let left_v_pos = point_to_dvec2(left.eval(v));
	let right_v_pos = point_to_dvec2(right.eval(v));

	let top_bottom_derivative_u = (1. - v) * pathseg_tangent(top, u) + v * pathseg_tangent(bottom, u);
	let left_right_derivative_u = right_v_pos - left_v_pos;
	let top_bottom_derivative_v = bottom_u_pos - top_u_pos;
	let left_right_derivative_v = (1. - u) * pathseg_tangent(left, v) + u * pathseg_tangent(right, v);

	let bilinear_derivative_u = (1. - v) * (top_right - top_left) + v * (bottom_right - bottom_left);
	let bilinear_derivative_v = (1. - u) * (bottom_left - top_left) + u * (bottom_right - top_right);

	let derivative_u = top_bottom_derivative_u + left_right_derivative_u - bilinear_derivative_u;
	let derivative_v = top_bottom_derivative_v + left_right_derivative_v - bilinear_derivative_v;

	DMat2::from_cols(derivative_u, derivative_v)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn assert_position(actual: DVec2, expected: DVec2) {
		assert!((actual - expected).length() < 1e-10, "expected {expected:?}, got {actual:?}");
	}

	fn point(position: DVec2) -> kurbo::Point {
		kurbo::Point::new(position.x, position.y)
	}

	fn line_edges([top_left, top_right, bottom_left, bottom_right]: [DVec2; 4]) -> [PathSeg; 4] {
		[
			PathSeg::Line(kurbo::Line::new(point(top_left), point(top_right))),
			PathSeg::Line(kurbo::Line::new(point(bottom_left), point(bottom_right))),
			PathSeg::Line(kurbo::Line::new(point(top_left), point(bottom_left))),
			PathSeg::Line(kurbo::Line::new(point(top_right), point(bottom_right))),
		]
	}

	fn patch_evaluator(corners: [DVec2; 4], edges: [PathSeg; 4]) -> MeshPatchEvaluator {
		MeshPatchEvaluator {
			corners,
			edges,
			colors: [Vec4::ZERO; 4],
			space: GradientSpace::RgbGamma,
			interpolation: MeshPatchInterpolation::Linear,
		}
	}

	fn mesh_with_corner_colors(mut color: impl FnMut(usize) -> Color) -> MeshGradient {
		let mut mesh = MeshGradient::default();
		for corner_index in 0..mesh.size() {
			mesh.set_corner_color(corner_index, color(corner_index)).unwrap();
		}
		mesh
	}

	fn single_patch_mesh(colors: [Color; 4]) -> MeshGradient {
		let positions = [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE];
		let mut mesh = MeshGradient::from_positions(&positions, 2, 2).unwrap();
		for (corner_index, color) in colors.into_iter().enumerate() {
			mesh.set_corner_color(corner_index, color).unwrap();
		}
		mesh
	}

	fn curved_patch_evaluator() -> MeshPatchEvaluator {
		let corners = [DVec2::new(0., 0.), DVec2::new(2., 0.), DVec2::new(0., 2.), DVec2::new(2., 2.)];
		let [top_left, top_right, bottom_left, bottom_right] = corners.map(point);
		let edges = [
			PathSeg::Cubic(kurbo::CubicBez::new(top_left, kurbo::Point::new(0.5, -0.5), kurbo::Point::new(1.5, 0.5), top_right)),
			PathSeg::Cubic(kurbo::CubicBez::new(bottom_left, kurbo::Point::new(0.5, 2.5), kurbo::Point::new(1.5, 1.5), bottom_right)),
			PathSeg::Cubic(kurbo::CubicBez::new(top_left, kurbo::Point::new(-0.4, 0.5), kurbo::Point::new(0.4, 1.5), bottom_left)),
			PathSeg::Cubic(kurbo::CubicBez::new(top_right, kurbo::Point::new(2.4, 0.5), kurbo::Point::new(1.6, 1.5), bottom_right)),
		];
		patch_evaluator(corners, edges)
	}

	#[test]
	fn evaluate_color_reproduces_an_affine_color_field() {
		let base = Vec4::new(0.1, 0.2, 0.3, 0.4);
		let u_delta = Vec4::new(0.2, 0.1, -0.1, 0.2);
		let v_delta = Vec4::new(0.3, -0.1, 0.2, 0.1);
		let colors = [base, base + u_delta, base + v_delta, base + u_delta + v_delta];
		let color_slopes = [MeshCornerDerivatives { u: u_delta, v: v_delta }; 4];
		let lengths = [1.; 4];
		let evaluator = MeshPatchEvaluator {
			corners: [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE],
			edges: line_edges([DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE]),
			colors,
			space: GradientSpace::RgbGamma,
			interpolation: MeshPatchInterpolation::Smooth {
				color_slopes,
				lengths,
				bezier_control_points: bicubic_bezier_control_net(&colors, &color_slopes, &lengths),
			},
		};

		for [u, v] in [[0., 0.], [0.37, 0.61], [1., 1.]] {
			let actual = Vec4::from_array(evaluator.evaluate_color(u, v));
			let expected = base + u_delta * u + v_delta * v;
			assert!((actual - expected).abs().max_element() < 1e-6, "expected {expected:?}, got {actual:?}");
		}
	}

	#[test]
	fn stepped_interpolation_uses_the_top_left_patch_color() {
		let colors = [Color::BLACK, Color::WHITE, Color::BLUE, Color::YELLOW];
		let evaluator = single_patch_mesh(colors).evaluator(GradientSpace::RgbGamma, GradientInterpolation::Stepped).unwrap();
		let patch = evaluator.patch_evaluator(0).unwrap();
		let expected = Vec4::from_array(colors[0].to_gamma_srgb_channels());

		for [u, v] in [[0., 0.], [0.25, 0.75], [1., 1.]] {
			let actual = Vec4::from_array(patch.evaluate_color(u, v));
			assert!((actual - expected).abs().max_element() < 1e-6, "expected {expected:?}, got {actual:?} at ({u}, {v})");
		}
	}

	#[test]
	fn linear_interpolation_bilinearly_blends_the_patch_colors() {
		let colors = [Color::BLACK, Color::WHITE, Color::BLUE, Color::YELLOW];
		let evaluator = single_patch_mesh(colors).evaluator(GradientSpace::RgbGamma, GradientInterpolation::Linear).unwrap();
		let patch = evaluator.patch_evaluator(0).unwrap();
		let [top_left, top_right, bottom_left, bottom_right] = colors.map(|color| Vec4::from_array(color.to_gamma_srgb_channels()));

		for [u, v] in [[0., 0.], [0.25, 0.75], [1., 1.]] {
			let expected = top_left.lerp(top_right, u).lerp(bottom_left.lerp(bottom_right, u), v);
			let actual = Vec4::from_array(patch.evaluate_color(u, v));
			assert!((actual - expected).abs().max_element() < 1e-6, "expected {expected:?}, got {actual:?} at ({u}, {v})");
		}
	}

	#[test]
	fn linear_oklab_interpolation_bilinearly_blends_oklab_channels() {
		let colors = [Color::BLACK, Color::WHITE, Color::BLUE, Color::YELLOW];
		let evaluator = single_patch_mesh(colors).evaluator(GradientSpace::OkLab, GradientInterpolation::Linear).unwrap();
		let patch = evaluator.patch_evaluator(0).unwrap();
		let [top_left, top_right, bottom_left, bottom_right] = colors.map(|color| Vec4::from_array(gradient_space_channels(color, GradientSpace::OkLab)));

		for [u, v] in [[0., 0.], [0.25, 0.75], [1., 1.]] {
			let oklab = top_left.lerp(top_right, u).lerp(bottom_left.lerp(bottom_right, u), v);
			let expected = Vec4::from_array(color_from_gradient_space_channels(oklab.to_array(), GradientSpace::OkLab).to_gamma_srgb_channels());
			let actual = Vec4::from_array(patch.evaluate_color(u, v));
			assert!((actual - expected).abs().max_element() < 1e-6, "expected {expected:?}, got {actual:?} at ({u}, {v})");
		}
	}

	#[test]
	fn rectangular_spaces_keep_their_own_channels() {
		let colors = [Color::BLACK, Color::WHITE, Color::from_rgbf32_unchecked(0.85, 0.05, 0.4)];
		let mesh = mesh_with_corner_colors(|corner_index| colors[corner_index % colors.len()]);

		for space in [GradientSpace::RgbGamma, GradientSpace::RgbLinear, GradientSpace::OkLab, GradientSpace::Lab] {
			let evaluator = mesh.evaluator(space, GradientInterpolation::Smooth).unwrap();
			let patch = evaluator.patch_evaluator(0).unwrap();
			let expected = Vec4::from_array(gradient_space_channels(colors[0], space));

			assert!((patch.colors[0] - expected).abs().max_element() < 1e-6, "{space:?} must store its corner channels untouched");
		}
	}

	#[test]
	fn evaluate_position_reproduces_patch_boundaries() {
		let evaluator = curved_patch_evaluator();

		for t in [0., 0.25, 0.5, 0.75, 1.] {
			assert_position(evaluator.evaluate_position(t, 0.), point_to_dvec2(evaluator.edges[0].eval(t)));
			assert_position(evaluator.evaluate_position(t, 1.), point_to_dvec2(evaluator.edges[1].eval(t)));
			assert_position(evaluator.evaluate_position(0., t), point_to_dvec2(evaluator.edges[2].eval(t)));
			assert_position(evaluator.evaluate_position(1., t), point_to_dvec2(evaluator.edges[3].eval(t)));
		}
	}

	#[test]
	fn position_jacobian_matches_affine_patch() {
		let transform = DAffine2::from_cols(DVec2::new(3., 0.5), DVec2::new(-0.25, 2.), DVec2::new(4., -3.));
		let corners = [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE].map(|corner| transform.transform_point2(corner));
		let edges = line_edges(corners);

		for [u, v] in [[0., 0.], [0.25, 0.75], [0.5, 0.5], [1., 1.]] {
			let jacobian = position_jacobian(corners, edges, u, v);
			assert_position(jacobian.x_axis, transform.matrix2.x_axis);
			assert_position(jacobian.y_axis, transform.matrix2.y_axis);
		}
	}

	#[test]
	fn bounding_box_uses_mesh_geometry() {
		let bounds = core_types::bounds::BoundingBox::bounding_box(&MeshGradient::default(), DAffine2::IDENTITY, false);
		assert_eq!(bounds, core_types::bounds::RenderBoundingBox::Rectangle([DVec2::ZERO, DVec2::ONE]));
	}

	#[test]
	fn position_jacobian_matches_numerical_derivative_for_curved_patch() {
		let evaluator = curved_patch_evaluator();
		let (u, v, step) = (0.37, 0.61, 1e-6);

		let numerical_u = (evaluator.evaluate_position(u + step, v) - evaluator.evaluate_position(u - step, v)) / (2. * step);
		let numerical_v = (evaluator.evaluate_position(u, v + step) - evaluator.evaluate_position(u, v - step)) / (2. * step);
		let jacobian = position_jacobian(evaluator.corners, evaluator.edges, u, v);

		assert!((jacobian.x_axis - numerical_u).length() < 1e-8, "expected {:?}, got {:?}", numerical_u, jacobian.x_axis);
		assert!((jacobian.y_axis - numerical_v).length() < 1e-8, "expected {:?}, got {:?}", numerical_v, jacobian.y_axis);
	}

	#[test]
	fn inverse_patch_position_recovers_curved_patch_uv() {
		let evaluator = curved_patch_evaluator();
		let expected = DVec2::new(0.37, 0.61);
		let target = evaluator.evaluate_position(expected.x, expected.y);
		let actual = evaluator.inverse_patch_position(target, DVec2::splat(0.5));

		assert!((actual - expected).length() < 1e-6, "expected {expected:?}, got {actual:?}");
	}

	#[test]
	fn inverse_patch_position_clamps_to_patch_uv_bounds() {
		let corners = [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE];
		let evaluator = patch_evaluator(corners, line_edges(corners));
		let actual = evaluator.inverse_patch_position(DVec2::new(1.5, 0.4), DVec2::splat(0.5));

		assert_position(actual, DVec2::new(1., 0.4));
	}

	#[test]
	fn try_inverse_patch_position_returns_unbounded_uv() {
		let corners = [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE];
		let evaluator = patch_evaluator(corners, line_edges(corners));
		let actual = evaluator.try_inverse_patch_position(DVec2::new(1.5, 0.4), DVec2::splat(0.5)).unwrap();

		assert_position(actual, DVec2::new(1.5, 0.4));
	}

	#[test]
	fn try_inverse_patch_position_reports_singular_patch() {
		let corners = [DVec2::ZERO; 4];
		let evaluator = patch_evaluator(corners, line_edges(corners));

		assert!(evaluator.try_inverse_patch_position(DVec2::ONE, DVec2::splat(0.5)).is_none());
	}

	#[test]
	fn inserting_mesh_grid_lines_preserves_row_major_topology() {
		let mut mesh = MeshGradient::default();
		let top_edge = *mesh.horizontal_edges.get(0, 0).unwrap();
		mesh.insert_grid_line(top_edge, GradientSpace::RgbGamma, GradientInterpolation::Smooth, 0.25).unwrap();

		assert_eq!(mesh.corner_points.dimensions(), [3, 4]);
		assert_eq!(mesh.horizontal_edges.dimensions(), [3, 3]);
		assert_eq!(mesh.vertical_edges.dimensions(), [2, 4]);
		let expected_x = [0., 0.125, 0.5, 1.];
		for row in 0..mesh.corner_points.rows {
			for (column, &x) in expected_x.iter().enumerate() {
				let position = mesh.mesh_geometry.point_domain.position_from_id(*mesh.corner_points.get(row, column).unwrap()).unwrap();
				assert_position(position, DVec2::new(x, row as f64 / 2.));
			}
		}

		let left_edge = *mesh.vertical_edges.get(0, 0).unwrap();
		mesh.insert_grid_line(left_edge, GradientSpace::RgbGamma, GradientInterpolation::Smooth, 0.5).unwrap();

		assert_eq!(mesh.corner_points.dimensions(), [4, 4]);
		assert_eq!(mesh.horizontal_edges.dimensions(), [4, 3]);
		assert_eq!(mesh.vertical_edges.dimensions(), [3, 4]);
		let expected_y = [0., 0.25, 0.5, 1.];
		for (row, &y) in expected_y.iter().enumerate() {
			for (column, &x) in expected_x.iter().enumerate() {
				let position = mesh.mesh_geometry.point_domain.position_from_id(*mesh.corner_points.get(row, column).unwrap()).unwrap();
				assert_position(position, DVec2::new(x, y));
			}
		}

		for row in 0..mesh.corner_points.rows - 1 {
			for column in 0..mesh.corner_points.columns - 1 {
				let patch = mesh.patch(row, column).unwrap();
				assert_position(patch.corners[0], DVec2::new(expected_x[column], expected_y[row]));
				assert_position(patch.corners[3], DVec2::new(expected_x[column + 1], expected_y[row + 1]));
			}
		}
	}

	#[test]
	fn removing_mesh_edges_removes_their_interior_grid_lines() {
		let mut mesh = MeshGradient::default();
		let expected_positions: Vec<_> = mesh.corners().map(|corner| corner.position).collect();
		let expected_colors: Vec<_> = mesh.corners().map(|corner| corner.color).collect();

		let top_edge = *mesh.horizontal_edges.get(0, 0).unwrap();
		mesh.insert_grid_line(top_edge, GradientSpace::RgbGamma, GradientInterpolation::Smooth, 0.25).unwrap();
		let inserted_vertical_edge = *mesh.vertical_edges.get(0, 1).unwrap();
		mesh.remove_edge(inserted_vertical_edge).unwrap();

		assert_eq!(mesh.corner_points.dimensions(), [3, 3]);
		assert_eq!(mesh.horizontal_edges.dimensions(), [3, 2]);
		assert_eq!(mesh.vertical_edges.dimensions(), [2, 3]);
		assert_eq!(mesh.corners().map(|corner| corner.position).collect::<Vec<_>>(), expected_positions);
		assert_eq!(mesh.corners().map(|corner| corner.color).collect::<Vec<_>>(), expected_colors);

		let left_edge = *mesh.vertical_edges.get(0, 0).unwrap();
		mesh.insert_grid_line(left_edge, GradientSpace::RgbGamma, GradientInterpolation::Smooth, 0.5).unwrap();
		let inserted_horizontal_edge = *mesh.horizontal_edges.get(1, 0).unwrap();
		mesh.remove_edge(inserted_horizontal_edge).unwrap();

		assert_eq!(mesh.corner_points.dimensions(), [3, 3]);
		assert_eq!(mesh.horizontal_edges.dimensions(), [3, 2]);
		assert_eq!(mesh.vertical_edges.dimensions(), [2, 3]);
		assert_eq!(mesh.corners().map(|corner| corner.position).collect::<Vec<_>>(), expected_positions);
		assert_eq!(mesh.corners().map(|corner| corner.color).collect::<Vec<_>>(), expected_colors);
		assert_eq!(mesh.patches().collect::<Option<Vec<_>>>().unwrap().len(), 4);

		let boundary_edge = *mesh.horizontal_edges.get(0, 0).unwrap();
		assert_eq!(mesh.remove_edge(boundary_edge), None);
	}

	#[test]
	fn removing_an_inserted_grid_line_restores_the_edge_curve() {
		let mut mesh = MeshGradient::default();
		let edge = *mesh.horizontal_edges.get(0, 0).unwrap();
		// Symmetric about the edge's midpoint, so an even split leaves the two halves with equal chords
		mesh.set_edge_handles(
			edge,
			BezierHandles::Cubic {
				handle_start: DVec2::new(0.125, 0.2),
				handle_end: DVec2::new(0.375, 0.2),
			},
		)
		.unwrap();
		let before = mesh.mesh_geometry.path_segment_from_id(edge).unwrap().to_cubic();

		mesh.insert_grid_line(edge, GradientSpace::RgbGamma, GradientInterpolation::Smooth, 0.5).unwrap();
		let inserted = *mesh.vertical_edges.get(0, 1).unwrap();
		mesh.remove_edge(inserted).unwrap();

		let merged = mesh.mesh_geometry.path_segment_from_id(*mesh.horizontal_edges.get(0, 0).unwrap()).unwrap().to_cubic();
		for (actual, expected) in [(merged.p0, before.p0), (merged.p1, before.p1), (merged.p2, before.p2), (merged.p3, before.p3)] {
			assert_position(point_to_dvec2(actual), point_to_dvec2(expected));
		}
	}
}
