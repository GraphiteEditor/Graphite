use super::HandleId;

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
			}
		)*
	};
}

create_ids! { PointId, SegmentId, RegionId, StrokeId, FillId }

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
	positions: Vec<DVec2>,
}

impl core::hash::Hash for PointDomain {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
		self.positions.iter().for_each(|pos| pos.to_array().map(|v| v.to_bits()).hash(state));
	}
}

impl PointDomain {
	pub const fn new() -> Self {
		Self {
			id: Vec::new(),
			positions: Vec::new(),
		}
	}

	pub fn clear(&mut self) {
		self.id.clear();
		self.positions.clear();
	}

	pub fn retain(&mut self, segment_domain: &mut SegmentDomain, f: impl Fn(&PointId) -> bool) {
		let mut keep = self.id.iter().map(&f);
		self.positions.retain(|_| keep.next().unwrap_or_default());

		// TODO(TrueDoctor): Consider using a prefix sum to avoid this Vec allocation (https://github.com/GraphiteEditor/Graphite/pull/1949#discussion_r1741711562)
		let mut id_map = Vec::with_capacity(self.ids().len());
		let mut new_index = 0;
		for id in self.ids() {
			if f(id) {
				id_map.push(new_index);
				new_index += 1;
			} else {
				id_map.push(usize::MAX); // A placeholder for invalid ids. This is checked after the segment domain is modified.
			}
		}

		let update_index = |index: &mut usize| *index = id_map[*index];
		segment_domain.start_point.iter_mut().for_each(update_index);
		segment_domain.end_point.iter_mut().for_each(update_index);

		self.id.retain(f);
	}

	pub fn push(&mut self, id: PointId, position: DVec2) {
		if self.id.contains(&id) {
			warn!("Duplicate point");
			return;
		}
		self.id.push(id);
		self.positions.push(position);
	}

	pub fn positions(&self) -> &[DVec2] {
		&self.positions
	}

	pub fn positions_mut(&mut self) -> impl Iterator<Item = (PointId, &mut DVec2)> {
		self.id.iter().copied().zip(self.positions.iter_mut())
	}

	pub fn set_position(&mut self, index: usize, position: DVec2) {
		self.positions[index] = position;
	}

	pub fn ids(&self) -> &[PointId] {
		&self.id
	}

	pub fn next_id(&self) -> PointId {
		self.ids().iter().copied().max_by(|a, b| a.0.cmp(&b.0)).map(|mut id| id.next_id()).unwrap_or(PointId::ZERO)
	}

	#[track_caller]
	pub fn position_from_id(&self, id: PointId) -> Option<DVec2> {
		let pos = self.resolve_id(id).map(|index| self.positions[index]);
		if pos.is_none() {
			warn!("Resolving pos of invalid id");
		}
		pos
	}

	pub(crate) fn resolve_id(&self, id: PointId) -> Option<usize> {
		self.id.iter().position(|&check_id| check_id == id)
	}

	fn concat(&mut self, other: &Self, transform: DAffine2, id_map: &IdMap) {
		self.id.extend(other.id.iter().map(|id| *id_map.point_map.get(id).unwrap_or(id)));
		self.positions.extend(other.positions.iter().map(|&pos| transform.transform_point2(pos)));
	}

	fn map_ids(&mut self, id_map: &IdMap) {
		self.id.iter_mut().for_each(|id| *id = *id_map.point_map.get(id).unwrap_or(id));
	}

	fn transform(&mut self, transform: DAffine2) {
		for pos in &mut self.positions {
			*pos = transform.transform_point2(*pos);
		}
	}
}

#[derive(Clone, Debug, Default, PartialEq, Hash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Stores data which is per-segment. A segment is a bézier curve between two end points with a stroke. In future this will be extendable at runtime with custom attributes.
pub struct SegmentDomain {
	ids: Vec<SegmentId>,
	start_point: Vec<usize>,
	end_point: Vec<usize>,
	handles: Vec<bezier_rs::BezierHandles>,
	stroke: Vec<StrokeId>,
}

impl SegmentDomain {
	pub const fn new() -> Self {
		Self {
			ids: Vec::new(),
			start_point: Vec::new(),
			end_point: Vec::new(),
			handles: Vec::new(),
			stroke: Vec::new(),
		}
	}

	pub fn clear(&mut self) {
		self.ids.clear();
		self.start_point.clear();
		self.end_point.clear();
		self.handles.clear();
		self.stroke.clear();
	}

	pub fn retain(&mut self, f: impl Fn(&SegmentId) -> bool, points_length: usize) {
		let additional_delete_ids = self
			.ids
			.iter()
			.zip(&self.start_point)
			.zip(&self.end_point)
			.filter(|((_, start), end)| **start >= points_length || **end >= points_length)
			.map(|x| *x.0 .0)
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

		let mut keep = self.ids.iter().map(can_delete());
		self.start_point.retain(|_| keep.next().unwrap_or_default());
		let mut keep = self.ids.iter().map(can_delete());
		self.end_point.retain(|_| keep.next().unwrap_or_default());
		let mut keep = self.ids.iter().map(can_delete());
		self.handles.retain(|_| keep.next().unwrap_or_default());
		let mut keep = self.ids.iter().map(can_delete());
		self.stroke.retain(|_| keep.next().unwrap_or_default());

		let mut delete_iter = additional_delete_ids.iter().peekable();
		self.ids.retain(move |id| {
			if delete_iter.peek() == Some(&id) {
				delete_iter.next();
				false
			} else {
				f(id)
			}
		});
	}

	pub fn ids(&self) -> &[SegmentId] {
		&self.ids
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
		if self.ids.contains(&id) {
			return;
		}
		// Attempt to keep line joins?
		let after = self.end_point.iter().copied().position(|other_end| other_end == start);
		let before = self.start_point.iter().copied().position(|other_start| other_start == end);
		let index = match (before, after) {
			(_, Some(after)) => after + 1,
			(Some(before), _) => before,
			(None, None) => self.ids.len(),
		};
		self.ids.insert(index, id);
		self.start_point.insert(index, start);
		self.end_point.insert(index, end);
		self.handles.insert(index, handles);
		self.stroke.insert(index, stroke);
	}

	pub(crate) fn start_point_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut usize)> {
		self.ids.iter().copied().zip(self.start_point.iter_mut())
	}

	pub(crate) fn end_point_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut usize)> {
		self.ids.iter().copied().zip(self.end_point.iter_mut())
	}

	pub(crate) fn handles_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut bezier_rs::BezierHandles, usize, usize)> {
		let nested = self.ids.iter().zip(&mut self.handles).zip(&self.start_point).zip(&self.end_point);
		nested.map(|(((&a, b), &c), &d)| (a, b, c, d))
	}

	pub(crate) fn handles_and_points_mut(&mut self) -> impl Iterator<Item = (&mut bezier_rs::BezierHandles, &mut usize, &mut usize)> {
		let nested = self.handles.iter_mut().zip(&mut self.start_point).zip(&mut self.end_point);
		nested.map(|((a, b), c)| (a, b, c))
	}

	pub fn stroke_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut StrokeId)> {
		self.ids.iter().copied().zip(self.stroke.iter_mut())
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

	fn id_to_index(&self, id: SegmentId) -> Option<usize> {
		debug_assert_eq!(self.ids.len(), self.handles.len());
		debug_assert_eq!(self.ids.len(), self.start_point.len());
		debug_assert_eq!(self.ids.len(), self.end_point.len());
		self.ids.iter().position(|&check_id| check_id == id)
	}

	fn resolve_range(&self, range: &core::ops::RangeInclusive<SegmentId>) -> Option<core::ops::RangeInclusive<usize>> {
		match (self.id_to_index(*range.start()), self.id_to_index(*range.end())) {
			(Some(start), Some(end)) if start.max(end) < self.handles.len().min(self.ids.len()).min(self.start_point.len()).min(self.end_point.len()) => Some(start..=end),
			_ => {
				warn!("Resolving range with invalid id");
				None
			}
		}
	}

	fn concat(&mut self, other: &Self, transform: DAffine2, id_map: &IdMap) {
		self.ids.extend(other.ids.iter().map(|id| *id_map.segment_map.get(id).unwrap_or(id)));
		self.start_point.extend(other.start_point.iter().map(|&index| id_map.point_offset + index));
		self.end_point.extend(other.end_point.iter().map(|&index| id_map.point_offset + index));
		self.handles.extend(other.handles.iter().map(|handles| handles.apply_transformation(|p| transform.transform_point2(p))));
		self.stroke.extend(&other.stroke);
	}

	fn map_ids(&mut self, id_map: &IdMap) {
		self.ids.iter_mut().for_each(|id| *id = *id_map.segment_map.get(id).unwrap_or(id));
	}

	fn transform(&mut self, transform: DAffine2) {
		for handles in &mut self.handles {
			*handles = handles.apply_transformation(|p| transform.transform_point2(p));
		}
	}

	/// Enumerate all segments that start at the point.
	pub(crate) fn start_connected(&self, point: usize) -> impl Iterator<Item = SegmentId> + '_ {
		self.start_point.iter().zip(&self.ids).filter(move |&(&found_point, _)| found_point == point).map(|(_, &seg)| seg)
	}

	/// Enumerate all segments that end at the point.
	pub(crate) fn end_connected(&self, point: usize) -> impl Iterator<Item = SegmentId> + '_ {
		self.end_point.iter().zip(&self.ids).filter(move |&(&found_point, _)| found_point == point).map(|(_, &seg)| seg)
	}

	/// Enumerate all segments that start or end at a point, converting them to [`HandleId`s]. Note that the handles may not exist e.g. for a linear segment.
	pub(crate) fn all_connected(&self, point: usize) -> impl Iterator<Item = HandleId> + '_ {
		self.start_connected(point).map(HandleId::primary).chain(self.end_connected(point).map(HandleId::end))
	}

	/// Enumerate the number of segments connected to a point. If a segment starts and ends at a point then it is counted twice.
	pub(crate) fn connected_count(&self, point: usize) -> usize {
		self.all_connected(point).count()
	}
}

#[derive(Clone, Debug, Default, PartialEq, Hash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Stores data which is per-region. A region is an enclosed area composed of a range of segments from the [`SegmentDomain`] that can be given a fill. In future this will be extendable at runtime with custom attributes.
pub struct RegionDomain {
	ids: Vec<RegionId>,
	segment_range: Vec<core::ops::RangeInclusive<SegmentId>>,
	fill: Vec<FillId>,
}

impl RegionDomain {
	pub const fn new() -> Self {
		Self {
			ids: Vec::new(),
			segment_range: Vec::new(),
			fill: Vec::new(),
		}
	}

	pub fn clear(&mut self) {
		self.ids.clear();
		self.segment_range.clear();
		self.fill.clear();
	}

	pub fn retain(&mut self, f: impl Fn(&RegionId) -> bool) {
		let mut keep = self.ids.iter().map(&f);
		self.segment_range.retain(|_| keep.next().unwrap_or_default());
		let mut keep = self.ids.iter().map(&f);
		self.fill.retain(|_| keep.next().unwrap_or_default());
		self.ids.retain(&f);
	}

	pub fn push(&mut self, id: RegionId, segment_range: core::ops::RangeInclusive<SegmentId>, fill: FillId) {
		if self.ids.contains(&id) {
			warn!("Duplicate region");
			return;
		}
		self.ids.push(id);
		self.segment_range.push(segment_range);
		self.fill.push(fill);
	}

	fn _resolve_id(&self, id: RegionId) -> Option<usize> {
		self.ids.iter().position(|&check_id| check_id == id)
	}

	pub fn next_id(&self) -> RegionId {
		self.ids.iter().copied().max_by(|a, b| a.0.cmp(&b.0)).map(|mut id| id.next_id()).unwrap_or(RegionId::ZERO)
	}

	pub fn segment_range_mut(&mut self) -> impl Iterator<Item = (RegionId, &mut core::ops::RangeInclusive<SegmentId>)> {
		self.ids.iter().copied().zip(self.segment_range.iter_mut())
	}

	pub fn fill_mut(&mut self) -> impl Iterator<Item = (RegionId, &mut FillId)> {
		self.ids.iter().copied().zip(self.fill.iter_mut())
	}

	pub fn ids(&self) -> &[RegionId] {
		&self.ids
	}

	pub fn segment_range(&self) -> &[core::ops::RangeInclusive<SegmentId>] {
		&self.segment_range
	}

	pub fn fill(&self) -> &[FillId] {
		&self.fill
	}

	fn concat(&mut self, other: &Self, _transform: DAffine2, id_map: &IdMap) {
		self.ids.extend(other.ids.iter().map(|id| *id_map.region_map.get(id).unwrap_or(id)));
		self.segment_range.extend(
			other
				.segment_range
				.iter()
				.map(|range| *id_map.segment_map.get(range.start()).unwrap_or(range.start())..=*id_map.segment_map.get(range.end()).unwrap_or(range.end())),
		);
		self.fill.extend(&other.fill);
	}

	fn map_ids(&mut self, id_map: &IdMap) {
		self.ids.iter_mut().for_each(|id| *id = *id_map.region_map.get(id).unwrap_or(id));
		self.segment_range
			.iter_mut()
			.for_each(|range| *range = *id_map.segment_map.get(range.start()).unwrap_or(range.start())..=*id_map.segment_map.get(range.end()).unwrap_or(range.end()));
	}
}

impl super::VectorData {
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
			.zip(&self.segment_domain.ids)
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
			.ids
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
struct StrokePathIterPointMetadata([Option<StrokePathIterPointSegmentMetadata>; 2]);

impl StrokePathIterPointMetadata {
	fn set(&mut self, value: StrokePathIterPointSegmentMetadata) {
		if self.0[0].is_none() {
			self.0[0] = Some(value)
		} else if self.0[1].is_none() {
			self.0[1] = Some(value);
		} else {
			panic!("Mesh networks are not supported");
		}
	}
	#[must_use]
	fn connected(&self) -> usize {
		self.0.iter().filter(|val| val.is_some()).count()
	}
	#[must_use]
	fn take_first(&mut self) -> Option<StrokePathIterPointSegmentMetadata> {
		self.0[0].take().or_else(|| self.0[1].take())
	}
	fn take_eq(&mut self, target: StrokePathIterPointSegmentMetadata) -> bool {
		self.0[0].take_if(|&mut value| value == target).or_else(|| self.0[1].take_if(|&mut value| value == target)).is_some()
	}
}

#[derive(Clone)]
pub struct StrokePathIter<'a> {
	vector_data: &'a super::VectorData,
	points: Vec<StrokePathIterPointMetadata>,
	skip: usize,
	done_one: bool,
}

impl<'a> Iterator for StrokePathIter<'a> {
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

impl crate::vector::ConcatElement for super::VectorData {
	fn concat(&mut self, other: &Self, transform: glam::DAffine2, node_id: u64) {
		let new_ids = other
			.point_domain
			.id
			.iter()
			.filter(|id| self.point_domain.id.contains(id))
			.map(|&old| (old, old.generate_from_hash(node_id)));
		let point_map = new_ids.collect::<HashMap<_, _>>();
		let new_ids = other
			.segment_domain
			.ids
			.iter()
			.filter(|id| self.segment_domain.ids.contains(id))
			.map(|&old| (old, old.generate_from_hash(node_id)));
		let segment_map = new_ids.collect::<HashMap<_, _>>();
		let new_ids = other
			.region_domain
			.ids
			.iter()
			.filter(|id| self.region_domain.ids.contains(id))
			.map(|&old| (old, old.generate_from_hash(node_id)));
		let region_map = new_ids.collect::<HashMap<_, _>>();
		let id_map = IdMap {
			point_offset: self.point_domain.ids().len(),
			point_map,
			segment_map,
			region_map,
		};
		self.point_domain.concat(&other.point_domain, transform * other.transform, &id_map);
		self.segment_domain.concat(&other.segment_domain, transform * other.transform, &id_map);
		self.region_domain.concat(&other.region_domain, transform * other.transform, &id_map);
		// TODO: properly deal with fills such as gradients
		self.style = other.style.clone();
		self.colinear_manipulators.extend(other.colinear_manipulators.iter().copied());
		self.alpha_blending = other.alpha_blending;
	}
}

/// Represents the conversion of ids used when concatenating vector data with conflicting ids.
struct IdMap {
	point_offset: usize,
	point_map: HashMap<PointId, PointId>,
	segment_map: HashMap<SegmentId, SegmentId>,
	region_map: HashMap<RegionId, RegionId>,
}
