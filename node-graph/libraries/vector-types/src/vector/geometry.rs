use super::geometry_cache::SetVectorPaths;
use super::misc::{HandleId, ManipulatorGroup, ManipulatorPointId};
use super::path::{Path, paths_from_item_attributes};
use super::vector_attributes::{FoundSubpath, PointId, SegmentId};
use super::vector_types::Vector;
use core_types::list::{Item, List};
use glam::{DAffine2, DVec2};
use kurbo::{BezPath, PathSeg, Rect};

/// A vector element bundled with its paths, so editor-side callers can query geometry without
/// threading the holder's paths attribute through every call.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VectorGeometry {
	pub vector: Vector,
	pub paths: List<Path>,
}

impl VectorGeometry {
	pub fn new(vector: Vector, paths: List<Path>) -> Self {
		Self { vector, paths }
	}

	/// Clones a holder item's element and paths attribute into one bundle.
	pub fn from_item(item: &Item<Vector>) -> Self {
		Self {
			vector: item.element().clone(),
			paths: paths_from_item_attributes(item).cloned().unwrap_or_default(),
		}
	}

	/// Rebuilds a holder item carrying this bundle's element and paths.
	pub fn into_item(self) -> Item<Vector> {
		Item::new_from_element(self.vector).with_paths(self.paths)
	}

	pub fn point_count(&self) -> usize {
		self.vector.point_count()
	}

	pub fn position(&self, index: usize) -> Option<DVec2> {
		self.vector.position(index)
	}

	pub fn positions(&self) -> impl Iterator<Item = DVec2> + '_ {
		self.vector.positions()
	}

	pub fn point_id_at(&self, index: u32) -> Option<PointId> {
		self.vector.point_id_at(index)
	}

	pub fn index_of_point_id(&self, point: PointId) -> Option<u32> {
		self.vector.index_of_point_id(point)
	}

	/// The position of the pool point a point ID names.
	pub fn position_from_id(&self, point: PointId) -> Option<DVec2> {
		self.vector.index_of_point_id(point).and_then(|index| self.vector.position(index as usize))
	}

	pub fn anchor_positions(&self) -> Vec<DVec2> {
		self.vector.anchor_positions(&self.paths)
	}

	pub fn anchor_points(&self) -> impl Iterator<Item = PointId> {
		self.vector.anchor_points(&self.paths)
	}

	pub fn anchor_endpoints(&self) -> impl Iterator<Item = PointId> {
		self.vector.anchor_endpoints(&self.paths)
	}

	/// The ID and position of every anchor and free point, found in one pass over the paths.
	pub fn anchors(&self) -> impl Iterator<Item = (PointId, DVec2)> + '_ {
		let anchor_indices = self.vector.anchor_indices(&self.paths);
		anchor_indices
			.into_iter()
			.filter_map(|index| Some((self.vector.point_id_at(index as u32)?, self.vector.position(index)?)))
	}

	/// The ID and position of each pool point that no path references, found in one pass over the paths.
	pub fn free_points(&self) -> impl Iterator<Item = (PointId, DVec2)> + '_ {
		let connectivity = self.vector.connectivity(&self.paths);
		(0..self.vector.point_count() as u32)
			.filter(move |&index| !connectivity.is_referenced(index))
			.filter_map(|index| Some((self.vector.point_id_at(index)?, self.vector.position(index as usize)?)))
	}

	pub fn segment_iter(&self) -> impl Iterator<Item = (SegmentId, PathSeg, PointId, PointId)> + '_ {
		self.vector.segment_iter(&self.paths)
	}

	pub fn segment_ids(&self) -> Vec<SegmentId> {
		self.vector.segment_ids(&self.paths)
	}

	pub fn path_beginnings(&self) -> rustc_hash::FxHashSet<SegmentId> {
		self.vector.path_beginnings(&self.paths)
	}

	pub fn segment_from_id(&self, id: SegmentId) -> Option<PathSeg> {
		self.vector.segment_from_id(&self.paths, id)
	}

	pub fn segment_points_from_id(&self, id: SegmentId) -> Option<(PointId, PointId, PathSeg)> {
		self.vector.segment_points_from_id(&self.paths, id)
	}

	pub fn segment_with_pool_endpoints(&self, id: SegmentId) -> Option<(PathSeg, usize, usize)> {
		self.vector.segment_with_pool_endpoints(&self.paths, id)
	}

	/// Appends a lone segment between two existing points. See [`Vector::push`].
	pub fn push(&mut self, id: SegmentId, start: PointId, end: PointId, handles: (Option<DVec2>, Option<DVec2>)) {
		self.vector.push(&mut self.paths, id, start, end, handles);
	}

	pub fn segment_start_from_id(&self, segment: SegmentId) -> Option<PointId> {
		self.vector.segment_start_from_id(&self.paths, segment)
	}

	pub fn segment_end_from_id(&self, segment: SegmentId) -> Option<PointId> {
		self.vector.segment_end_from_id(&self.paths, segment)
	}

	pub fn start_point(&self) -> impl Iterator<Item = PointId> + '_ {
		self.vector.start_point(&self.paths)
	}

	pub fn end_point(&self) -> impl Iterator<Item = PointId> + '_ {
		self.vector.end_point(&self.paths)
	}

	pub fn other_point(&self, segment: SegmentId, current: PointId) -> Option<PointId> {
		self.vector.other_point(&self.paths, segment, current)
	}

	pub fn stroke_bezpath_iter(&self) -> impl Iterator<Item = BezPath> {
		self.vector.stroke_bezpath_iter(&self.paths)
	}

	pub fn stroke_manipulator_groups(&self) -> impl Iterator<Item = (Vec<ManipulatorGroup>, bool)> {
		self.vector.stroke_manipulator_groups(&self.paths)
	}

	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.vector.bounding_box(&self.paths)
	}

	pub fn bounding_box_rect(&self) -> Option<Rect> {
		self.vector.bounding_box_rect(&self.paths)
	}

	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.vector.bounding_box_with_transform(&self.paths, transform)
	}

	pub fn nonzero_bounding_box(&self) -> [DVec2; 2] {
		self.vector.nonzero_bounding_box(&self.paths)
	}

	pub fn stroke_inclusive_bounding_box_with_transform(&self, transform: DAffine2, stroke: Option<&super::style::Stroke>) -> Option<[DVec2; 2]> {
		self.vector.stroke_inclusive_bounding_box_with_transform(&self.paths, transform, stroke)
	}

	pub fn check_point_inside_shape(&self, transform: DAffine2, point: DVec2) -> bool {
		self.vector.check_point_inside_shape(&self.paths, transform, point)
	}

	pub fn start_connected(&self, point: PointId) -> impl Iterator<Item = SegmentId> {
		self.vector.start_connected(&self.paths, point)
	}

	pub fn end_connected(&self, point: PointId) -> impl Iterator<Item = SegmentId> {
		self.vector.end_connected(&self.paths, point)
	}

	pub fn all_connected(&self, point: PointId) -> impl Iterator<Item = HandleId> {
		self.vector.all_connected(&self.paths, point)
	}

	pub fn connected_count(&self, point: PointId) -> usize {
		self.vector.connected_count(&self.paths, point)
	}

	pub fn any_connected(&self, point: PointId) -> bool {
		self.vector.any_connected(&self.paths, point)
	}

	pub fn connected_points(&self, current: PointId) -> impl Iterator<Item = PointId> {
		self.vector.connected_points(&self.paths, current)
	}

	pub fn connected_linear_segments(&self, point_id: PointId) -> usize {
		self.vector.connected_linear_segments(&self.paths, point_id)
	}

	pub fn colinear(&self, point: ManipulatorPointId) -> bool {
		self.vector.colinear(&self.paths, point)
	}

	pub fn other_colinear_handle(&self, handle: HandleId) -> Option<HandleId> {
		self.vector.other_colinear_handle(&self.paths, handle)
	}

	pub fn colinear_pairs(&self) -> Vec<[HandleId; 2]> {
		self.vector.colinear_pairs(&self.paths)
	}

	pub fn colinear_pair_containing(&self, handle: HandleId) -> Option<[HandleId; 2]> {
		self.vector.colinear_pair_containing(&self.paths, handle)
	}

	pub fn adjacent_segment(&self, manipulator_id: &ManipulatorPointId) -> Option<(PointId, SegmentId)> {
		self.vector.adjacent_segment(&self.paths, manipulator_id)
	}

	pub fn auto_join_paths(&self) -> Vec<FoundSubpath> {
		self.vector.auto_join_paths(&self.paths)
	}

	pub fn bezpath_from_segments_ignore_discontinuities(&self, segments: impl Iterator<Item = (PathSeg, usize, usize)>) -> Option<BezPath> {
		self.vector.bezpath_from_segments_ignore_discontinuities(segments)
	}

	/// The manipulator's position in layer space. See [`ManipulatorPointId::get_position`].
	pub fn position_of(&self, point: ManipulatorPointId) -> Option<DVec2> {
		point.get_position(&self.vector, &self.paths)
	}

	/// The position of the anchor the manipulator sits on or belongs to.
	pub fn anchor_position_of(&self, point: ManipulatorPointId) -> Option<DVec2> {
		point.get_anchor_position(&self.vector, &self.paths)
	}

	/// The anchor the manipulator sits on or belongs to.
	pub fn anchor_of(&self, point: ManipulatorPointId) -> Option<PointId> {
		point.get_anchor(&self.vector, &self.paths)
	}

	/// The manipulator's handle pair. See [`ManipulatorPointId::get_handle_pair`].
	pub fn handle_pair_of(&self, point: ManipulatorPointId) -> Option<[HandleId; 2]> {
		point.get_handle_pair(&self.vector, &self.paths)
	}

	/// All handles connected to the manipulator's anchor, excluding itself when it is a handle.
	pub fn all_connected_handles_of(&self, point: ManipulatorPointId) -> Option<Vec<HandleId>> {
		point.get_all_connected_handles(&self.vector, &self.paths)
	}

	/// The handle's length from its anchor. See [`HandleId::length`].
	pub fn handle_length(&self, handle: HandleId) -> f64 {
		handle.length(&self.vector, &self.paths)
	}
}
