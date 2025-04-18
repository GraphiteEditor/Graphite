use crate::vector::vector_data::{HandleId, VectorData};
use bezier_rs::BezierHandles;
use core::iter::zip;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// A simple macro for creating strongly typed ids (to avoid confusion when passing around ids).
macro_rules! create_ids {
	($($id:ident),*) => {
		$(
			#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, DynAny)]
			#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
			/// A strongly typed ID
			pub struct $id(u64);

			impl $id {
				pub const ZERO: $id = $id(0);

				/// Generate a new random id
				pub fn generate() -> Self {
					Self(crate::uuid::generate_uuid())
				}

				pub fn generate_from_hash(self, node_id: u64) -> Self {
					let mut hasher = std::hash::DefaultHasher::new();
					node_id.hash(&mut hasher);
					self.hash(&mut hasher);
					let hash_value = hasher.finish();
					Self(hash_value)
				}

				/// Gets the inner raw value.
				pub fn inner(self) -> u64 {
					self.0
				}

				/// Adds one to the current value and returns the old value. Note that the ids are not going to be unique unless you use the largest id.
				pub fn next_id(&mut self) -> Self {
					self.0 += 1;
					*self
				}

				pub fn from_u64(id: u64) -> Self {
					Self(id)
				}
			}
		)*
	};
}

create_ids! { InstanceId, PointId, SegmentId, RegionId, StrokeId, FillId }

/// A no-op hasher that allows writing u64s (the id type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoHash(Option<u64>);

impl core::hash::Hasher for NoHash {
	fn finish(&self) -> u64 {
		self.0.unwrap()
	}
	fn write(&mut self, _bytes: &[u8]) {
		unimplemented!()
	}
	fn write_u64(&mut self, i: u64) {
		debug_assert!(self.0.is_none());
		self.0 = Some(i)
	}
}

/// A hash builder that builds the [`NoHash`] hasher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoHashBuilder;

impl core::hash::BuildHasher for NoHashBuilder {
	type Hasher = NoHash;
	fn build_hasher(&self) -> Self::Hasher {
		NoHash::default()
	}
}

#[derive(Clone, Debug, Default, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Stores data which is per-point. Each point is merely a position and can be used in a point cloud or to for a bézier path. In future this will be extendable at runtime with custom attributes.
pub struct PointDomain {
	id: Vec<PointId>,
	#[serde(alias = "positions")]
	pub(crate) position: Vec<DVec2>,
}

impl core::hash::Hash for PointDomain {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
		self.position.iter().for_each(|pos| pos.to_array().map(|v| v.to_bits()).hash(state));
	}
}

impl PointDomain {
	pub const fn new() -> Self {
		Self { id: Vec::new(), position: Vec::new() }
	}

	pub fn clear(&mut self) {
		self.id.clear();
		self.position.clear();
	}

	pub fn retain(&mut self, segment_domain: &mut SegmentDomain, f: impl Fn(&PointId) -> bool) {
		let mut keep = self.id.iter().map(&f);
		self.position.retain(|_| keep.next().unwrap_or_default());

		// TODO(TrueDoctor): Consider using a prefix sum to avoid this Vec allocation (https://github.com/GraphiteEditor/Graphite/pull/1949#discussion_r1741711562)
		let mut id_map = Vec::with_capacity(self.ids().len());
		let mut new_index = 0;
		for id in self.ids() {
			if f(id) {
				id_map.push(new_index);
				new_index += 1;
			} else {
				// A placeholder for invalid IDs. This is checked after the segment domain is modified.
				id_map.push(usize::MAX);
			}
		}

		let update_index = |index: &mut usize| *index = id_map[*index];
		segment_domain.start_point.iter_mut().for_each(update_index);
		segment_domain.end_point.iter_mut().for_each(update_index);

		self.id.retain(f);
	}

	pub fn push(&mut self, id: PointId, position: DVec2) {
		debug_assert!(!self.id.contains(&id));
		self.id.push(id);
		self.position.push(position);
	}

	pub fn positions(&self) -> &[DVec2] {
		&self.position
	}

	pub fn positions_mut(&mut self) -> impl Iterator<Item = (PointId, &mut DVec2)> {
		self.id.iter().copied().zip(self.position.iter_mut())
	}

	pub fn set_position(&mut self, index: usize, position: DVec2) {
		self.position[index] = position;
	}

	pub fn ids(&self) -> &[PointId] {
		&self.id
	}

	pub fn next_id(&self) -> PointId {
		self.ids().iter().copied().max_by(|a, b| a.0.cmp(&b.0)).map(|mut id| id.next_id()).unwrap_or(PointId::ZERO)
	}

	#[track_caller]
	pub fn position_from_id(&self, id: PointId) -> Option<DVec2> {
		let pos = self.resolve_id(id).map(|index| self.position[index]);
		if pos.is_none() {
			warn!("Resolving pos of invalid id");
		}
		pos
	}

	pub(crate) fn resolve_id(&self, id: PointId) -> Option<usize> {
		self.id.iter().position(|&check_id| check_id == id)
	}

	pub fn concat(&mut self, other: &Self, transform: DAffine2, id_map: &IdMap) {
		self.id.extend(other.id.iter().map(|id| *id_map.point_map.get(id).unwrap_or(id)));
		self.position.extend(other.position.iter().map(|&pos| transform.transform_point2(pos)));
	}

	pub fn map_ids(&mut self, id_map: &IdMap) {
		self.id.iter_mut().for_each(|id| *id = *id_map.point_map.get(id).unwrap_or(id));
	}

	pub fn transform(&mut self, transform: DAffine2) {
		for pos in &mut self.position {
			*pos = transform.transform_point2(*pos);
		}
	}

	/// Iterate over point IDs and positions
	pub fn iter(&self) -> impl Iterator<Item = (PointId, DVec2)> + '_ {
		self.ids().iter().copied().zip(self.positions().iter().copied())
	}
}

#[derive(Clone, Debug, Default, PartialEq, Hash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Stores data which is per-segment. A segment is a bézier curve between two end points with a stroke. In future this will be extendable at runtime with custom attributes.
pub struct SegmentDomain {
	#[serde(alias = "ids")]
	id: Vec<SegmentId>,
	start_point: Vec<usize>,
	end_point: Vec<usize>,
	handles: Vec<bezier_rs::BezierHandles>,
	stroke: Vec<StrokeId>,
}

impl SegmentDomain {
	pub const fn new() -> Self {
		Self {
			id: Vec::new(),
			start_point: Vec::new(),
			end_point: Vec::new(),
			handles: Vec::new(),
			stroke: Vec::new(),
		}
	}

	pub fn clear(&mut self) {
		self.id.clear();
		self.start_point.clear();
		self.end_point.clear();
		self.handles.clear();
		self.stroke.clear();
	}

	pub fn retain(&mut self, f: impl Fn(&SegmentId) -> bool, points_length: usize) {
		let additional_delete_ids = self
			.id
			.iter()
			.zip(&self.start_point)
			.zip(&self.end_point)
			.filter(|((_, start), end)| **start >= points_length || **end >= points_length)
			.map(|x| *x.0.0)
			.collect::<Vec<_>>();

		let can_delete = || {
			let f = &f;
			let mut delete_iter = additional_delete_ids.iter().peekable();
			move |id| {
				if delete_iter.peek() == Some(&id) {
					delete_iter.next();
					false
				} else {
					f(id)
				}
			}
		};

		let mut keep = self.id.iter().map(can_delete());
		self.start_point.retain(|_| keep.next().unwrap_or_default());
		let mut keep = self.id.iter().map(can_delete());
		self.end_point.retain(|_| keep.next().unwrap_or_default());
		let mut keep = self.id.iter().map(can_delete());
		self.handles.retain(|_| keep.next().unwrap_or_default());
		let mut keep = self.id.iter().map(can_delete());
		self.stroke.retain(|_| keep.next().unwrap_or_default());

		let mut delete_iter = additional_delete_ids.iter().peekable();
		self.id.retain(move |id| {
			if delete_iter.peek() == Some(&id) {
				delete_iter.next();
				false
			} else {
				f(id)
			}
		});
	}

	pub fn ids(&self) -> &[SegmentId] {
		&self.id
	}

	pub fn next_id(&self) -> SegmentId {
		self.ids().iter().copied().max_by(|a, b| a.0.cmp(&b.0)).map(|mut id| id.next_id()).unwrap_or(SegmentId::ZERO)
	}

	pub(crate) fn start_point(&self) -> &[usize] {
		&self.start_point
	}

	pub(crate) fn end_point(&self) -> &[usize] {
		&self.end_point
	}

	pub fn set_start_point(&mut self, segment_index: usize, new: usize) {
		self.start_point[segment_index] = new;
	}

	pub fn set_end_point(&mut self, segment_index: usize, new: usize) {
		self.end_point[segment_index] = new;
	}

	pub fn handles(&self) -> &[bezier_rs::BezierHandles] {
		&self.handles
	}

	pub fn stroke(&self) -> &[StrokeId] {
		&self.stroke
	}

	pub(crate) fn push(&mut self, id: SegmentId, start: usize, end: usize, handles: bezier_rs::BezierHandles, stroke: StrokeId) {
		debug_assert!(!self.id.contains(&id), "Tried to push an existing point to a point domain");

		self.id.push(id);
		self.start_point.push(start);
		self.end_point.push(end);
		self.handles.push(handles);
		self.stroke.push(stroke);
	}

	pub(crate) fn start_point_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut usize)> {
		self.id.iter().copied().zip(self.start_point.iter_mut())
	}

	pub(crate) fn end_point_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut usize)> {
		self.id.iter().copied().zip(self.end_point.iter_mut())
	}

	pub(crate) fn handles_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut bezier_rs::BezierHandles, usize, usize)> {
		let nested = self.id.iter().zip(&mut self.handles).zip(&self.start_point).zip(&self.end_point);
		nested.map(|(((&a, b), &c), &d)| (a, b, c, d))
	}

	pub(crate) fn handles_and_points_mut(&mut self) -> impl Iterator<Item = (&mut bezier_rs::BezierHandles, &mut usize, &mut usize)> {
		let nested = self.handles.iter_mut().zip(&mut self.start_point).zip(&mut self.end_point);
		nested.map(|((a, b), c)| (a, b, c))
	}

	pub fn stroke_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut StrokeId)> {
		self.id.iter().copied().zip(self.stroke.iter_mut())
	}

	pub(crate) fn segment_start_from_id(&self, segment: SegmentId) -> Option<usize> {
		self.id_to_index(segment).and_then(|index| self.start_point.get(index)).copied()
	}

	pub(crate) fn segment_end_from_id(&self, segment: SegmentId) -> Option<usize> {
		self.id_to_index(segment).and_then(|index| self.end_point.get(index)).copied()
	}

	/// Returns an array for the start and end points of a segment.
	pub(crate) fn points_from_id(&self, segment: SegmentId) -> Option<[usize; 2]> {
		self.segment_start_from_id(segment).and_then(|start| self.segment_end_from_id(segment).map(|end| [start, end]))
	}

	/// Attempts to find another point in the segment that is not the one passed in.
	pub(crate) fn other_point(&self, segment: SegmentId, current: usize) -> Option<usize> {
		self.points_from_id(segment).and_then(|points| points.into_iter().find(|&point| point != current))
	}

	/// Gets all points connected to the current one but not including the current one.
	pub(crate) fn connected_points(&self, current: usize) -> impl Iterator<Item = usize> + '_ {
		self.start_point.iter().zip(&self.end_point).filter_map(move |(&a, &b)| match (a == current, b == current) {
			(true, false) => Some(b),
			(false, true) => Some(a),
			_ => None,
		})
	}

	/// Get index from ID by linear search. Takes `O(n)` time.
	fn id_to_index(&self, id: SegmentId) -> Option<usize> {
		debug_assert_eq!(self.id.len(), self.handles.len());
		debug_assert_eq!(self.id.len(), self.start_point.len());
		debug_assert_eq!(self.id.len(), self.end_point.len());
		self.id.iter().position(|&check_id| check_id == id)
	}

	fn resolve_range(&self, range: &core::ops::RangeInclusive<SegmentId>) -> Option<core::ops::RangeInclusive<usize>> {
		match (self.id_to_index(*range.start()), self.id_to_index(*range.end())) {
			(Some(start), Some(end)) if start.max(end) < self.handles.len().min(self.id.len()).min(self.start_point.len()).min(self.end_point.len()) => Some(start..=end),
			_ => {
				warn!("Resolving range with invalid id");
				None
			}
		}
	}

	pub fn concat(&mut self, other: &Self, transform: DAffine2, id_map: &IdMap) {
		self.id.extend(other.id.iter().map(|id| *id_map.segment_map.get(id).unwrap_or(id)));
		self.start_point.extend(other.start_point.iter().map(|&index| id_map.point_offset + index));
		self.end_point.extend(other.end_point.iter().map(|&index| id_map.point_offset + index));
		self.handles.extend(other.handles.iter().map(|handles| handles.apply_transformation(|p| transform.transform_point2(p))));
		self.stroke.extend(&other.stroke);
	}

	pub fn map_ids(&mut self, id_map: &IdMap) {
		self.id.iter_mut().for_each(|id| *id = *id_map.segment_map.get(id).unwrap_or(id));
	}

	pub fn transform(&mut self, transform: DAffine2) {
		for handles in &mut self.handles {
			*handles = handles.apply_transformation(|p| transform.transform_point2(p));
		}
	}

	/// Enumerate all segments that start at the point.
	pub(crate) fn start_connected(&self, point: usize) -> impl Iterator<Item = SegmentId> + '_ {
		self.start_point.iter().zip(&self.id).filter(move |&(&found_point, _)| found_point == point).map(|(_, &seg)| seg)
	}

	/// Enumerate all segments that end at the point.
	pub(crate) fn end_connected(&self, point: usize) -> impl Iterator<Item = SegmentId> + '_ {
		self.end_point.iter().zip(&self.id).filter(move |&(&found_point, _)| found_point == point).map(|(_, &seg)| seg)
	}

	/// Enumerate all segments that start or end at a point, converting them to [`HandleId`s]. Note that the handles may not exist e.g. for a linear segment.
	pub(crate) fn all_connected(&self, point: usize) -> impl Iterator<Item = HandleId> + '_ {
		self.start_connected(point).map(HandleId::primary).chain(self.end_connected(point).map(HandleId::end))
	}

	/// Enumerate the number of segments connected to a point. If a segment starts and ends at a point then it is counted twice.
	pub(crate) fn connected_count(&self, point: usize) -> usize {
		self.all_connected(point).count()
	}

	/// Iterates over segments in the domain.
	///
	/// Tuple is: (id, start point, end point, handles)
	pub fn iter(&self) -> impl Iterator<Item = (SegmentId, usize, usize, BezierHandles)> + '_ {
		let ids = self.id.iter().copied();
		let start_point = self.start_point.iter().copied();
		let end_point = self.end_point.iter().copied();
		let handles = self.handles.iter().copied();
		zip(ids, zip(start_point, zip(end_point, handles))).map(|(id, (start_point, (end_point, handles)))| (id, start_point, end_point, handles))
	}

	/// Iterates over segments in the domain, mutably.
	///
	/// Tuple is: (id, start point, end point, handles)
	pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (&mut SegmentId, &mut usize, &mut usize, &mut BezierHandles)> + '_ {
		let ids = self.id.iter_mut();
		let start_point = self.start_point.iter_mut();
		let end_point = self.end_point.iter_mut();
		let handles = self.handles.iter_mut();
		zip(ids, zip(start_point, zip(end_point, handles))).map(|(id, (start_point, (end_point, handles)))| (id, start_point, end_point, handles))
	}
}

#[derive(Clone, Debug, Default, PartialEq, Hash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Stores data which is per-region. A region is an enclosed area composed of a range of segments from the
/// [`SegmentDomain`] that can be given a fill. In future this will be extendable at runtime with custom attributes.
pub struct RegionDomain {
	#[serde(alias = "ids")]
	id: Vec<RegionId>,
	segment_range: Vec<core::ops::RangeInclusive<SegmentId>>,
	fill: Vec<FillId>,
}

impl RegionDomain {
	pub const fn new() -> Self {
		Self {
			id: Vec::new(),
			segment_range: Vec::new(),
			fill: Vec::new(),
		}
	}

	pub fn clear(&mut self) {
		self.id.clear();
		self.segment_range.clear();
		self.fill.clear();
	}

	pub fn retain(&mut self, f: impl Fn(&RegionId) -> bool) {
		let mut keep = self.id.iter().map(&f);
		self.segment_range.retain(|_| keep.next().unwrap_or_default());
		let mut keep = self.id.iter().map(&f);
		self.fill.retain(|_| keep.next().unwrap_or_default());
		self.id.retain(&f);
	}

	/// Like [`Self::retain`] but also gives the function access to the segment range.
	///
	/// Note that this function requires an allocation that `retain` avoids.
	pub fn retain_with_region(&mut self, f: impl Fn(&RegionId, &core::ops::RangeInclusive<SegmentId>) -> bool) {
		let keep = self.id.iter().zip(self.segment_range.iter()).map(|(id, range)| f(id, range)).collect::<Vec<_>>();
		let mut iter = keep.iter().copied();
		self.segment_range.retain(|_| iter.next().unwrap());
		let mut iter = keep.iter().copied();
		self.fill.retain(|_| iter.next().unwrap());
		let mut iter = keep.iter().copied();
		self.id.retain(|_| iter.next().unwrap());
	}

	pub fn push(&mut self, id: RegionId, segment_range: core::ops::RangeInclusive<SegmentId>, fill: FillId) {
		if self.id.contains(&id) {
			warn!("Duplicate region");
			return;
		}
		self.id.push(id);
		self.segment_range.push(segment_range);
		self.fill.push(fill);
	}

	fn _resolve_id(&self, id: RegionId) -> Option<usize> {
		self.id.iter().position(|&check_id| check_id == id)
	}

	pub fn next_id(&self) -> RegionId {
		self.id.iter().copied().max_by(|a, b| a.0.cmp(&b.0)).map(|mut id| id.next_id()).unwrap_or(RegionId::ZERO)
	}

	pub fn segment_range_mut(&mut self) -> impl Iterator<Item = (RegionId, &mut core::ops::RangeInclusive<SegmentId>)> {
		self.id.iter().copied().zip(self.segment_range.iter_mut())
	}

	pub fn fill_mut(&mut self) -> impl Iterator<Item = (RegionId, &mut FillId)> {
		self.id.iter().copied().zip(self.fill.iter_mut())
	}

	pub fn ids(&self) -> &[RegionId] {
		&self.id
	}

	pub fn segment_range(&self) -> &[core::ops::RangeInclusive<SegmentId>] {
		&self.segment_range
	}

	pub fn fill(&self) -> &[FillId] {
		&self.fill
	}

	pub fn concat(&mut self, other: &Self, _transform: DAffine2, id_map: &IdMap) {
		self.id.extend(other.id.iter().map(|id| *id_map.region_map.get(id).unwrap_or(id)));
		self.segment_range.extend(
			other
				.segment_range
				.iter()
				.map(|range| *id_map.segment_map.get(range.start()).unwrap_or(range.start())..=*id_map.segment_map.get(range.end()).unwrap_or(range.end())),
		);
		self.fill.extend(&other.fill);
	}

	pub fn map_ids(&mut self, id_map: &IdMap) {
		self.id.iter_mut().for_each(|id| *id = *id_map.region_map.get(id).unwrap_or(id));
		self.segment_range
			.iter_mut()
			.for_each(|range| *range = *id_map.segment_map.get(range.start()).unwrap_or(range.start())..=*id_map.segment_map.get(range.end()).unwrap_or(range.end()));
	}

	/// Iterates over regions in the domain.
	///
	/// Tuple is: (id, segment_range, fill)
	pub fn iter(&self) -> impl Iterator<Item = (RegionId, core::ops::RangeInclusive<SegmentId>, FillId)> + '_ {
		let ids = self.id.iter().copied();
		let segment_range = self.segment_range.iter().cloned();
		let fill = self.fill.iter().copied();
		zip(ids, zip(segment_range, fill)).map(|(id, (segment_range, fill))| (id, segment_range, fill))
	}
}

impl VectorData {
	/// Construct a [`bezier_rs::Bezier`] curve spanning from the resolved position of the start and end points with the specified handles.
	fn segment_to_bezier_with_index(&self, start: usize, end: usize, handles: bezier_rs::BezierHandles) -> bezier_rs::Bezier {
		let start = self.point_domain.positions()[start];
		let end = self.point_domain.positions()[end];
		bezier_rs::Bezier { start, end, handles }
	}

	/// Tries to convert a segment with the specified id to a [`bezier_rs::Bezier`], returning None if the id is invalid.
	pub fn segment_from_id(&self, id: SegmentId) -> Option<bezier_rs::Bezier> {
		self.segment_points_from_id(id).map(|(_, _, bezier)| bezier)
	}

	/// Tries to convert a segment with the specified id to the start and end points and a [`bezier_rs::Bezier`], returning None if the id is invalid.
	pub fn segment_points_from_id(&self, id: SegmentId) -> Option<(PointId, PointId, bezier_rs::Bezier)> {
		Some(self.segment_points_from_index(self.segment_domain.id_to_index(id)?))
	}

	/// Tries to convert a segment with the specified index to the start and end points and a [`bezier_rs::Bezier`].
	pub fn segment_points_from_index(&self, index: usize) -> (PointId, PointId, bezier_rs::Bezier) {
		let start = self.segment_domain.start_point[index];
		let end = self.segment_domain.end_point[index];
		let start_id = self.point_domain.ids()[start];
		let end_id = self.point_domain.ids()[end];
		(start_id, end_id, self.segment_to_bezier_with_index(start, end, self.segment_domain.handles[index]))
	}

	/// Iterator over all of the [`bezier_rs::Bezier`] following the order that they are stored in the segment domain, skipping invalid segments.
	pub fn segment_bezier_iter(&self) -> impl Iterator<Item = (SegmentId, bezier_rs::Bezier, PointId, PointId)> + '_ {
		let to_bezier = |(((&handles, &id), &start), &end)| (id, self.segment_to_bezier_with_index(start, end, handles), self.point_domain.ids()[start], self.point_domain.ids()[end]);
		self.segment_domain
			.handles
			.iter()
			.zip(&self.segment_domain.id)
			.zip(self.segment_domain.start_point())
			.zip(self.segment_domain.end_point())
			.map(to_bezier)
	}

	/// Construct a [`bezier_rs::Bezier`] curve from an iterator of segments with (handles, start point, end point). Returns None if any ids are invalid or if the segments are not continuous.
	fn subpath_from_segments(&self, segments: impl Iterator<Item = (bezier_rs::BezierHandles, usize, usize)>) -> Option<bezier_rs::Subpath<PointId>> {
		let mut first_point = None;
		let mut groups = Vec::new();
		let mut last: Option<(usize, bezier_rs::BezierHandles)> = None;

		for (handle, start, end) in segments {
			if last.is_some_and(|(previous_end, _)| previous_end != start) {
				warn!("subpath_from_segments that were not continuous");
				return None;
			}
			first_point = Some(first_point.unwrap_or(start));

			groups.push(bezier_rs::ManipulatorGroup {
				anchor: self.point_domain.positions()[start],
				in_handle: last.and_then(|(_, handle)| handle.end()),
				out_handle: handle.start(),
				id: self.point_domain.ids()[start],
			});

			last = Some((end, handle));
		}

		let closed = groups.len() > 1 && last.map(|(point, _)| point) == first_point;

		if let Some((end, last_handle)) = last {
			if closed {
				groups[0].in_handle = last_handle.end();
			} else {
				groups.push(bezier_rs::ManipulatorGroup {
					anchor: self.point_domain.positions()[end],
					in_handle: last_handle.end(),
					out_handle: None,
					id: self.point_domain.ids()[end],
				});
			}
		}
		Some(bezier_rs::Subpath::new(groups, closed))
	}

	/// Construct a [`bezier_rs::Bezier`] curve for each region, skipping invalid regions.
	pub fn region_bezier_paths(&self) -> impl Iterator<Item = (RegionId, bezier_rs::Subpath<PointId>)> + '_ {
		self.region_domain
			.id
			.iter()
			.zip(&self.region_domain.segment_range)
			.filter_map(|(&id, segment_range)| self.segment_domain.resolve_range(segment_range).map(|range| (id, range)))
			.filter_map(|(id, range)| {
				let segments_iter = self
					.segment_domain
					.handles
					.get(range.clone())?
					.iter()
					.zip(self.segment_domain.start_point.get(range.clone())?)
					.zip(self.segment_domain.end_point.get(range)?)
					.map(|((&handles, &start), &end)| (handles, start, end));

				self.subpath_from_segments(segments_iter).map(|subpath| (id, subpath))
			})
	}

	/// Construct a [`bezier_rs::Bezier`] curve for stroke.
	pub fn stroke_bezier_paths(&self) -> StrokePathIter<'_> {
		let mut points = vec![StrokePathIterPointMetadata::default(); self.point_domain.ids().len()];
		for (segment_index, (&start, &end)) in self.segment_domain.start_point.iter().zip(&self.segment_domain.end_point).enumerate() {
			points[start].set(StrokePathIterPointSegmentMetadata::new(segment_index, false));
			points[end].set(StrokePathIterPointSegmentMetadata::new(segment_index, true));
		}

		StrokePathIter {
			vector_data: self,
			points,
			skip: 0,
			done_one: false,
		}
	}

	/// Construct an iterator [`bezier_rs::ManipulatorGroup`] for stroke.
	pub fn manipulator_groups(&self) -> impl Iterator<Item = bezier_rs::ManipulatorGroup<PointId>> + '_ {
		self.stroke_bezier_paths().flat_map(|mut path| std::mem::take(path.manipulator_groups_mut()))
	}

	/// Get manipulator by id
	pub fn manipulator_group_id(&self, id: impl Into<PointId>) -> Option<bezier_rs::ManipulatorGroup<PointId>> {
		let id = id.into();
		self.manipulator_groups().find(|group| group.id == id)
	}

	/// Transforms this vector data
	pub fn transform(&mut self, transform: DAffine2) {
		self.point_domain.transform(transform);
		self.segment_domain.transform(transform);
	}

	pub fn vector_new_ids_from_hash(&mut self, node_id: u64) {
		let point_map = self.point_domain.ids().iter().map(|&old| (old, old.generate_from_hash(node_id))).collect::<HashMap<_, _>>();
		let segment_map = self.segment_domain.ids().iter().map(|&old| (old, old.generate_from_hash(node_id))).collect::<HashMap<_, _>>();
		let region_map = self.region_domain.ids().iter().map(|&old| (old, old.generate_from_hash(node_id))).collect::<HashMap<_, _>>();

		let id_map = IdMap {
			point_offset: self.point_domain.ids().len(),
			point_map,
			segment_map,
			region_map,
		};

		self.point_domain.map_ids(&id_map);
		self.segment_domain.map_ids(&id_map);
		self.region_domain.map_ids(&id_map);
	}
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct StrokePathIterPointSegmentMetadata {
	segment_index: usize,
	start_from_end: bool,
}

impl StrokePathIterPointSegmentMetadata {
	#[must_use]
	const fn new(segment_index: usize, start_from_end: bool) -> Self {
		Self { segment_index, start_from_end }
	}
	#[must_use]
	const fn flipped(&self) -> Self {
		Self {
			segment_index: self.segment_index,
			start_from_end: !self.start_from_end,
		}
	}
}

#[derive(Clone, Default)]
struct StrokePathIterPointMetadata(tinyvec::TinyVec<[StrokePathIterPointSegmentMetadata; 2]>);

impl StrokePathIterPointMetadata {
	fn set(&mut self, value: StrokePathIterPointSegmentMetadata) {
		self.0.insert(0, value);
	}
	#[must_use]
	fn connected(&self) -> usize {
		self.0.len()
	}
	#[must_use]
	fn take_first(&mut self) -> Option<StrokePathIterPointSegmentMetadata> {
		self.0.pop()
	}
	fn take_eq(&mut self, target: StrokePathIterPointSegmentMetadata) -> bool {
		let has_taken = self.0.contains(&target);
		self.0.retain(|value| *value != target);
		has_taken
	}
}

#[derive(Clone)]
pub struct StrokePathIter<'a> {
	vector_data: &'a VectorData,
	points: Vec<StrokePathIterPointMetadata>,
	skip: usize,
	done_one: bool,
}

impl Iterator for StrokePathIter<'_> {
	type Item = bezier_rs::Subpath<PointId>;

	fn next(&mut self) -> Option<Self::Item> {
		let current_start = if let Some((index, _)) = self.points.iter().enumerate().skip(self.skip).find(|(_, val)| val.connected() == 1) {
			index
		} else {
			if !self.done_one {
				self.done_one = true;
				self.skip = 0;
			}
			self.points.iter().enumerate().skip(self.skip).find(|(_, val)| val.connected() > 0)?.0
		};
		self.skip = current_start + 1;

		// There will always be one (seeing as we checked above)
		let mut point_index = current_start;
		let mut groups = Vec::new();
		let mut in_handle = None;
		let mut closed = false;
		loop {
			let Some(val) = self.points[point_index].take_first() else {
				// Dead end
				groups.push(bezier_rs::ManipulatorGroup {
					anchor: self.vector_data.point_domain.positions()[point_index],
					in_handle,
					out_handle: None,
					id: self.vector_data.point_domain.ids()[point_index],
				});

				break;
			};

			let mut handles = self.vector_data.segment_domain.handles()[val.segment_index];
			if val.start_from_end {
				handles = handles.reversed();
			}
			let next_point_index = if val.start_from_end {
				self.vector_data.segment_domain.start_point()[val.segment_index]
			} else {
				self.vector_data.segment_domain.end_point()[val.segment_index]
			};
			groups.push(bezier_rs::ManipulatorGroup {
				anchor: self.vector_data.point_domain.positions()[point_index],
				in_handle,
				out_handle: handles.start(),
				id: self.vector_data.point_domain.ids()[point_index],
			});

			in_handle = handles.end();

			point_index = next_point_index;
			self.points[next_point_index].take_eq(val.flipped());
			if next_point_index == current_start {
				closed = true;
				groups[0].in_handle = in_handle;
				break;
			}
		}

		Some(bezier_rs::Subpath::new(groups, closed))
	}
}

impl bezier_rs::Identifier for PointId {
	fn new() -> Self {
		Self::generate()
	}
}

/// Represents the conversion of ids used when concatenating vector data with conflicting ids.
pub struct IdMap {
	pub point_offset: usize,
	pub point_map: HashMap<PointId, PointId>,
	pub segment_map: HashMap<SegmentId, SegmentId>,
	pub region_map: HashMap<RegionId, RegionId>,
}
