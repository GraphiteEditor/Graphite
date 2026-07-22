use core_types::{Color, render_complexity::RenderComplexity};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2, Vec4};
use kurbo::{ParamCurve, PathSeg};

use crate::{
	Vector,
	subpath::{BezierHandles, pathseg_points},
	vector::{
		PointId, SegmentId, StrokeId,
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
	/// Corner positions. [top-left, top-right, bottom-left, bottom-right]
	pub corners: [DVec2; 4],
	/// Corner colors. [top-left, top-right, bottom-left, bottom-right]
	pub colors: [Color; 4],
	/// Edges defining the patch. [top, bottom, left, right]
	pub edges: [PathSeg; 4],
}

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
				vector.push(
					segment_id,
					corner_points[start_index],
					corner_points[end_index],
					handles(positions[start_index], positions[end_index]),
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

				let segment_id = vector.segment_domain.next_id();
				vector.push(
					segment_id,
					corner_points[start_index],
					corner_points[end_index],
					handles(positions[start_index], positions[end_index]),
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
			mesh_geometry: vector,
			corner_points: MeshGrid::new(corner_points, corner_rows, corner_columns)?,
			corner_colors: MeshGrid::new(corner_colors, corner_rows, corner_columns)?,
			horizontal_edges: MeshGrid::new(horizontal_edges, corner_rows, corner_columns - 1)?,
			vertical_edges: MeshGrid::new(vertical_edges, corner_rows - 1, corner_columns)?,
		})
	}

	/// Returns resolved patch by the provided row/column position, if any.
	fn patch(&self, row: usize, column: usize) -> Option<MeshPatch> {
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

		Some(MeshPatch { corners, colors, edges })
	}

	/// Iterator over all of the mesh gradient patches by row-major order, `None` if the patch is defined in unexpected structure.
	pub fn patches(&self) -> impl Iterator<Item = Option<MeshPatch>> + '_ {
		let patch_rows = self.corner_points.rows.saturating_sub(1);
		let patch_columns = self.corner_points.columns.saturating_sub(1);
		(0..patch_rows).flat_map(move |row| (0..patch_columns).map(move |column| self.patch(row, column)))
	}

	/// Returns a new `MeshGradientEvaluator`.
	pub fn evaluator(&self) -> Option<MeshGradientEvaluator> {
		MeshGradientEvaluator::new(self)
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

	/// Set the corner position. The corresponding handles are also moved same amount.
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

	/// Inserts a new grid line through the provided segment at the given parameter.
	pub fn insert_grid_line(&mut self, segment_id: SegmentId, t: f64) -> Option<()> {
		#[derive(Clone, Copy)]
		struct SplitSource {
			segment_id: SegmentId,
			start_point_id: PointId,
			end_point_id: PointId,
			segment: PathSeg,
		}

		let (axis, split_patch_index) = self.grid_line_axis(segment_id)?;
		let grid_line_insertion_index = split_patch_index + 1;
		let evaluator = self.evaluator()?;
		let (split_edge_grid, _) = axis.edge_grids(&self.horizontal_edges, &self.vertical_edges);
		let [across_corner_count, _] = axis.logical_indices(split_edge_grid.rows, split_edge_grid.columns);
		let across_patch_count = across_corner_count - 1;
		let patch_columns = self.corner_points.columns - 1;

		// Collect the existing segments that will be split by inserting new corners
		let split_sources: Vec<SplitSource> = (0..across_corner_count)
			.map(|across| {
				let [edge_row, edge_column] = axis.physical_indices(across, split_patch_index);
				let segment_id = *split_edge_grid.get(edge_row, edge_column)?;
				let [start_point_id, end_point_id] = self.mesh_geometry.points_from_id(segment_id)?;
				let segment = self.mesh_geometry.path_segment_from_id(segment_id)?;
				Some(SplitSource {
					segment_id,
					start_point_id,
					end_point_id,
					segment,
				})
			})
			.collect::<Option<_>>()?;

		// Calculate the new corners' information
		let inserted_positions: Vec<DVec2> = split_sources.iter().map(|source| point_to_dvec2(source.segment.eval(t))).collect();
		let inserted_colors: Vec<Color> = (0..across_corner_count)
			.map(|across| {
				let (patch_across, across_t) = if across < across_patch_count { (across, 0.) } else { (across - 1, 1.) };
				let [patch_row, patch_column] = axis.physical_indices(patch_across, split_patch_index);
				let patch_index = patch_row * patch_columns + patch_column;
				let [u, v] = axis.uv(t as f32, across_t);
				let [r, g, b, a] = evaluator.eval_color(patch_index, u, v);
				Color::from_gamma_srgb_channels(r, g, b, a)
			})
			.collect();

		let mut inserted_corners = Vec::with_capacity(across_corner_count);
		for &position in &inserted_positions {
			let point_id = self.mesh_geometry.point_domain.next_id();
			self.mesh_geometry.point_domain.push(point_id, position);
			inserted_corners.push(point_id);
		}

		// Split the existing segments by the new corners
		let mut first_split_edges = Vec::with_capacity(across_corner_count);
		let mut second_split_edges = Vec::with_capacity(across_corner_count);
		for (source, &inserted_corner) in split_sources.iter().zip(&inserted_corners) {
			let first_half = pathseg_points(source.segment.subsegment(0. ..t));
			let second_half = pathseg_points(source.segment.subsegment(t..1.));

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
		for (corner_pair, position_pair) in inserted_corners.windows(2).zip(inserted_positions.windows(2)) {
			let &[start, end] = corner_pair else { unreachable!() };
			let &[start_position, end_position] = position_pair else { unreachable!() };
			let connecting_segment_id = self.mesh_geometry.segment_domain.next_id();
			self.mesh_geometry.push(connecting_segment_id, start, end, handles(start_position, end_position), StrokeId::ZERO);
			connecting_edges.push(connecting_segment_id);
		}

		self.corner_points.splice_lines(axis, grid_line_insertion_index..grid_line_insertion_index, &[&inserted_corners])?;
		self.corner_colors.splice_lines(axis, grid_line_insertion_index..grid_line_insertion_index, &[&inserted_colors])?;
		let (split_edge_grid, connecting_edge_grid) = axis.edge_grids_mut(&mut self.horizontal_edges, &mut self.vertical_edges);
		split_edge_grid.splice_lines(axis, split_patch_index..grid_line_insertion_index, &[&first_split_edges, &second_split_edges])?;
		connecting_edge_grid.splice_lines(axis, grid_line_insertion_index..grid_line_insertion_index, &[&connecting_edges])?;

		let replaced_edges: Vec<_> = split_sources.iter().map(|source| source.segment_id).collect();
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

			let merged_segment_id = self.mesh_geometry.segment_domain.next_id();
			self.mesh_geometry.push(
				merged_segment_id,
				start_point_id,
				end_point_id,
				(Some(point_to_dvec2(first_segment.p1)), Some(point_to_dvec2(second_segment.p2))),
				StrokeId::ZERO,
			);
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
	fn eval_color(&self, u: f32, v: f32) -> [f32; 4] {
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

		interpolated_gamma_color
	}

	fn eval_vertex(&self, u: f64, v: f64, mesh_transform: DAffine2) -> MeshSubpatchVertex {
		let [top_seg, bottom_seg, left_seg, right_seg] = self.edges;
		let [top_left, top_right, bottom_left, bottom_right] = self.corners;

		let top_u = point_to_dvec2(top_seg.eval(u));
		let bottom_u = point_to_dvec2(bottom_seg.eval(u));
		let left_v = point_to_dvec2(left_seg.eval(v));
		let right_v = point_to_dvec2(right_seg.eval(v));

		let s_c = (1. - v) * top_u + v * bottom_u;
		let s_d = (1. - u) * left_v + u * right_v;
		let s_b = top_left * (1. - u) * (1. - v) + top_right * u * (1. - v) + bottom_left * (1. - u) * v + bottom_right * u * v;

		MeshSubpatchVertex {
			position: mesh_transform.transform_point2(s_c + s_d - s_b),
			gamma_color: self.eval_color(u as f32, v as f32),
		}
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

		// We need to calculate the color derivatives in sRGB since SVG uses sRGB for color interpolation.
		// `color-interpolation="linearRGB"` is part of the SVG2 spec but not yet implemented in major browsers as of Jul. 2026.
		// See also: https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Attribute/color-interpolation
		let gamma_colors: Vec<Vec4> = mesh_gradient.corner_colors.values.iter().map(|color| Vec4::from_array(color.to_gamma_srgb_channels())).collect();

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

		let mut patch_color_data = Vec::with_capacity(patch_rows.checked_mul(patch_columns)?);
		for row in 0..patch_rows {
			for column in 0..patch_columns {
				let patch = mesh_gradient.patch(row, column)?;
				let top_left_index = row * corner_columns + column;
				let corner_indices = [top_left_index, top_left_index + 1, top_left_index + corner_columns, top_left_index + corner_columns + 1];
				let patch_gamma_colors = corner_indices.map(|index| gamma_colors[index]);
				let color_slopes = corner_indices.map(|index| corner_slopes[index]);

				let [top_left_pos, top_right_pos, bottom_left_pos, bottom_right_pos] = patch.corners;
				let lengths = [
					top_left_pos.distance(top_right_pos) as f32,
					bottom_left_pos.distance(bottom_right_pos) as f32,
					top_left_pos.distance(bottom_left_pos) as f32,
					top_right_pos.distance(bottom_right_pos) as f32,
				];
				patch_color_data.push(MeshPatchEvaluator {
					corners: patch.corners,
					edges: patch.edges,
					gamma_colors: patch_gamma_colors,
					color_slopes,
					lengths,
				});
			}
		}

		Some(Self { patches: patch_color_data })
	}

	fn eval_color(&self, patch_index: usize, u: f32, v: f32) -> [f32; 4] {
		self.patches[patch_index].eval_color(u, v)
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
			let evaluate_row = |row: usize| -> Vec<MeshSubpatchVertex> {
				let v = row as f64 / count as f64;

				(0..=count)
					.map(|column| {
						let u = column as f64 / count as f64;
						patch.eval_vertex(u, v, mesh_transform)
					})
					.collect()
			};

			// Reusing the previous bottom row as a current top row to prevent duplicated evaluation on the same subpatch vertices.
			let mut top_row = evaluate_row(0);
			for row in 0..count {
				let bottom_row = evaluate_row(row + 1);
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
				let top_left = patch.eval_vertex(u_start, v_start, mesh_transform);
				let top_right = patch.eval_vertex(u_start + stride, v_start, mesh_transform);
				let bottom_left = patch.eval_vertex(u_start, v_start + stride, mesh_transform);
				let bottom_right = patch.eval_vertex(u_start + stride, v_start + stride, mesh_transform);
				let corners = [top_left, top_right, bottom_left, bottom_right];
				let [top_left_color, top_right_color, bottom_left_color, bottom_right_color] = corners.map(|vertex| Vec4::from_array(vertex.gamma_color));

				let mut within_tolerance = true;
				'error_samples: for &local_v in &samples {
					for &local_u in &samples {
						let vertex = patch.eval_vertex(u_start + local_u * stride, v_start + local_v * stride, mesh_transform);
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
					let child_subdivisions_per_axis = subdivisions_per_axis * 2;
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
		// FIXME: implement proper complexity calc
		return 10000000;
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

/// Helper to create initial handles.
fn handles(start: DVec2, end: DVec2) -> (Option<DVec2>, Option<DVec2>) {
	(Some(start + (end - start) / 3.), Some(end + (start - end) / 3.))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn assert_position(actual: DVec2, expected: DVec2) {
		assert!((actual - expected).length() < 1e-10, "expected {expected:?}, got {actual:?}");
	}

	#[test]
	fn inserting_mesh_grid_lines_preserves_row_major_topology() {
		let mut mesh = MeshGradient::default();
		let top_edge = *mesh.horizontal_edges.get(0, 0).unwrap();
		mesh.insert_grid_line(top_edge, 0.25).unwrap();

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
		mesh.insert_grid_line(left_edge, 0.5).unwrap();

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
		mesh.insert_grid_line(top_edge, 0.25).unwrap();
		let inserted_vertical_edge = *mesh.vertical_edges.get(0, 1).unwrap();
		mesh.remove_edge(inserted_vertical_edge).unwrap();

		assert_eq!(mesh.corner_points.dimensions(), [3, 3]);
		assert_eq!(mesh.horizontal_edges.dimensions(), [3, 2]);
		assert_eq!(mesh.vertical_edges.dimensions(), [2, 3]);
		assert_eq!(mesh.corners().map(|corner| corner.position).collect::<Vec<_>>(), expected_positions);
		assert_eq!(mesh.corners().map(|corner| corner.color).collect::<Vec<_>>(), expected_colors);

		let left_edge = *mesh.vertical_edges.get(0, 0).unwrap();
		mesh.insert_grid_line(left_edge, 0.5).unwrap();
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
}
