mod attributes;
mod indexed;
mod modification;

use super::misc::point_to_dvec2;
use super::style::{PathStyle, Stroke};
use crate::instances::Instances;
use crate::renderer::{ClickTargetGroup, FreePoint};
use crate::{AlphaBlending, Color, GraphicGroupTable};
pub use attributes::*;
use bezier_rs::{BezierHandles, ManipulatorGroup};
use core::borrow::Borrow;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
pub use indexed::VectorDataIndex;
pub use modification::*;
use std::collections::HashMap;

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_vector_data<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<VectorDataTable, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Debug, PartialEq, DynAny)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct OldVectorData {
		pub transform: DAffine2,
		pub alpha_blending: AlphaBlending,

		pub style: PathStyle,

		/// A list of all manipulator groups (referenced in `subpaths`) that have colinear handles (where they're locked at 180° angles from one another).
		/// This gets read in `graph_operation_message_handler.rs` by calling `inputs.as_mut_slice()` (search for the string `"Shape does not have both `subpath` and `colinear_manipulators` inputs"` to find it).
		pub colinear_manipulators: Vec<[HandleId; 2]>,

		pub point_domain: PointDomain,
		pub segment_domain: SegmentDomain,
		pub region_domain: RegionDomain,

		// Used to store the upstream graphic group during destructive Boolean Operations (and other nodes with a similar effect) so that click targets can be preserved.
		pub upstream_graphic_group: Option<GraphicGroupTable>,
	}

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	#[allow(clippy::large_enum_variant)]
	enum EitherFormat {
		VectorData(VectorData),
		OldVectorData(OldVectorData),
		VectorDataTable(VectorDataTable),
	}

	Ok(match EitherFormat::deserialize(deserializer)? {
		EitherFormat::VectorData(vector_data) => VectorDataTable::new(vector_data),
		EitherFormat::OldVectorData(old) => {
			let mut vector_data_table = VectorDataTable::new(VectorData {
				style: old.style,
				colinear_manipulators: old.colinear_manipulators,
				point_domain: old.point_domain,
				segment_domain: old.segment_domain,
				region_domain: old.region_domain,
				upstream_graphic_group: old.upstream_graphic_group,
			});
			*vector_data_table.one_instance_mut().transform = old.transform;
			*vector_data_table.one_instance_mut().alpha_blending = old.alpha_blending;
			vector_data_table
		}
		EitherFormat::VectorDataTable(vector_data_table) => vector_data_table,
	})
}

pub type VectorDataTable = Instances<VectorData>;

/// [VectorData] is passed between nodes.
/// It contains a list of subpaths (that may be open or closed), a transform, and some style information.
///
/// Segments are connected if they share endpoints.
#[derive(Clone, Debug, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VectorData {
	pub style: PathStyle,

	/// A list of all manipulator groups (referenced in `subpaths`) that have colinear handles (where they're locked at 180° angles from one another).
	/// This gets read in `graph_operation_message_handler.rs` by calling `inputs.as_mut_slice()` (search for the string `"Shape does not have both `subpath` and `colinear_manipulators` inputs"` to find it).
	pub colinear_manipulators: Vec<[HandleId; 2]>,

	pub point_domain: PointDomain,
	pub segment_domain: SegmentDomain,
	pub region_domain: RegionDomain,

	// Used to store the upstream graphic group during destructive Boolean Operations (and other nodes with a similar effect) so that click targets can be preserved.
	pub upstream_graphic_group: Option<GraphicGroupTable>,
}

impl core::hash::Hash for VectorData {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.point_domain.hash(state);
		self.segment_domain.hash(state);
		self.region_domain.hash(state);
		self.style.hash(state);
		self.colinear_manipulators.hash(state);
	}
}

impl VectorData {
	/// An empty subpath with no data, an identity transform, and a black fill.
	// TODO: Replace with just `Default`
	pub const fn empty() -> Self {
		Self {
			style: PathStyle::new(Some(Stroke::new(Some(Color::BLACK), 0.)), super::style::Fill::None),
			colinear_manipulators: Vec::new(),
			point_domain: PointDomain::new(),
			segment_domain: SegmentDomain::new(),
			region_domain: RegionDomain::new(),
			upstream_graphic_group: None,
		}
	}

	/// Push a subpath to the vector data
	pub fn append_subpath(&mut self, subpath: impl Borrow<bezier_rs::Subpath<PointId>>, preserve_id: bool) {
		let subpath: &bezier_rs::Subpath<PointId> = subpath.borrow();
		let stroke_id = StrokeId::ZERO;
		let mut point_id = self.point_domain.next_id();

		let handles = |a: &ManipulatorGroup<_>, b: &ManipulatorGroup<_>| match (a.out_handle, b.in_handle) {
			(None, None) => bezier_rs::BezierHandles::Linear,
			(Some(handle), None) | (None, Some(handle)) => bezier_rs::BezierHandles::Quadratic { handle },
			(Some(handle_start), Some(handle_end)) => bezier_rs::BezierHandles::Cubic { handle_start, handle_end },
		};
		let [mut first_seg, mut last_seg] = [None, None];
		let mut segment_id = self.segment_domain.next_id();
		let mut last_point = None;
		let mut first_point = None;
		// Constructs a bezier segment from the two manipulators on the subpath.
		for pair in subpath.manipulator_groups().windows(2) {
			let start = last_point.unwrap_or_else(|| {
				let id = if preserve_id && !self.point_domain.ids().contains(&pair[0].id) {
					pair[0].id
				} else {
					point_id.next_id()
				};
				self.point_domain.push(id, pair[0].anchor);
				self.point_domain.ids().len() - 1
			});
			first_point = Some(first_point.unwrap_or(start));
			let end = if preserve_id && !self.point_domain.ids().contains(&pair[1].id) {
				pair[1].id
			} else {
				point_id.next_id()
			};
			let end_index = self.point_domain.ids().len();
			self.point_domain.push(end, pair[1].anchor);

			let id = segment_id.next_id();
			first_seg = Some(first_seg.unwrap_or(id));
			last_seg = Some(id);
			self.segment_domain.push(id, start, end_index, handles(&pair[0], &pair[1]), stroke_id);

			last_point = Some(end_index);
		}

		let fill_id = FillId::ZERO;

		if subpath.closed() {
			if let (Some(last), Some(first), Some(first_id), Some(last_id)) = (subpath.manipulator_groups().last(), subpath.manipulator_groups().first(), first_point, last_point) {
				let id = segment_id.next_id();
				first_seg = Some(first_seg.unwrap_or(id));
				last_seg = Some(id);
				self.segment_domain.push(id, last_id, first_id, handles(last, first), stroke_id);
			}

			if let [Some(first_seg), Some(last_seg)] = [first_seg, last_seg] {
				self.region_domain.push(self.region_domain.next_id(), first_seg..=last_seg, fill_id);
			}
		}
	}

	pub fn append_free_point(&mut self, point: &FreePoint, preserve_id: bool) {
		let mut point_id = self.point_domain.next_id();
		// Use the current point id if it is not already in the domain else generate a new one
		let id = if preserve_id && !self.point_domain.ids().contains(&point.id) {
			point.id
		} else {
			point_id.next_id()
		};
		self.point_domain.push(id, point.position);
	}

	/// Appends a Kurbo BezPath to the vector data.
	pub fn append_bezpath(&mut self, bezpath: kurbo::BezPath) {
		let mut first_point_index = None;
		let mut last_point_index = None;

		let mut first_segment_id = None;
		let mut last_segment_id = None;

		let mut point_id = self.point_domain.next_id();
		let mut segment_id = self.segment_domain.next_id();

		let stroke_id = StrokeId::ZERO;
		let fill_id = FillId::ZERO;

		for element in bezpath.elements() {
			match *element {
				kurbo::PathEl::MoveTo(point) => {
					let next_point_index = self.point_domain.ids().len();
					self.point_domain.push(point_id.next_id(), point_to_dvec2(point));
					first_point_index = Some(next_point_index);
					last_point_index = Some(next_point_index);
				}
				kurbo::PathEl::ClosePath => match (first_point_index, last_point_index) {
					(Some(first_point_index), Some(last_point_index)) => {
						let next_segment_id = segment_id.next_id();
						self.segment_domain.push(next_segment_id, first_point_index, last_point_index, BezierHandles::Linear, stroke_id);

						let next_region_id = self.region_domain.next_id();
						self.region_domain.push(next_region_id, first_segment_id.unwrap()..=next_segment_id, fill_id);
					}
					_ => {
						error!("Empty bezpath cannot be closed.")
					}
				},
				_ => {}
			}

			let mut append_path_element = |handle: BezierHandles, point: kurbo::Point| {
				let next_point_index = self.point_domain.ids().len();
				self.point_domain.push(point_id.next_id(), point_to_dvec2(point));

				let next_segment_id = segment_id.next_id();
				self.segment_domain.push(segment_id.next_id(), last_point_index.unwrap(), next_point_index, handle, stroke_id);

				last_point_index = Some(next_point_index);
				first_segment_id = Some(first_segment_id.unwrap_or(next_segment_id));
				last_segment_id = Some(next_segment_id);
			};

			match *element {
				kurbo::PathEl::LineTo(point) => append_path_element(BezierHandles::Linear, point),
				kurbo::PathEl::QuadTo(handle, point) => append_path_element(BezierHandles::Quadratic { handle: point_to_dvec2(handle) }, point),
				kurbo::PathEl::CurveTo(handle_start, handle_end, point) => append_path_element(
					BezierHandles::Cubic {
						handle_start: point_to_dvec2(handle_start),
						handle_end: point_to_dvec2(handle_end),
					},
					point,
				),
				_ => {}
			}
		}
	}

	/// Construct some new vector data from a single subpath with an identity transform and black fill.
	pub fn from_subpath(subpath: impl Borrow<bezier_rs::Subpath<PointId>>) -> Self {
		Self::from_subpaths([subpath], false)
	}

	/// Construct some new vector data from subpaths with an identity transform and black fill.
	pub fn from_subpaths(subpaths: impl IntoIterator<Item = impl Borrow<bezier_rs::Subpath<PointId>>>, preserve_id: bool) -> Self {
		let mut vector_data = Self::empty();

		for subpath in subpaths.into_iter() {
			vector_data.append_subpath(subpath, preserve_id);
		}

		vector_data
	}

	pub fn from_target_groups(target_groups: impl IntoIterator<Item = impl Borrow<ClickTargetGroup>>, preserve_id: bool) -> Self {
		let mut vector_data = Self::empty();
		for target_group in target_groups.into_iter() {
			let target_group = target_group.borrow();
			match target_group {
				ClickTargetGroup::Subpath(subpath) => vector_data.append_subpath(subpath, preserve_id),
				ClickTargetGroup::FreePoint(point) => vector_data.append_free_point(point, preserve_id),
			}
		}
		vector_data
	}

	/// Compute the bounding boxes of the subpaths without any transform
	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(DAffine2::IDENTITY)
	}

	/// Compute the bounding boxes of the subpaths with the specified transform
	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		let combine = |[a_min, a_max]: [DVec2; 2], [b_min, b_max]: [DVec2; 2]| [a_min.min(b_min), a_max.max(b_max)];

		let anchor_bounds = self
			.point_domain
			.positions()
			.iter()
			.map(|&point| transform.transform_point2(point))
			.map(|point| [point, point])
			.reduce(combine);

		let segment_bounds = self
			.segment_bezier_iter()
			.map(|(_, bezier, _, _)| bezier.apply_transformation(|point| transform.transform_point2(point)).bounding_box())
			.reduce(combine);

		anchor_bounds.iter().chain(segment_bounds.iter()).copied().reduce(combine)
	}

	/// Calculate the corners of the bounding box but with a nonzero size.
	///
	/// If the layer bounds are `0` in either axis then they are changed to be `1`.
	pub fn nonzero_bounding_box(&self) -> [DVec2; 2] {
		let [bounds_min, mut bounds_max] = self.bounding_box().unwrap_or_default();

		let bounds_size = bounds_max - bounds_min;
		if bounds_size.x < 1e-10 {
			bounds_max.x = bounds_min.x + 1.;
		}
		if bounds_size.y < 1e-10 {
			bounds_max.y = bounds_min.y + 1.;
		}

		[bounds_min, bounds_max]
	}

	/// Compute the pivot of the layer in layerspace (the coordinates of the subpaths)
	pub fn layerspace_pivot(&self, normalized_pivot: DVec2) -> DVec2 {
		let [bounds_min, bounds_max] = self.nonzero_bounding_box();
		let bounds_size = bounds_max - bounds_min;
		bounds_min + bounds_size * normalized_pivot
	}

	pub fn start_point(&self) -> impl Iterator<Item = PointId> + '_ {
		self.segment_domain.start_point().iter().map(|&index| self.point_domain.ids()[index])
	}

	pub fn end_point(&self) -> impl Iterator<Item = PointId> + '_ {
		self.segment_domain.end_point().iter().map(|&index| self.point_domain.ids()[index])
	}

	pub fn push(&mut self, id: SegmentId, start: PointId, end: PointId, handles: bezier_rs::BezierHandles, stroke: StrokeId) {
		let [Some(start), Some(end)] = [start, end].map(|id| self.point_domain.resolve_id(id)) else {
			return;
		};
		self.segment_domain.push(id, start, end, handles, stroke)
	}

	pub fn handles_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut bezier_rs::BezierHandles, PointId, PointId)> {
		self.segment_domain
			.handles_mut()
			.map(|(id, handles, start, end)| (id, handles, self.point_domain.ids()[start], self.point_domain.ids()[end]))
	}

	pub fn segment_start_from_id(&self, segment: SegmentId) -> Option<PointId> {
		self.segment_domain.segment_start_from_id(segment).map(|index| self.point_domain.ids()[index])
	}

	pub fn segment_end_from_id(&self, segment: SegmentId) -> Option<PointId> {
		self.segment_domain.segment_end_from_id(segment).map(|index| self.point_domain.ids()[index])
	}

	/// Returns an array for the start and end points of a segment.
	pub fn points_from_id(&self, segment: SegmentId) -> Option<[PointId; 2]> {
		self.segment_domain.points_from_id(segment).map(|val| val.map(|index| self.point_domain.ids()[index]))
	}

	/// Attempts to find another point in the segment that is not the one passed in.
	pub fn other_point(&self, segment: SegmentId, current: PointId) -> Option<PointId> {
		let index = self.point_domain.resolve_id(current);
		index.and_then(|index| self.segment_domain.other_point(segment, index)).map(|index| self.point_domain.ids()[index])
	}

	/// Gets all points connected to the current one but not including the current one.
	pub fn connected_points(&self, current: PointId) -> impl Iterator<Item = PointId> + '_ {
		let index = [self.point_domain.resolve_id(current)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.connected_points(index).map(|index| self.point_domain.ids()[index]))
	}

	/// Get an array slice of all segment IDs.
	pub fn segment_ids(&self) -> &[SegmentId] {
		self.segment_domain.ids()
	}

	/// Enumerate all segments that start at the point.
	pub fn start_connected(&self, point: PointId) -> impl Iterator<Item = SegmentId> + '_ {
		let index = [self.point_domain.resolve_id(point)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.start_connected(index))
	}

	/// Enumerate all segments that end at the point.
	pub fn end_connected(&self, point: PointId) -> impl Iterator<Item = SegmentId> + '_ {
		let index = [self.point_domain.resolve_id(point)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.end_connected(index))
	}

	/// Enumerate all segments that start or end at a point, converting them to [`HandleId`s]. Note that the handles may not exist e.g. for a linear segment.
	pub fn all_connected(&self, point: PointId) -> impl Iterator<Item = HandleId> + '_ {
		let index = [self.point_domain.resolve_id(point)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.all_connected(index))
	}

	/// Enumerate the number of segments connected to a point. If a segment starts and ends at a point then it is counted twice.
	pub fn connected_count(&self, point: PointId) -> usize {
		self.point_domain.resolve_id(point).map_or(0, |point| self.segment_domain.connected_count(point))
	}

	/// Points that can be extended from.
	///
	/// This is usually only points with exactly one connection unless vector meshes are enabled.
	pub fn extendable_points(&self, vector_meshes: bool) -> impl Iterator<Item = PointId> + '_ {
		let point_ids = self.point_domain.ids().iter().enumerate();
		point_ids.filter(move |(index, _)| vector_meshes || self.segment_domain.connected_count(*index) == 1).map(|(_, &id)| id)
	}

	/// Computes if all the connected handles are colinear for an anchor, or if that handle is colinear for a handle.
	pub fn colinear(&self, point: ManipulatorPointId) -> bool {
		let has_handle = |target| self.colinear_manipulators.iter().flatten().any(|&handle| handle == target);
		match point {
			ManipulatorPointId::Anchor(id) => {
				self.start_connected(id).all(|segment| has_handle(HandleId::primary(segment))) && self.end_connected(id).all(|segment| has_handle(HandleId::end(segment)))
			}
			ManipulatorPointId::PrimaryHandle(segment) => has_handle(HandleId::primary(segment)),
			ManipulatorPointId::EndHandle(segment) => has_handle(HandleId::end(segment)),
		}
	}

	pub fn other_colinear_handle(&self, handle: HandleId) -> Option<HandleId> {
		let pair = self.colinear_manipulators.iter().find(|pair| pair.iter().any(|&val| val == handle))?;
		let other = pair.iter().copied().find(|&val| val != handle)?;
		if handle.to_manipulator_point().get_anchor(self) == other.to_manipulator_point().get_anchor(self) {
			Some(other)
		} else {
			None
		}
	}

	pub fn adjacent_segment(&self, manipulator_id: &ManipulatorPointId) -> Option<(PointId, SegmentId)> {
		match manipulator_id {
			ManipulatorPointId::PrimaryHandle(segment_id) => {
				// For start handle, find segments ending at our start point
				let (start_point_id, _, _) = self.segment_points_from_id(*segment_id)?;
				let start_index = self.point_domain.resolve_id(start_point_id)?;

				self.segment_domain.end_connected(start_index).find(|&id| id != *segment_id).map(|id| (start_point_id, id)).or(self
					.segment_domain
					.start_connected(start_index)
					.find(|&id| id != *segment_id)
					.map(|id| (start_point_id, id)))
			}
			ManipulatorPointId::EndHandle(segment_id) => {
				// For end handle, find segments starting at our end point
				let (_, end_point_id, _) = self.segment_points_from_id(*segment_id)?;
				let end_index = self.point_domain.resolve_id(end_point_id)?;

				self.segment_domain.start_connected(end_index).find(|&id| id != *segment_id).map(|id| (end_point_id, id)).or(self
					.segment_domain
					.end_connected(end_index)
					.find(|&id| id != *segment_id)
					.map(|id| (end_point_id, id)))
			}
			ManipulatorPointId::Anchor(_) => None,
		}
	}

	pub fn concat(&mut self, additional: &Self, transform_of_additional: DAffine2, collision_hash_seed: u64) {
		let point_map = additional
			.point_domain
			.ids()
			.iter()
			.filter(|id| self.point_domain.ids().contains(id))
			.map(|&old| (old, old.generate_from_hash(collision_hash_seed)))
			.collect::<HashMap<_, _>>();

		let segment_map = additional
			.segment_domain
			.ids()
			.iter()
			.filter(|id| self.segment_domain.ids().contains(id))
			.map(|&old| (old, old.generate_from_hash(collision_hash_seed)))
			.collect::<HashMap<_, _>>();

		let region_map = additional
			.region_domain
			.ids()
			.iter()
			.filter(|id| self.region_domain.ids().contains(id))
			.map(|&old| (old, old.generate_from_hash(collision_hash_seed)))
			.collect::<HashMap<_, _>>();

		let id_map = IdMap {
			point_offset: self.point_domain.ids().len(),
			point_map,
			segment_map,
			region_map,
		};

		self.point_domain.concat(&additional.point_domain, transform_of_additional, &id_map);
		self.segment_domain.concat(&additional.segment_domain, transform_of_additional, &id_map);
		self.region_domain.concat(&additional.region_domain, transform_of_additional, &id_map);

		// TODO: properly deal with fills such as gradients
		self.style = additional.style.clone();

		self.colinear_manipulators.extend(additional.colinear_manipulators.iter().copied());
	}
}

impl Default for VectorData {
	fn default() -> Self {
		Self::empty()
	}
}

/// A selectable part of a curve, either an anchor (start or end of a bézier) or a handle (doesn't necessarily go through the bézier but influences curvature).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ManipulatorPointId {
	/// A control anchor - the start or end point of a bézier.
	Anchor(PointId),
	/// The handle for a bézier - the first handle on a cubic and the only handle on a quadratic.
	PrimaryHandle(SegmentId),
	/// The end handle on a cubic bézier.
	EndHandle(SegmentId),
}

impl ManipulatorPointId {
	/// Attempt to retrieve the manipulator position in layer space (no transformation applied).
	#[must_use]
	#[track_caller]
	pub fn get_position(&self, vector_data: &VectorData) -> Option<DVec2> {
		match self {
			ManipulatorPointId::Anchor(id) => vector_data.point_domain.position_from_id(*id),
			ManipulatorPointId::PrimaryHandle(id) => vector_data.segment_from_id(*id).and_then(|bezier| bezier.handle_start()),
			ManipulatorPointId::EndHandle(id) => vector_data.segment_from_id(*id).and_then(|bezier| bezier.handle_end()),
		}
	}

	pub fn get_anchor_position(&self, vector_data: &VectorData) -> Option<DVec2> {
		match self {
			ManipulatorPointId::EndHandle(_) | ManipulatorPointId::PrimaryHandle(_) => self.get_anchor(vector_data).and_then(|id| vector_data.point_domain.position_from_id(id)),
			_ => self.get_position(vector_data),
		}
	}

	/// Attempt to get a pair of handles. For an anchor this is the first two handles connected. For a handle it is self and the first opposing handle.
	#[must_use]
	pub fn get_handle_pair(self, vector_data: &VectorData) -> Option<[HandleId; 2]> {
		match self {
			ManipulatorPointId::Anchor(point) => vector_data.all_connected(point).take(2).collect::<Vec<_>>().try_into().ok(),
			ManipulatorPointId::PrimaryHandle(segment) => {
				let point = vector_data.segment_domain.segment_start_from_id(segment)?;
				let current = HandleId::primary(segment);
				let other = vector_data.segment_domain.all_connected(point).find(|&value| value != current);
				other.map(|other| [current, other])
			}
			ManipulatorPointId::EndHandle(segment) => {
				let point = vector_data.segment_domain.segment_end_from_id(segment)?;
				let current = HandleId::end(segment);
				let other = vector_data.segment_domain.all_connected(point).find(|&value| value != current);
				other.map(|other| [current, other])
			}
		}
	}

	/// Attempt to find the closest anchor. If self is already an anchor then it is just self. If it is a start or end handle, then the start or end point is chosen.
	#[must_use]
	pub fn get_anchor(self, vector_data: &VectorData) -> Option<PointId> {
		match self {
			ManipulatorPointId::Anchor(point) => Some(point),
			ManipulatorPointId::PrimaryHandle(segment) => vector_data.segment_start_from_id(segment),
			ManipulatorPointId::EndHandle(segment) => vector_data.segment_end_from_id(segment),
		}
	}

	/// Attempt to convert self to a [`HandleId`], returning none for an anchor.
	#[must_use]
	pub fn as_handle(self) -> Option<HandleId> {
		match self {
			ManipulatorPointId::PrimaryHandle(segment) => Some(HandleId::primary(segment)),
			ManipulatorPointId::EndHandle(segment) => Some(HandleId::end(segment)),
			ManipulatorPointId::Anchor(_) => None,
		}
	}

	/// Attempt to convert self to an anchor, returning None for a handle.
	#[must_use]
	pub fn as_anchor(self) -> Option<PointId> {
		match self {
			ManipulatorPointId::Anchor(point) => Some(point),
			_ => None,
		}
	}

	pub fn get_segment(self) -> Option<SegmentId> {
		match self {
			ManipulatorPointId::PrimaryHandle(segment) | ManipulatorPointId::EndHandle(segment) => Some(segment),
			_ => None,
		}
	}
}

/// The type of handle found on a bézier curve.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HandleType {
	/// The first handle on a cubic bézier or the only handle on a quadratic bézier.
	Primary,
	/// The second handle on a cubic bézier.
	End,
}

/// Represents a primary or end handle found in a particular segment.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HandleId {
	pub ty: HandleType,
	pub segment: SegmentId,
}

impl HandleId {
	/// Construct a handle for the first handle on a cubic bézier or the only handle on a quadratic bézier.
	#[must_use]
	pub const fn primary(segment: SegmentId) -> Self {
		Self { ty: HandleType::Primary, segment }
	}

	/// Construct a handle for the end handle on a cubic bézier.
	#[must_use]
	pub const fn end(segment: SegmentId) -> Self {
		Self { ty: HandleType::End, segment }
	}

	/// Convert to [`ManipulatorPointId`].
	#[must_use]
	pub fn to_manipulator_point(self) -> ManipulatorPointId {
		match self.ty {
			HandleType::Primary => ManipulatorPointId::PrimaryHandle(self.segment),
			HandleType::End => ManipulatorPointId::EndHandle(self.segment),
		}
	}

	/// Calculate the magnitude of the handle from the anchor.
	pub fn length(self, vector_data: &VectorData) -> f64 {
		let Some(anchor_position) = self.to_manipulator_point().get_anchor_position(vector_data) else {
			// TODO: This was previously an unwrap which was encountered, so this is a temporary way to avoid a crash
			return 0.;
		};
		let handle_position = self.to_manipulator_point().get_position(vector_data);
		handle_position.map(|pos| (pos - anchor_position).length()).unwrap_or(f64::MAX)
	}

	/// Set the handle's position relative to the anchor which is the start anchor for the primary handle and end anchor for the end handle.
	#[must_use]
	pub fn set_relative_position(self, relative_position: DVec2) -> VectorModificationType {
		let Self { ty, segment } = self;
		match ty {
			HandleType::Primary => VectorModificationType::SetPrimaryHandle { segment, relative_position },
			HandleType::End => VectorModificationType::SetEndHandle { segment, relative_position },
		}
	}

	/// Convert an end handle to the primary handle and a primary handle to an end handle. Note that the new handle may not exist (e.g. for a quadratic bézier).
	#[must_use]
	pub fn opposite(self) -> Self {
		match self.ty {
			HandleType::Primary => Self::end(self.segment),
			HandleType::End => Self::primary(self.segment),
		}
	}
}

#[cfg(test)]
fn assert_subpath_eq(generated: &[bezier_rs::Subpath<PointId>], expected: &[bezier_rs::Subpath<PointId>]) {
	assert_eq!(generated.len(), expected.len());
	for (generated, expected) in generated.iter().zip(expected) {
		assert_eq!(generated.manipulator_groups().len(), expected.manipulator_groups().len());
		assert_eq!(generated.closed(), expected.closed());
		for (generated, expected) in generated.manipulator_groups().iter().zip(expected.manipulator_groups()) {
			assert_eq!(generated.in_handle, expected.in_handle);
			assert_eq!(generated.out_handle, expected.out_handle);
			assert_eq!(generated.anchor, expected.anchor);
		}
	}
}

#[test]
fn construct_closed_subpath() {
	let circle = bezier_rs::Subpath::new_ellipse(DVec2::NEG_ONE, DVec2::ONE);
	let vector_data = VectorData::from_subpath(&circle);
	assert_eq!(vector_data.point_domain.ids().len(), 4);
	let bezier_paths = vector_data.segment_bezier_iter().map(|(_, bezier, _, _)| bezier).collect::<Vec<_>>();
	assert_eq!(bezier_paths.len(), 4);
	assert!(bezier_paths.iter().all(|&bezier| circle.iter().any(|original_bezier| original_bezier == bezier)));

	let generated = vector_data.stroke_bezier_paths().collect::<Vec<_>>();
	assert_subpath_eq(&generated, &[circle]);
}

#[test]
fn construct_open_subpath() {
	let bezier = bezier_rs::Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::NEG_ONE, DVec2::ONE, DVec2::X);
	let subpath = bezier_rs::Subpath::from_bezier(&bezier);
	let vector_data = VectorData::from_subpath(&subpath);
	assert_eq!(vector_data.point_domain.ids().len(), 2);
	let bezier_paths = vector_data.segment_bezier_iter().map(|(_, bezier, _, _)| bezier).collect::<Vec<_>>();
	assert_eq!(bezier_paths, vec![bezier]);

	let generated = vector_data.stroke_bezier_paths().collect::<Vec<_>>();
	assert_subpath_eq(&generated, &[subpath]);
}

#[test]
fn construct_many_subpath() {
	let curve = bezier_rs::Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::NEG_ONE, DVec2::ONE, DVec2::X);
	let curve = bezier_rs::Subpath::from_bezier(&curve);
	let circle = bezier_rs::Subpath::new_ellipse(DVec2::NEG_ONE, DVec2::ONE);

	let vector_data = VectorData::from_subpaths([&curve, &circle], false);
	assert_eq!(vector_data.point_domain.ids().len(), 6);

	let bezier_paths = vector_data.segment_bezier_iter().map(|(_, bezier, _, _)| bezier).collect::<Vec<_>>();
	assert_eq!(bezier_paths.len(), 5);
	assert!(bezier_paths.iter().all(|&bezier| circle.iter().chain(curve.iter()).any(|original_bezier| original_bezier == bezier)));

	let generated = vector_data.stroke_bezier_paths().collect::<Vec<_>>();
	assert_subpath_eq(&generated, &[curve, circle]);
}
