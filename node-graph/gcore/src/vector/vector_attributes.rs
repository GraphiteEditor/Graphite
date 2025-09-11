use crate::subpath::{Bezier, BezierHandles, Identifier, ManipulatorGroup, Subpath};
use crate::vector::misc::{HandleId, dvec2_to_point};
use crate::vector::vector_types::Vector;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use kurbo::{CubicBez, Line, PathSeg, QuadBez};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::iter::zip;

/// A simple macro for creating strongly typed ids (to avoid confusion when passing around ids).
macro_rules! create_ids {
	($($id:ident),*) => {
		$(
			#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, DynAny)]
			#[derive(serde::Serialize, serde::Deserialize)]
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
			}
		)*
	};
}

create_ids! { PointId, SegmentId, RegionId, StrokeId, FillId }

/// A no-op hasher that allows writing u64s (the id type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoHash(Option<u64>);

impl Hasher for NoHash {
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

impl std::hash::BuildHasher for NoHashBuilder {
	type Hasher = NoHash;
	fn build_hasher(&self) -> Self::Hasher {
		NoHash::default()
	}
}

#[derive(Clone, Debug, Default, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
/// Stores data which is per-point. Each point is merely a position and can be used in a point cloud or to for a bézier path. In future this will be extendable at runtime with custom attributes.
pub struct PointDomain {
	id: Vec<PointId>,
	#[serde(alias = "positions")]
	pub(crate) position: Vec<DVec2>,
}

impl Hash for PointDomain {
	fn hash<H: Hasher>(&self, state: &mut H) {
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
		if self.id.contains(&id) {
			return;
		}

		self.id.push(id);
		self.position.push(position);
	}

	pub fn push_unchecked(&mut self, id: PointId, position: DVec2) {
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

	pub fn len(&self) -> usize {
		self.id.len()
	}

	pub fn is_empty(&self) -> bool {
		self.id.is_empty()
	}

	/// Iterate over point IDs and positions
	pub fn iter(&self) -> impl Iterator<Item = (PointId, DVec2)> + '_ {
		self.ids().iter().copied().zip(self.positions().iter().copied())
	}
}

#[derive(Clone, Debug, Default, PartialEq, Hash, DynAny, serde::Serialize, serde::Deserialize)]
/// Stores data which is per-segment. A segment is a bézier curve between two end points with a stroke. In future this will be extendable at runtime with custom attributes.
pub struct SegmentDomain {
	#[serde(alias = "ids")]
	id: Vec<SegmentId>,
	start_point: Vec<usize>,
	end_point: Vec<usize>,
	handles: Vec<BezierHandles>,
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

	pub fn handles(&self) -> &[BezierHandles] {
		&self.handles
	}

	pub fn stroke(&self) -> &[StrokeId] {
		&self.stroke
	}

	pub fn push(&mut self, id: SegmentId, start: usize, end: usize, handles: BezierHandles, stroke: StrokeId) {
		#[cfg(debug_assertions)]
		if self.id.contains(&id) {
			warn!("Tried to push an existing point to a point domain");
		}

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

	pub(crate) fn handles_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut BezierHandles, usize, usize)> {
		let nested = self.id.iter().zip(&mut self.handles).zip(&self.start_point).zip(&self.end_point);
		nested.map(|(((&a, b), &c), &d)| (a, b, c, d))
	}

	pub(crate) fn handles_and_points_mut(&mut self) -> impl Iterator<Item = (&mut BezierHandles, &mut usize, &mut usize)> {
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

	fn resolve_range(&self, range: &std::ops::RangeInclusive<SegmentId>) -> Option<std::ops::RangeInclusive<usize>> {
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

	/// Enumerate the number of segments connected to a point. If a segment starts and ends at a point then it is counted twice.
	pub(crate) fn any_connected(&self, point: usize) -> bool {
		self.all_connected(point).next().is_some()
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

	pub(crate) fn pair_handles_and_points_mut_by_index(&mut self, index1: usize, index2: usize) -> (&mut BezierHandles, &mut usize, &mut usize, &mut BezierHandles, &mut usize, &mut usize) {
		// Use split_at_mut to avoid multiple mutable borrows of the same slice
		let (handles_first, handles_second) = self.handles.split_at_mut(index2.max(index1));
		let (start_first, start_second) = self.start_point.split_at_mut(index2.max(index1));
		let (end_first, end_second) = self.end_point.split_at_mut(index2.max(index1));

		let (h1, h2) = if index1 < index2 {
			(&mut handles_first[index1], &mut handles_second[0])
		} else {
			(&mut handles_second[0], &mut handles_first[index2])
		};
		let (sp1, sp2) = if index1 < index2 {
			(&mut start_first[index1], &mut start_second[0])
		} else {
			(&mut start_second[0], &mut start_first[index2])
		};
		let (ep1, ep2) = if index1 < index2 {
			(&mut end_first[index1], &mut end_second[0])
		} else {
			(&mut end_second[0], &mut end_first[index2])
		};

		(h1, sp1, ep1, h2, sp2, ep2)
	}
}

#[derive(Clone, Debug, Default, PartialEq, Hash, DynAny, serde::Serialize, serde::Deserialize)]
/// Stores data which is per-region. A region is an enclosed area composed of a range of segments from the
/// [`SegmentDomain`] that can be given a fill. In future this will be extendable at runtime with custom attributes.
pub struct RegionDomain {
	#[serde(alias = "ids")]
	id: Vec<RegionId>,
	segment_range: Vec<std::ops::RangeInclusive<SegmentId>>,
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
	pub fn retain_with_region(&mut self, f: impl Fn(&RegionId, &std::ops::RangeInclusive<SegmentId>) -> bool) {
		let keep = self.id.iter().zip(self.segment_range.iter()).map(|(id, range)| f(id, range)).collect::<Vec<_>>();
		let mut iter = keep.iter().copied();
		self.segment_range.retain(|_| iter.next().unwrap());
		let mut iter = keep.iter().copied();
		self.fill.retain(|_| iter.next().unwrap());
		let mut iter = keep.iter().copied();
		self.id.retain(|_| iter.next().unwrap());
	}

	pub fn push(&mut self, id: RegionId, segment_range: std::ops::RangeInclusive<SegmentId>, fill: FillId) {
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

	pub fn segment_range_mut(&mut self) -> impl Iterator<Item = (RegionId, &mut std::ops::RangeInclusive<SegmentId>)> {
		self.id.iter().copied().zip(self.segment_range.iter_mut())
	}

	pub fn fill_mut(&mut self) -> impl Iterator<Item = (RegionId, &mut FillId)> {
		self.id.iter().copied().zip(self.fill.iter_mut())
	}

	pub fn ids(&self) -> &[RegionId] {
		&self.id
	}

	pub fn segment_range(&self) -> &[std::ops::RangeInclusive<SegmentId>] {
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
	pub fn iter(&self) -> impl Iterator<Item = (RegionId, std::ops::RangeInclusive<SegmentId>, FillId)> + '_ {
		let ids = self.id.iter().copied();
		let segment_range = self.segment_range.iter().cloned();
		let fill = self.fill.iter().copied();
		zip(ids, zip(segment_range, fill)).map(|(id, (segment_range, fill))| (id, segment_range, fill))
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HalfEdge {
	pub id: SegmentId,
	pub start: usize,
	pub end: usize,
	pub reverse: bool,
}

impl HalfEdge {
	pub fn new(id: SegmentId, start: usize, end: usize, reverse: bool) -> Self {
		Self { id, start, end, reverse }
	}

	pub fn reversed(&self) -> Self {
		Self {
			id: self.id,
			start: self.start,
			end: self.end,
			reverse: !self.reverse,
		}
	}

	pub fn normalize_direction(&self) -> Self {
		if self.reverse {
			Self {
				id: self.id,
				start: self.end,
				end: self.start,
				reverse: false,
			}
		} else {
			*self
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FoundSubpath {
	pub edges: Vec<HalfEdge>,
}

impl FoundSubpath {
	pub fn new(segments: Vec<HalfEdge>) -> Self {
		Self { edges: segments }
	}

	pub fn endpoints(&self) -> Option<(&HalfEdge, &HalfEdge)> {
		match (self.edges.first(), self.edges.last()) {
			(Some(first), Some(last)) => Some((first, last)),
			_ => None,
		}
	}

	pub fn push(&mut self, segment: HalfEdge) {
		self.edges.push(segment);
	}

	pub fn insert(&mut self, index: usize, segment: HalfEdge) {
		self.edges.insert(index, segment);
	}

	pub fn extend(&mut self, segments: impl IntoIterator<Item = HalfEdge>) {
		self.edges.extend(segments);
	}

	pub fn splice<I>(&mut self, range: std::ops::Range<usize>, replace_with: I)
	where
		I: IntoIterator<Item = HalfEdge>,
	{
		self.edges.splice(range, replace_with);
	}

	pub fn is_closed(&self) -> bool {
		match (self.edges.first(), self.edges.last()) {
			(Some(first), Some(last)) => first.start == last.end,
			_ => false,
		}
	}

	pub fn from_segment(segment: HalfEdge) -> Self {
		Self { edges: vec![segment] }
	}

	pub fn contains(&self, segment_id: SegmentId) -> bool {
		self.edges.iter().any(|s| s.id == segment_id)
	}
}

impl Vector {
	/// Construct a [`kurbo::PathSeg`] by resolving the points from their ids.
	fn path_segment_from_index(&self, start: usize, end: usize, handles: BezierHandles) -> PathSeg {
		let start = dvec2_to_point(self.point_domain.positions()[start]);
		let end = dvec2_to_point(self.point_domain.positions()[end]);

		match handles {
			BezierHandles::Linear => PathSeg::Line(Line::new(start, end)),
			BezierHandles::Quadratic { handle } => PathSeg::Quad(QuadBez::new(start, dvec2_to_point(handle), end)),
			BezierHandles::Cubic { handle_start, handle_end } => PathSeg::Cubic(CubicBez::new(start, dvec2_to_point(handle_start), dvec2_to_point(handle_end), end)),
		}
	}

	/// Construct a [`Bezier`] curve spanning from the resolved position of the start and end points with the specified handles.
	fn segment_to_bezier_with_index(&self, start: usize, end: usize, handles: BezierHandles) -> Bezier {
		let start = self.point_domain.positions()[start];
		let end = self.point_domain.positions()[end];
		Bezier { start, end, handles }
	}

	/// Tries to convert a segment with the specified id to a [`Bezier`], returning None if the id is invalid.
	pub fn segment_from_id(&self, id: SegmentId) -> Option<Bezier> {
		self.segment_points_from_id(id).map(|(_, _, bezier)| bezier)
	}

	/// Tries to convert a segment with the specified id to the start and end points and a [`Bezier`], returning None if the id is invalid.
	pub fn segment_points_from_id(&self, id: SegmentId) -> Option<(PointId, PointId, Bezier)> {
		Some(self.segment_points_from_index(self.segment_domain.id_to_index(id)?))
	}

	/// Tries to convert a segment with the specified index to the start and end points and a [`Bezier`].
	pub fn segment_points_from_index(&self, index: usize) -> (PointId, PointId, Bezier) {
		let start = self.segment_domain.start_point[index];
		let end = self.segment_domain.end_point[index];
		let start_id = self.point_domain.ids()[start];
		let end_id = self.point_domain.ids()[end];
		(start_id, end_id, self.segment_to_bezier_with_index(start, end, self.segment_domain.handles[index]))
	}

	/// Iterator over all of the [`Bezier`] following the order that they are stored in the segment domain, skipping invalid segments.
	pub fn segment_iter(&self) -> impl Iterator<Item = (SegmentId, PathSeg, PointId, PointId)> {
		let to_segment = |(((&handles, &id), &start), &end)| (id, self.path_segment_from_index(start, end, handles), self.point_domain.ids()[start], self.point_domain.ids()[end]);

		self.segment_domain
			.handles
			.iter()
			.zip(&self.segment_domain.id)
			.zip(self.segment_domain.start_point())
			.zip(self.segment_domain.end_point())
			.map(to_segment)
	}

	/// Iterator over all of the [`Bezier`] following the order that they are stored in the segment domain, skipping invalid segments.
	pub fn segment_bezier_iter(&self) -> impl Iterator<Item = (SegmentId, Bezier, PointId, PointId)> + '_ {
		let to_bezier = |(((&handles, &id), &start), &end)| (id, self.segment_to_bezier_with_index(start, end, handles), self.point_domain.ids()[start], self.point_domain.ids()[end]);
		self.segment_domain
			.handles
			.iter()
			.zip(&self.segment_domain.id)
			.zip(self.segment_domain.start_point())
			.zip(self.segment_domain.end_point())
			.map(to_bezier)
	}

	pub fn auto_join_paths(&self) -> Vec<FoundSubpath> {
		let segments = self.segment_domain.iter().map(|(id, start, end, _)| HalfEdge::new(id, start, end, false));

		let mut paths: Vec<FoundSubpath> = Vec::new();
		let mut current_path: Option<&mut FoundSubpath> = None;
		let mut previous: Option<(usize, usize)> = None;

		// First pass. Generates subpaths from continuous segments.
		for seg_ref in segments {
			let (start, end) = (seg_ref.start, seg_ref.end);

			if previous.is_some_and(|(_, prev_end)| start == prev_end) {
				if let Some(path) = current_path.as_mut() {
					path.push(seg_ref);
				}
			} else {
				paths.push(FoundSubpath::from_segment(seg_ref));
				current_path = paths.last_mut();
			}

			previous = Some((start, end));
		}

		// Second pass. Try to join paths together.
		let mut joined_paths = Vec::new();

		loop {
			let mut prev_index: Option<usize> = None;
			let original_len = paths.len();

			for current in paths.into_iter() {
				// If there's no previous subpath, start a new one
				if prev_index.is_none() {
					joined_paths.push(current);
					prev_index = Some(joined_paths.len() - 1);
					continue;
				}

				let prev = &mut joined_paths[prev_index.unwrap()];

				// Compare segment connections
				let (prev_first, prev_last) = prev.endpoints().unwrap();
				let (cur_first, cur_last) = current.endpoints().unwrap();

				// Join paths if the endpoints connect
				if prev_last.end == cur_first.start {
					prev.edges.extend(current.edges.into_iter().map(|s| s.normalize_direction()));
				} else if prev_first.start == cur_last.end {
					prev.edges.splice(0..0, current.edges.into_iter().rev().map(|s| s.normalize_direction()));
				} else if prev_last.end == cur_last.end {
					prev.edges.extend(current.edges.into_iter().rev().map(|s| s.reversed().normalize_direction()));
				} else if prev_first.start == cur_first.start {
					prev.edges.splice(0..0, current.edges.into_iter().map(|s| s.reversed().normalize_direction()));
				} else {
					// If not connected, start a new subpath
					joined_paths.push(current);
					prev_index = Some(joined_paths.len() - 1);
				}
			}

			// If no paths were joined in this pass, we're done
			if joined_paths.len() == original_len {
				return joined_paths;
			}

			// Repeat pass with newly joined paths
			paths = joined_paths;
			joined_paths = Vec::new();
		}
	}

	/// Construct a [`Bezier`] curve from an iterator of segments with (handles, start point, end point) independently of discontinuities.
	pub fn subpath_from_segments_ignore_discontinuities(&self, segments: impl Iterator<Item = (BezierHandles, usize, usize)>) -> Option<Subpath<PointId>> {
		let mut first_point = None;
		let mut manipulators_list = Vec::new();
		let mut last: Option<(usize, BezierHandles)> = None;

		for (handle, start, end) in segments {
			first_point = Some(first_point.unwrap_or(start));

			manipulators_list.push(ManipulatorGroup {
				anchor: self.point_domain.positions()[start],
				in_handle: last.and_then(|(_, handle)| handle.end()),
				out_handle: handle.start(),
				id: self.point_domain.ids()[start],
			});

			last = Some((end, handle));
		}

		let closed = manipulators_list.len() > 1 && last.map(|(point, _)| point) == first_point;

		if let Some((end, last_handle)) = last {
			if closed {
				manipulators_list[0].in_handle = last_handle.end();
			} else {
				manipulators_list.push(ManipulatorGroup {
					anchor: self.point_domain.positions()[end],
					in_handle: last_handle.end(),
					out_handle: None,
					id: self.point_domain.ids()[end],
				});
			}
		}

		Some(Subpath::new(manipulators_list, closed))
	}

	/// Construct a [`Bezier`] curve for each region, skipping invalid regions.
	pub fn region_manipulator_groups(&self) -> impl Iterator<Item = (RegionId, Vec<ManipulatorGroup<PointId>>)> + '_ {
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

				let mut manipulator_groups = Vec::new();
				let mut in_handle = None;

				for segment in segments_iter {
					let (handles, start_point_index, _end_point_index) = segment;
					let start_point_id = self.point_domain.id[start_point_index];
					let start_point = self.point_domain.position[start_point_index];

					let (manipulator_group, next_in_handle) = match handles {
						BezierHandles::Linear => (ManipulatorGroup::new_with_id(start_point, in_handle, None, start_point_id), None),
						BezierHandles::Quadratic { handle } => (ManipulatorGroup::new_with_id(start_point, in_handle, Some(handle), start_point_id), None),
						BezierHandles::Cubic { handle_start, handle_end } => (ManipulatorGroup::new_with_id(start_point, in_handle, Some(handle_start), start_point_id), Some(handle_end)),
					};

					in_handle = next_in_handle;
					manipulator_groups.push(manipulator_group);
				}

				if let Some(first) = manipulator_groups.first_mut() {
					first.in_handle = in_handle;
				}

				Some((id, manipulator_groups))
			})
	}

	pub fn build_stroke_path_iter(&self) -> StrokePathIter<'_> {
		let mut points = vec![StrokePathIterPointMetadata::default(); self.point_domain.ids().len()];
		for (segment_index, (&start, &end)) in self.segment_domain.start_point.iter().zip(&self.segment_domain.end_point).enumerate() {
			points[start].set(StrokePathIterPointSegmentMetadata::new(segment_index, false));
			points[end].set(StrokePathIterPointSegmentMetadata::new(segment_index, true));
		}

		StrokePathIter {
			vector: self,
			points,
			skip: 0,
			done_one: false,
		}
	}

	/// Construct a [`Bezier`] curve for stroke.
	pub fn stroke_bezier_paths(&self) -> impl Iterator<Item = Subpath<PointId>> {
		self.build_stroke_path_iter().map(|(manipulators_list, closed)| Subpath::new(manipulators_list, closed))
	}

	/// Construct and return an iterator of Vec of `(ManipulatorGroup<PointId>], bool)` for stroke.
	/// The boolean in the tuple indicates if the path is closed.
	pub fn stroke_manipulator_groups(&self) -> impl Iterator<Item = (Vec<ManipulatorGroup<PointId>>, bool)> {
		self.build_stroke_path_iter()
	}

	/// Construct a [`kurbo::BezPath`] curve for stroke.
	pub fn stroke_bezpath_iter(&self) -> impl Iterator<Item = kurbo::BezPath> {
		self.build_stroke_path_iter().map(|(manipulators_list, closed)| {
			let mut bezpath = kurbo::BezPath::new();
			let mut out_handle;

			let Some(first) = manipulators_list.first() else { return bezpath };
			bezpath.move_to(dvec2_to_point(first.anchor));
			out_handle = first.out_handle;

			for manipulator in manipulators_list.iter().skip(1) {
				match (out_handle, manipulator.in_handle) {
					(Some(handle_start), Some(handle_end)) => bezpath.curve_to(dvec2_to_point(handle_start), dvec2_to_point(handle_end), dvec2_to_point(manipulator.anchor)),
					(None, None) => bezpath.line_to(dvec2_to_point(manipulator.anchor)),
					(None, Some(handle)) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(manipulator.anchor)),
					(Some(handle), None) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(manipulator.anchor)),
				}
				out_handle = manipulator.out_handle;
			}

			if closed {
				match (out_handle, first.in_handle) {
					(Some(handle_start), Some(handle_end)) => bezpath.curve_to(dvec2_to_point(handle_start), dvec2_to_point(handle_end), dvec2_to_point(first.anchor)),
					(None, None) => bezpath.line_to(dvec2_to_point(first.anchor)),
					(None, Some(handle)) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(first.anchor)),
					(Some(handle), None) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(first.anchor)),
				}
				bezpath.close_path();
			}
			bezpath
		})
	}

	/// Construct an iterator [`ManipulatorGroup`] for stroke.
	pub fn manipulator_groups(&self) -> impl Iterator<Item = ManipulatorGroup<PointId>> + '_ {
		self.stroke_bezier_paths().flat_map(|mut path| std::mem::take(path.manipulator_groups_mut()))
	}

	pub fn manipulator_group_id(&self, id: impl Into<PointId>) -> Option<ManipulatorGroup<PointId>> {
		let id = id.into();
		self.manipulator_groups().find(|manipulators| manipulators.id == id)
	}

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
	vector: &'a Vector,
	points: Vec<StrokePathIterPointMetadata>,
	skip: usize,
	done_one: bool,
}

impl Iterator for StrokePathIter<'_> {
	type Item = (Vec<ManipulatorGroup<PointId>>, bool);

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
		let mut manipulators_list = Vec::new();
		let mut in_handle = None;
		let mut closed = false;
		loop {
			let Some(val) = self.points[point_index].take_first() else {
				// Dead end
				manipulators_list.push(ManipulatorGroup {
					anchor: self.vector.point_domain.positions()[point_index],
					in_handle,
					out_handle: None,
					id: self.vector.point_domain.ids()[point_index],
				});

				break;
			};

			let mut handles = self.vector.segment_domain.handles()[val.segment_index];
			if val.start_from_end {
				handles = handles.reversed();
			}
			let next_point_index = if val.start_from_end {
				self.vector.segment_domain.start_point()[val.segment_index]
			} else {
				self.vector.segment_domain.end_point()[val.segment_index]
			};
			manipulators_list.push(ManipulatorGroup {
				anchor: self.vector.point_domain.positions()[point_index],
				in_handle,
				out_handle: handles.start(),
				id: self.vector.point_domain.ids()[point_index],
			});

			in_handle = handles.end();

			point_index = next_point_index;
			self.points[next_point_index].take_eq(val.flipped());
			if next_point_index == current_start {
				closed = true;
				manipulators_list[0].in_handle = in_handle;
				break;
			}
		}

		Some((manipulators_list, closed))
	}
}

impl Identifier for PointId {
	fn new() -> Self {
		Self::generate()
	}
}

/// Represents the conversion of IDs used when concatenating vector paths with conflicting IDs.
pub struct IdMap {
	pub point_offset: usize,
	pub point_map: HashMap<PointId, PointId>,
	pub segment_map: HashMap<SegmentId, SegmentId>,
	pub region_map: HashMap<RegionId, RegionId>,
}
