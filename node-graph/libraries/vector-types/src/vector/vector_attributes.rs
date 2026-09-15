use crate::vector::misc::{BezierHandles, HandleId, ManipulatorGroup, Tangent, dvec2_to_point};
use crate::vector::vector_types::Vector;
use dyn_any::DynAny;
use fixedbitset::FixedBitSet;
use glam::{DAffine2, DVec2};
use kurbo::{CubicBez, Line, ParamCurve, PathSeg, QuadBez};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::iter::zip;

/// A simple macro for creating strongly typed ids (to avoid confusion when passing around ids).
macro_rules! create_ids {
	($($id:ident),*) => {
		$(
			#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, graphene_hash::CacheHash, DynAny)]
			#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
			/// A strongly typed ID
			pub struct $id(u64);

			impl $id {
				pub const ZERO: $id = $id(0);

				/// Generate a new random id
				pub fn generate() -> Self {
					Self(core_types::uuid::generate_uuid())
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

create_ids! { PointId, SegmentId }

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

#[derive(Clone, Debug, Default, PartialEq, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Stores data which is per-point. Each point is merely a position and can be used in a point cloud or to for a bézier path. In future this will be extendable at runtime with custom attributes.
pub struct PointDomain {
	id: Vec<PointId>,
	#[cfg_attr(feature = "serde", serde(alias = "positions"))]
	pub(crate) position: Vec<DVec2>,
}

impl PointDomain {
	pub const fn new() -> Self {
		Self { id: Vec::new(), position: Vec::new() }
	}

	#[inline(always)]
	pub fn reserve(&mut self, additional: usize) {
		self.id.reserve(additional);
		self.position.reserve(additional);
	}

	pub(crate) fn retain(&mut self, segment_domain: &mut SegmentDomain, f: impl Fn(&PointId) -> bool) {
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
		#[cfg(debug_assertions)]
		if self.id.contains(&id) {
			warn!("Tried to push a duplicate point to a point domain");
			return;
		}

		self.push_unchecked(id, position);
	}

	#[inline(always)]
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

	pub fn resolve_id(&self, id: PointId) -> Option<usize> {
		self.id.iter().position(|&check_id| check_id == id)
	}

	pub(crate) fn concat(&mut self, other: &Self, transform: DAffine2, id_map: &IdMap) {
		self.id.extend(other.id.iter().map(|id| *id_map.point_map.get(id).unwrap_or(id)));
		self.position.extend(other.position.iter().map(|&pos| transform.transform_point2(pos)));
	}

	pub(crate) fn map_ids(&mut self, id_map: &IdMap) {
		self.id.iter_mut().for_each(|id| *id = *id_map.point_map.get(id).unwrap_or(id));
	}

	pub(crate) fn transform(&mut self, transform: DAffine2) {
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

#[derive(Clone, Debug, Default, PartialEq, graphene_hash::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Stores data which is per-segment. A segment is a bézier curve between two end points. In future this will be extendable at runtime with custom attributes.
pub struct SegmentDomain {
	#[cfg_attr(feature = "serde", serde(alias = "ids"))]
	id: Vec<SegmentId>,
	start_point: Vec<usize>,
	end_point: Vec<usize>,
	handles: Vec<BezierHandles>,
}

impl SegmentDomain {
	pub const fn new() -> Self {
		Self {
			id: Vec::new(),
			start_point: Vec::new(),
			end_point: Vec::new(),
			handles: Vec::new(),
		}
	}

	#[inline(always)]
	pub fn reserve(&mut self, additional: usize) {
		self.id.reserve(additional);
		self.start_point.reserve(additional);
		self.end_point.reserve(additional);
		self.handles.reserve(additional);
	}

	pub(crate) fn retain(&mut self, f: impl Fn(&SegmentId) -> bool, points_length: usize) {
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

	pub fn start_point(&self) -> &[usize] {
		&self.start_point
	}

	pub fn end_point(&self) -> &[usize] {
		&self.end_point
	}

	pub fn set_start_point(&mut self, segment_index: usize, new: usize) {
		self.start_point[segment_index] = new;
	}

	pub fn set_end_point(&mut self, segment_index: usize, new: usize) {
		self.end_point[segment_index] = new;
	}

	pub fn set_handles(&mut self, segment_index: usize, new: BezierHandles) {
		self.handles[segment_index] = new;
	}

	pub fn handles(&self) -> &[BezierHandles] {
		&self.handles
	}

	pub fn push(&mut self, id: SegmentId, start: usize, end: usize, handles: BezierHandles) {
		#[cfg(debug_assertions)]
		if self.id.contains(&id) {
			warn!("Tried to push a duplicate segment to a segment domain");
			return;
		}

		self.push_unchecked(id, start, end, handles);
	}

	#[inline(always)]
	pub fn push_unchecked(&mut self, id: SegmentId, start: usize, end: usize, handles: BezierHandles) {
		self.id.push(id);
		self.start_point.push(start);
		self.end_point.push(end);
		self.handles.push(handles);
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

	pub fn handles_and_points_mut(&mut self) -> impl Iterator<Item = (&mut BezierHandles, &mut usize, &mut usize)> {
		let nested = self.handles.iter_mut().zip(&mut self.start_point).zip(&mut self.end_point);
		nested.map(|((a, b), c)| (a, b, c))
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

	pub(crate) fn concat(&mut self, other: &Self, transform: DAffine2, id_map: &IdMap) {
		self.id.extend(other.id.iter().map(|id| *id_map.segment_map.get(id).unwrap_or(id)));
		self.start_point.extend(other.start_point.iter().map(|&index| id_map.point_offset + index));
		self.end_point.extend(other.end_point.iter().map(|&index| id_map.point_offset + index));
		self.handles.extend(other.handles.iter().map(|handles| handles.apply_transformation(|p| transform.transform_point2(p))));
	}

	pub(crate) fn map_ids(&mut self, id_map: &IdMap) {
		self.id.iter_mut().for_each(|id| *id = *id_map.segment_map.get(id).unwrap_or(id));
	}

	pub(crate) fn transform(&mut self, transform: DAffine2) {
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

	/// Computes the direction-of-travel tangent at one endpoint of a segment.
	/// Uses the "first distinct control point" pattern: iterates through the Bezier control points
	/// from the anchor outward, returning the direction to the first one that differs in position.
	/// This handles zero-length handles by finding the tangent direction in the limit.
	/// Returns `DVec2::ZERO` if all control points coincide (fully degenerate segment).
	fn segment_tangent_at_endpoint(&self, segment_index: usize, positions: &[DVec2], at_start: bool) -> DVec2 {
		let anchor_start = positions[self.start_point[segment_index]];
		let anchor_end = positions[self.end_point[segment_index]];

		// Build ordered control points for this segment
		let (points, count) = match self.handles[segment_index] {
			BezierHandles::Linear => ([anchor_start, anchor_end, DVec2::ZERO, DVec2::ZERO], 2),
			BezierHandles::Quadratic { handle } => ([anchor_start, handle, anchor_end, DVec2::ZERO], 3),
			BezierHandles::Cubic { handle_start, handle_end } => ([anchor_start, handle_start, handle_end, anchor_end], 4),
		};

		let not_near = |a: DVec2, b: DVec2| a.distance_squared(b) > f64::EPSILON * 1e3;

		if at_start {
			let anchor = points[0];
			points[1..count].iter().find(|&&p| not_near(p, anchor)).map_or(DVec2::ZERO, |&point| point - anchor)
		} else {
			let anchor = points[count - 1];
			points[..count - 1].iter().rev().find(|&&p| not_near(p, anchor)).map_or(DVec2::ZERO, |&point| anchor - point)
		}
	}

	/// Computes the average tangent direction at a point based on its 1 or 2 connected segments.
	/// Returns `None` for points with 0 or 3+ connections (ambiguous or undefined tangent),
	/// or if the tangent is degenerate (all control points coincide).
	pub fn point_tangent(&self, point_index: usize, positions: &[DVec2]) -> Option<DVec2> {
		// Collect connected segments with their relationship to this point (at_start flag)
		let mut connections: [(usize, bool); 2] = [(0, false); 2];
		let mut connection_count = 0;

		for (segment_index, (&start, &end)) in self.start_point.iter().zip(&self.end_point).enumerate() {
			// Self-loop segments count as two connections (outgoing and incoming)
			let is_start = start == point_index;
			let is_end = end == point_index;

			if !is_start && !is_end {
				continue;
			}

			if is_start {
				if connection_count >= 2 {
					return None;
				}
				connections[connection_count] = (segment_index, true);
				connection_count += 1;
			}
			if is_end {
				if connection_count >= 2 {
					return None;
				}
				connections[connection_count] = (segment_index, false);
				connection_count += 1;
			}
		}

		if connection_count == 0 {
			return None;
		}

		// Compute the direction-of-travel tangent for the first connection
		let (segment_index, at_start) = connections[0];
		let tangent1 = self.segment_tangent_at_endpoint(segment_index, positions, at_start).try_normalize();

		if connection_count == 1 {
			return tangent1;
		}

		// Compute the direction-of-travel tangent for the second connection
		let (segment_index, at_start) = connections[1];
		let tangent2 = self.segment_tangent_at_endpoint(segment_index, positions, at_start).try_normalize();

		// Average the two normalized tangents
		let average = tangent1? + tangent2?;

		// If the tangents are nearly opposite (straight-through), use t1 directly
		if average.length_squared() < (f64::EPSILON * 1e3).powi(2) {
			return tangent1;
		}

		average.try_normalize()
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

	pub fn pair_handles_and_points_mut_by_index(&mut self, index1: usize, index2: usize) -> (&mut BezierHandles, &mut usize, &mut usize, &mut BezierHandles, &mut usize, &mut usize) {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HalfEdge {
	pub id: SegmentId,
	pub start: usize,
	pub end: usize,
	pub reverse: bool,
}

impl HalfEdge {
	fn new(id: SegmentId, start: usize, end: usize, reverse: bool) -> Self {
		Self { id, start, end, reverse }
	}

	fn reversed(&self) -> Self {
		Self {
			id: self.id,
			start: self.start,
			end: self.end,
			reverse: !self.reverse,
		}
	}

	fn normalize_direction(&self) -> Self {
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
	fn endpoints(&self) -> Option<(&HalfEdge, &HalfEdge)> {
		match (self.edges.first(), self.edges.last()) {
			(Some(first), Some(last)) => Some((first, last)),
			_ => None,
		}
	}

	fn push(&mut self, segment: HalfEdge) {
		self.edges.push(segment);
	}

	pub fn is_closed(&self) -> bool {
		match (self.edges.first(), self.edges.last()) {
			(Some(first), Some(last)) => first.start == last.end,
			_ => false,
		}
	}

	fn from_segment(segment: HalfEdge) -> Self {
		Self { edges: vec![segment] }
	}

	pub fn contains(&self, segment_id: SegmentId) -> bool {
		self.edges.iter().any(|s| s.id == segment_id)
	}
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
struct FaceSide {
	segment_index: usize,
	reversed: bool,
}

impl FaceSide {
	/// The same segment walked in the opposite direction.
	fn mirrored(&self) -> Self {
		Self {
			segment_index: self.segment_index,
			reversed: !self.reversed,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FaceSideSet {
	set: FixedBitSet,
}
impl FaceSideSet {
	fn new(size: usize) -> Self {
		Self {
			set: FixedBitSet::with_capacity(size * 2),
		}
	}

	fn index_of(side: &FaceSide) -> usize {
		(side.segment_index << 1) | (side.reversed as usize)
	}

	fn index(&self, side: FaceSide) -> usize {
		Self::index_of(&side)
	}

	fn insert(&mut self, side: FaceSide) {
		self.set.insert(self.index(side));
	}

	fn remove(&mut self, side: FaceSide) {
		self.set.set(self.index(side), false);
	}

	fn contains(&self, side: FaceSide) -> bool {
		self.set.contains(self.index(side))
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Faces {
	sides: Vec<FaceSide>,
	face_start: Vec<usize>,
}

/// Every face orbit of the segment graph: the flattened side lists plus each orbit's traced path,
/// signed area (bounded faces are walked clockwise so they come out negative, silhouettes positive),
/// and a lookup from any side to the orbit containing it.
struct FaceOrbits {
	faces: Faces,
	paths: Vec<kurbo::BezPath>,
	areas: Vec<f64>,
	side_to_orbit: Vec<usize>,
}

impl FaceOrbits {
	fn face_count(&self) -> usize {
		self.faces.face_start.len()
	}

	fn face_sides(&self, orbit_index: usize) -> &[FaceSide] {
		let start = self.faces.face_start[orbit_index];
		let end = self.faces.face_start.get(orbit_index + 1).copied().unwrap_or(self.faces.sides.len());
		&self.faces.sides[start..end]
	}
}

impl Faces {
	pub fn new() -> Self {
		Self {
			sides: Vec::new(),
			face_start: Vec::new(),
		}
	}
	pub fn add_side(&mut self, side: FaceSide) {
		self.sides.push(side);
	}
	pub fn start_new_face(&mut self) {
		self.face_start.push(self.sides.len());
	}
	pub fn backtrack(&mut self) {
		if let Some(last_start) = self.face_start.pop() {
			self.sides.truncate(last_start);
		}
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

	/// Tries to convert a segment with the specified id to a [`PathSeg`], returning None if the id is invalid.
	pub fn segment_from_id(&self, id: SegmentId) -> Option<PathSeg> {
		self.segment_points_from_id(id).map(|(_, _, segment)| segment)
	}

	/// Tries to convert a segment with the specified id to the start and end points and a [`PathSeg`], returning None if the id is invalid.
	pub fn segment_points_from_id(&self, id: SegmentId) -> Option<(PointId, PointId, PathSeg)> {
		Some(self.segment_points_from_index(self.segment_domain.id_to_index(id)?))
	}

	/// Converts a segment with the specified index to the start and end points and a [`PathSeg`].
	pub fn segment_points_from_index(&self, index: usize) -> (PointId, PointId, PathSeg) {
		let start = self.segment_domain.start_point[index];
		let end = self.segment_domain.end_point[index];
		let start_id = self.point_domain.ids()[start];
		let end_id = self.point_domain.ids()[end];
		(start_id, end_id, self.path_segment_from_index(start, end, self.segment_domain.handles[index]))
	}

	/// Iterator over all of the [`PathSeg`]s following the order that they are stored in the segment domain, skipping invalid segments.
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

	/// Construct a [`kurbo::BezPath`] from an iterator of segments with (handles, start point, end point) independently of discontinuities.
	pub fn bezpath_from_segments_ignore_discontinuities(&self, segments: impl Iterator<Item = (BezierHandles, usize, usize)>) -> Option<kurbo::BezPath> {
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

		Some(crate::vector::misc::bezpath_from_manipulator_groups(&manipulators_list, closed))
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

	/// Construct and return an iterator of `(Vec<ManipulatorGroup>, bool)` for each stroke.
	/// The boolean in the tuple indicates if the path is closed.
	pub fn stroke_manipulator_groups(&self) -> impl Iterator<Item = (Vec<ManipulatorGroup>, bool)> {
		self.build_stroke_path_iter()
	}

	/// Construct a [`kurbo::BezPath`] curve for stroke.
	pub fn stroke_bezpath_iter(&self) -> impl Iterator<Item = kurbo::BezPath> {
		self.build_stroke_path_iter()
			.map(|(manipulators_list, closed)| crate::vector::misc::bezpath_from_manipulator_groups(&manipulators_list, closed))
	}

	pub fn transform(&mut self, transform: DAffine2) {
		self.point_domain.transform(transform);
		self.segment_domain.transform(transform);
	}

	pub fn vector_new_ids_from_hash(&mut self, node_id: u64) {
		let point_map = self.point_domain.ids().iter().map(|&old| (old, old.generate_from_hash(node_id))).collect::<HashMap<_, _>>();
		let segment_map = self.segment_domain.ids().iter().map(|&old| (old, old.generate_from_hash(node_id))).collect::<HashMap<_, _>>();

		let id_map = IdMap {
			point_offset: self.point_domain.ids().len(),
			point_map,
			segment_map,
		};

		self.point_domain.map_ids(&id_map);
		self.segment_domain.map_ids(&id_map);
	}

	pub fn is_branching(&self) -> bool {
		// Tally segment endpoints per point in one `O(points + segments)` pass, short-circuiting once any point exceeds two
		let mut connected_count = vec![0_u8; self.point_domain.len()];
		for &point_index in self.segment_domain.start_point().iter().chain(self.segment_domain.end_point()) {
			connected_count[point_index] += 1;
			if connected_count[point_index] > 2 {
				return true;
			}
		}
		false
	}

	/// Determines if face-by-face fill rendering should be used. Branching vectors are meshes, whose
	/// bounded faces are found and filled individually rather than filling the stroke path directly.
	pub fn use_face_fill(&self) -> bool {
		self.is_branching()
	}

	/// Returns the fillable faces of the segment graph: bounded regions, skipping the unbounded
	/// silhouette orbits and any face classified as deliberate negative space by its boundary winding.
	/// Negative space loops are subtracted from any separately-bounded face covering them, so a
	/// reverse-wound contour still punches its hole when it shares no points with what surrounds it.
	pub fn construct_faces(&self) -> Vec<kurbo::BezPath> {
		let orbits = self.face_orbits();
		let (core_degree, ..) = self.two_core();

		// Split the bounded orbits (negative area) into fillable faces and deliberate negative space
		let mut kept_faces: Vec<(kurbo::BezPath, HashSet<usize>)> = Vec::new();
		let mut negative_regions: Vec<(kurbo::BezPath, HashSet<usize>)> = Vec::new();
		for orbit_index in 0..orbits.face_count() {
			if orbits.areas[orbit_index] > 0. {
				continue;
			}
			let sides = orbits.face_sides(orbit_index);

			let side_set: HashSet<usize> = sides.iter().map(FaceSideSet::index_of).collect();
			let non_spur_sides: Vec<FaceSide> = sides.iter().copied().filter(|side| !side_set.contains(&FaceSideSet::index_of(&side.mirrored()))).collect();
			if non_spur_sides.is_empty() {
				continue;
			}

			let path = orbits.paths[orbit_index].clone();
			let segment_set: HashSet<usize> = sides.iter().map(|side| side.segment_index).collect();
			if non_spur_sides.iter().any(|side| side.reversed) {
				kept_faces.push((path, segment_set));
				continue;
			}

			// An all-forward face is a reverse-wound loop's interior. It only counts as deliberate negative
			// space when it is a standalone simple loop or a single face surrounds it, the cases where its
			// contour was drawn as one unit. Walls shared among several faces are mesh and stay filled.
			let is_standalone_loop = non_spur_sides
				.iter()
				.all(|side| core_degree[self.segment_domain.start_point[side.segment_index]] == 2 && core_degree[self.segment_domain.end_point[side.segment_index]] == 2);
			let mut mirror_orbits = non_spur_sides.iter().map(|side| orbits.side_to_orbit[FaceSideSet::index_of(&side.mirrored())]);
			let first_mirror_orbit = mirror_orbits.next().unwrap_or(usize::MAX);
			let surrounded_by_one_face = first_mirror_orbit != usize::MAX && orbits.areas[first_mirror_orbit] <= 0. && mirror_orbits.all(|orbit| orbit == first_mirror_orbit);
			if is_standalone_loop || surrounded_by_one_face {
				negative_regions.push((path, segment_set));
			} else {
				kept_faces.push((path, segment_set));
			}
		}

		// Subtract each negative space loop from the kept faces that cover it. Faces sharing segments with the loop
		// already exclude it through their own boundary, and the winding test naturally skips faces it lies outside of.
		for (negative_path, negative_segments) in &negative_regions {
			// A segment midpoint is a safe winding sample, whereas snapping can place a vertex exactly on the covering contour
			let Some(first_segment) = negative_path.segments().next() else { continue };
			let sample = first_segment.eval(0.5);
			let reversed_negative = negative_path.reverse_subpaths();
			for (face_path, face_segments) in kept_faces.iter_mut() {
				if face_segments.is_disjoint(negative_segments) && kurbo::Shape::winding(face_path, sample) != 0 {
					face_path.extend(reversed_negative.clone());
				}
			}
		}

		kept_faces.into_iter().map(|(path, _)| path).collect()
	}

	/// Walks every face orbit of the segment graph and traces each one's path and signed area.
	fn face_orbits(&self) -> FaceOrbits {
		let mut adjacency: Vec<Vec<FaceSide>> = vec![Vec::new(); self.point_domain.len()];
		for (segment_index, (&start, &end)) in self.segment_domain.start_point.iter().zip(&self.segment_domain.end_point).enumerate() {
			adjacency[start].push(FaceSide { segment_index, reversed: false });
			adjacency[end].push(FaceSide { segment_index, reversed: true });
		}

		for neighbors in &mut adjacency {
			neighbors.sort_by(|a, b| {
				let angle = [a, b].map(|side| {
					let curve = self.path_segment_from_index(
						self.segment_domain.start_point[side.segment_index],
						self.segment_domain.end_point[side.segment_index],
						self.segment_domain.handles[side.segment_index],
					);
					let curve = if side.reversed { curve.reverse() } else { curve };
					let tangent = curve.tangent_at_start();
					tangent.y.atan2(tangent.x)
				});
				angle[0].partial_cmp(&angle[1]).unwrap_or(std::cmp::Ordering::Equal)
			})
		}

		let mut faces: Faces = Faces::new();
		let mut seen = FaceSideSet::new(self.segment_domain.id.len());

		for segment_index in 0..self.segment_domain.id.len() {
			for &reversed in &[false, true] {
				let side = FaceSide { segment_index, reversed };
				if seen.contains(side) {
					continue;
				}
				if self.construct_face(&adjacency, side, &mut faces, &mut seen).is_none() {
					// Undo `seen` markings for sides added during this failed face construction,
					// so they remain available for future face constructions starting from different sides.
					if let Some(&last_start) = faces.face_start.last() {
						for &failed_side in &faces.sides[last_start..] {
							seen.remove(failed_side);
						}
					}
					faces.backtrack();
				}
			}
		}

		let mut paths = Vec::with_capacity(faces.face_start.len());
		let mut areas = Vec::with_capacity(faces.face_start.len());
		let mut side_to_orbit = vec![usize::MAX; self.segment_domain.id.len() * 2];
		for orbit_index in 0..faces.face_start.len() {
			let start = faces.face_start[orbit_index];
			let end = faces.face_start.get(orbit_index + 1).copied().unwrap_or(faces.sides.len());
			let sides = &faces.sides[start..end];

			for side in sides {
				side_to_orbit[FaceSideSet::index_of(side)] = orbit_index;
			}
			let path = self.face_path(sides);
			areas.push(kurbo::Shape::area(&path));
			paths.push(path);
		}

		FaceOrbits { faces, paths, areas, side_to_orbit }
	}

	/// Computes each point's degree in the two-core: the graph left after iteratively stripping dead-end segments.
	/// Returns the degrees and which segments were pruned as spurs.
	fn two_core(&self) -> (Vec<usize>, Vec<bool>, Vec<Vec<usize>>) {
		let segment_count = self.segment_domain.id.len();
		let point_count = self.point_domain.len();

		let mut degree = vec![0_usize; point_count];
		let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); point_count];
		for (segment_index, (&start, &end)) in self.segment_domain.start_point.iter().zip(&self.segment_domain.end_point).enumerate() {
			degree[start] += 1;
			degree[end] += 1;
			adjacency[start].push(segment_index);
			adjacency[end].push(segment_index);
		}

		let mut pruned = vec![false; segment_count];
		let mut leaf_points: Vec<usize> = (0..point_count).filter(|&point| degree[point] == 1).collect();
		while let Some(point) = leaf_points.pop() {
			if degree[point] != 1 {
				continue;
			}
			let Some(&segment_index) = adjacency[point].iter().find(|&&segment_index| !pruned[segment_index]) else {
				continue;
			};

			pruned[segment_index] = true;
			for endpoint in [self.segment_domain.start_point[segment_index], self.segment_domain.end_point[segment_index]] {
				degree[endpoint] -= 1;
				if degree[endpoint] == 1 {
					leaf_points.push(endpoint);
				}
			}
		}

		(degree, pruned, adjacency)
	}

	/// Traces one face orbit's sides into a path, reversing each segment walked against its drawn direction.
	fn face_path(&self, sides: &[FaceSide]) -> kurbo::BezPath {
		let mut path = kurbo::BezPath::new();
		let Some(first_side) = sides.first() else { return path };

		let get_point = |point: usize| dvec2_to_point(self.point_domain.positions()[point]);
		let start_point_index = if first_side.reversed {
			self.segment_domain.end_point[first_side.segment_index]
		} else {
			self.segment_domain.start_point[first_side.segment_index]
		};
		path.move_to(get_point(start_point_index));

		for side in sides {
			let (handle, end_index) = match side.reversed {
				false => (self.segment_domain.handles[side.segment_index], self.segment_domain.end_point[side.segment_index]),
				true => (self.segment_domain.handles[side.segment_index].reversed(), self.segment_domain.start_point[side.segment_index]),
			};
			let path_element = match handle {
				BezierHandles::Linear => kurbo::PathEl::LineTo(get_point(end_index)),
				BezierHandles::Quadratic { handle } => kurbo::PathEl::QuadTo(dvec2_to_point(handle), get_point(end_index)),
				BezierHandles::Cubic { handle_start, handle_end } => kurbo::PathEl::CurveTo(dvec2_to_point(handle_start), dvec2_to_point(handle_end), get_point(end_index)),
			};
			path.push(path_element);
		}

		path
	}

	fn construct_face(&self, adjacency: &[Vec<FaceSide>], first: FaceSide, faces: &mut Faces, seen: &mut FaceSideSet) -> Option<()> {
		faces.start_new_face();
		let max_iterations = self.segment_domain.id.len() * 2;
		let mut side = first;
		for _iteration in 1..max_iterations {
			if seen.contains(side) {
				return None;
			}
			seen.insert(side);
			faces.add_side(side);
			let next_vertex = if side.reversed {
				self.segment_domain.start_point[side.segment_index]
			} else {
				self.segment_domain.end_point[side.segment_index]
			};
			let neighbors = &adjacency[next_vertex];
			let side_index = neighbors.iter().position(|s| {
				FaceSide {
					segment_index: s.segment_index,
					reversed: !s.reversed,
				} == side
			})?;
			side = neighbors[(side_index + 1) % neighbors.len()];
			if side == first {
				return Some(());
			}
		}
		None
	}

	/// Normalizes the winding direction of every simple closed loop so nesting depth alone decides fill:
	/// loops at even depth wind positive and loops nested inside them wind negative, regardless of the direction they happened
	/// to be drawn in. Loops passing through branch points, and open runs including dead-end spurs, are left untouched.
	pub fn normalize_winding_directions(&mut self) {
		let segment_count = self.segment_domain.id.len();

		// Strip dead-end spurs first so a spur hanging off a loop doesn't disguise it
		let (degree, pruned, adjacency) = self.two_core();

		// Walk out each simple loop: a chain of surviving segments passing only through degree-2 points.
		// Chains that reach a branch point are mesh or welded structure and keep their drawn directions.
		let mut visited = pruned.clone();
		let mut loops: Vec<Vec<(usize, bool)>> = Vec::new();
		for first_segment in 0..segment_count {
			if visited[first_segment] {
				continue;
			}
			let start = self.segment_domain.start_point[first_segment];
			let end = self.segment_domain.end_point[first_segment];
			if degree[start] != 2 || degree[end] != 2 {
				visited[first_segment] = true;
				continue;
			}

			let mut loop_sides = vec![(first_segment, false)];
			let mut current_point = end;
			let mut is_simple_loop = false;
			for _ in 0..segment_count {
				if current_point == start {
					is_simple_loop = true;
					break;
				}
				if degree[current_point] != 2 {
					break;
				}
				let previous_segment = loop_sides.last().unwrap().0;
				let Some(&next_segment) = adjacency[current_point].iter().find(|&&candidate| !pruned[candidate] && candidate != previous_segment) else {
					break;
				};

				let walked_reversed = self.segment_domain.end_point[next_segment] == current_point;
				current_point = if walked_reversed {
					self.segment_domain.start_point[next_segment]
				} else {
					self.segment_domain.end_point[next_segment]
				};
				loop_sides.push((next_segment, walked_reversed));
			}

			for &(segment_index, _) in &loop_sides {
				visited[segment_index] = true;
			}
			if is_simple_loop {
				loops.push(loop_sides);
			}
		}

		// Trace each loop as a path so area gives its winding and other loops' winding gives containment
		let loop_paths: Vec<kurbo::BezPath> = loops
			.iter()
			.map(|loop_sides| {
				let mut path = kurbo::BezPath::new();
				let (first_segment, first_reversed) = loop_sides[0];
				let start_point = if first_reversed {
					self.segment_domain.end_point[first_segment]
				} else {
					self.segment_domain.start_point[first_segment]
				};
				path.move_to(dvec2_to_point(self.point_domain.positions()[start_point]));
				for &(segment_index, walked_reversed) in loop_sides {
					let (handle, end_index) = match walked_reversed {
						false => (self.segment_domain.handles[segment_index], self.segment_domain.end_point[segment_index]),
						true => (self.segment_domain.handles[segment_index].reversed(), self.segment_domain.start_point[segment_index]),
					};
					let end = dvec2_to_point(self.point_domain.positions()[end_index]);
					let path_element = match handle {
						BezierHandles::Linear => kurbo::PathEl::LineTo(end),
						BezierHandles::Quadratic { handle } => kurbo::PathEl::QuadTo(dvec2_to_point(handle), end),
						BezierHandles::Cubic { handle_start, handle_end } => kurbo::PathEl::CurveTo(dvec2_to_point(handle_start), dvec2_to_point(handle_end), end),
					};
					path.push(path_element);
				}
				path.close_path();
				path
			})
			.collect();

		// Nesting depth counts the bounded face regions covering each loop, so containment still registers when the
		// surrounding contour is branching mesh structure rather than a simple loop. The bounding box check keeps
		// the containment tests from being `O(loops × faces × segments)` on many-loop paths like converted text.
		let orbits = self.face_orbits();
		let bounded_orbits: Vec<(&kurbo::BezPath, kurbo::Rect, HashSet<usize>)> = (0..orbits.face_count())
			.filter(|&orbit_index| orbits.areas[orbit_index] <= 0.)
			.map(|orbit_index| {
				let path = &orbits.paths[orbit_index];
				let segment_set = orbits.face_sides(orbit_index).iter().map(|side| side.segment_index).collect();
				(path, kurbo::Shape::bounding_box(path), segment_set)
			})
			.collect();

		// Reverse the segments of each loop whose drawn winding disagrees with its nesting parity
		for (loop_index, loop_sides) in loops.iter().enumerate() {
			// A segment midpoint is a safe winding sample, as in `construct_faces`
			let Some(first_segment) = loop_paths[loop_index].segments().next() else { continue };
			let sample = first_segment.eval(0.5);
			let loop_segments: HashSet<usize> = loop_sides.iter().map(|&(segment_index, _)| segment_index).collect();
			let depth = bounded_orbits
				.iter()
				.filter(|(path, bounding_box, segment_set)| segment_set.is_disjoint(&loop_segments) && bounding_box.contains(sample) && kurbo::Shape::winding(*path, sample) != 0)
				.count();

			let area = kurbo::Shape::area(&loop_paths[loop_index]);
			let wants_positive = depth % 2 == 0;
			if area != 0. && (area > 0.) != wants_positive {
				for &(segment_index, _) in loop_sides {
					let segment_domain = &mut self.segment_domain;
					std::mem::swap(&mut segment_domain.start_point[segment_index], &mut segment_domain.end_point[segment_index]);
					segment_domain.handles[segment_index] = segment_domain.handles[segment_index].reversed();
				}
			}
		}
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
	type Item = (Vec<ManipulatorGroup>, bool);

	fn next(&mut self) -> Option<Self::Item> {
		let mut current_start = None;
		// First iterate over the single connected points
		if !self.done_one {
			current_start = self.points.iter().enumerate().skip(self.skip).find(|(_, val)| val.connected() == 1);
			self.done_one = current_start.is_none();
			self.skip = current_start.map_or(0, |(index, _)| index + 1);
		}

		// If we've already done the single connected, then go through looking at multi connected
		if current_start.is_none() {
			current_start = self.points.iter().enumerate().skip(self.skip).find(|(_, val)| val.connected() > 0);
			self.skip = current_start.map_or(self.points.len(), |(index, _)| index);
		}

		// If there is no starting point, exit
		let current_start = current_start?.0;

		// There will always be at least one segment connected to this one
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

/// Represents the conversion of IDs used when concatenating vector paths with conflicting IDs.
pub(crate) struct IdMap {
	pub point_offset: usize,
	pub point_map: HashMap<PointId, PointId>,
	pub segment_map: HashMap<SegmentId, SegmentId>,
}

#[cfg(test)]
mod tests {
	use super::*;
	use kurbo::Shape;

	fn build_vector(points: &[DVec2], segments: &[(usize, usize)]) -> Vector {
		let mut vector = Vector::default();
		let mut point_id = PointId::ZERO;
		for &position in points {
			vector.point_domain.push(point_id.next_id(), position);
		}
		let mut segment_id = SegmentId::ZERO;
		for &(start, end) in segments {
			vector.segment_domain.push(segment_id.next_id(), start, end, BezierHandles::Linear);
		}
		vector
	}

	/// Square wound positively with a reverse-wound triangle welded to its corner:
	/// the triangle's interior face is deliberate negative space, so only the surrounding face is filled.
	#[test]
	fn reverse_wound_loop_is_negative_space() {
		let points = [
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(2., 4.),
			DVec2::new(4., 2.),
		];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 4), (4, 5), (5, 0)];
		let vector = build_vector(&points, &segments);
		assert!(vector.is_branching());

		let faces = vector.construct_faces();
		assert_eq!(faces.len(), 1);
		assert_ne!(faces[0].winding(kurbo::Point::new(8., 8.)), 0, "the region around the triangle should be filled");
		assert_eq!(faces[0].winding(kurbo::Point::new(1.5, 1.5)), 0, "the reverse-wound triangle interior should stay empty");
	}

	/// The same welded triangle wound the same way as the square reads as positive space, so both faces fill.
	#[test]
	fn same_wound_loop_is_positive_space() {
		let points = [
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(2., 4.),
			DVec2::new(4., 2.),
		];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 5), (5, 4), (4, 0)];
		let vector = build_vector(&points, &segments);

		let faces = vector.construct_faces();
		assert_eq!(faces.len(), 2);
		assert!(
			faces.iter().any(|face| face.winding(kurbo::Point::new(1.5, 1.5)) != 0),
			"the same-wound triangle interior should be filled"
		);
		assert!(faces.iter().any(|face| face.winding(kurbo::Point::new(8., 8.)) != 0), "the region around the triangle should be filled");
	}

	/// A diamond-in-square mesh with every wall emitted once in index-canonical direction:
	/// every cell is mixed-direction, so all five cells fill even though every point has even degree.
	#[test]
	fn mesh_cells_fill_regardless_of_wall_direction() {
		let points = [
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(5., 0.),
			DVec2::new(10., 5.),
			DVec2::new(5., 10.),
			DVec2::new(0., 5.),
		];
		let segments = [(0, 4), (1, 4), (1, 5), (2, 5), (2, 6), (3, 6), (3, 7), (0, 7), (4, 5), (5, 6), (6, 7), (4, 7)];
		let vector = build_vector(&points, &segments);
		assert!(vector.use_face_fill());

		let faces = vector.construct_faces();
		assert_eq!(faces.len(), 5);
		assert!(faces.iter().any(|face| face.winding(kurbo::Point::new(5., 5.)) != 0), "the center diamond cell should be filled");
		assert!(faces.iter().any(|face| face.winding(kurbo::Point::new(1., 1.)) != 0), "the corner cells should be filled");
	}

	/// A boolean-style donut whose hole shares no points with the outer contour,
	/// made branching by a pen-drawn spur: the hole must be subtracted from the disc face that covers it.
	#[test]
	fn disjoint_negative_loop_is_subtracted_from_covering_face() {
		let points = [
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(3., 3.),
			DVec2::new(3., 7.),
			DVec2::new(7., 7.),
			DVec2::new(7., 3.),
			DVec2::new(1., 5.),
		];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 8)];
		let vector = build_vector(&points, &segments);
		assert!(vector.is_branching());

		let faces = vector.construct_faces();
		assert_eq!(faces.len(), 1);
		assert_ne!(faces[0].winding(kurbo::Point::new(1., 1.)), 0, "the ring around the hole should be filled");
		assert_eq!(faces[0].winding(kurbo::Point::new(5., 5.)), 0, "the reverse-wound hole should stay empty");
	}

	/// The disjoint hole again, but with a vertex snapped exactly onto the covering contour,
	/// where a winding test sampled at that vertex would be unreliable.
	#[test]
	fn hole_with_a_vertex_snapped_onto_the_covering_contour_still_subtracts() {
		let points = [
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(5., 10.),
			DVec2::new(7., 6.),
			DVec2::new(3., 6.),
			DVec2::new(1., 5.),
		];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 4), (0, 7)];
		let vector = build_vector(&points, &segments);
		assert!(vector.is_branching());

		let faces = vector.construct_faces();
		assert_eq!(faces.len(), 1);
		assert_ne!(faces[0].winding(kurbo::Point::new(1., 1.)), 0, "the region around the hole should be filled");
		assert_eq!(faces[0].winding(kurbo::Point::new(5., 7.5)), 0, "the snapped hole should stay empty");
	}

	/// A pen mesh of two cells sharing a wall, with the first cell's loop happening to be drawn in the
	/// reverse direction: both cells must fill, since mesh users don't control shared wall winding.
	#[test]
	fn reverse_drawn_mesh_cell_still_fills() {
		let points = [
			DVec2::new(0., 0.),
			DVec2::new(0., 10.),
			DVec2::new(10., 10.),
			DVec2::new(10., 0.),
			DVec2::new(20., 0.),
			DVec2::new(20., 10.),
		];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (3, 4), (4, 5), (5, 2)];
		let vector = build_vector(&points, &segments);
		assert!(vector.is_branching());

		let faces = vector.construct_faces();
		assert_eq!(faces.len(), 2);
		assert!(faces.iter().any(|face| face.winding(kurbo::Point::new(5., 5.)) != 0), "the reverse-drawn left cell should be filled");
		assert!(faces.iter().any(|face| face.winding(kurbo::Point::new(15., 5.)) != 0), "the right cell should be filled");
	}

	/// A hub-and-spokes mesh whose hub cell happens to be drawn in the reverse direction: its walls
	/// are shared with several sector cells, so it is mesh structure and must still fill.
	#[test]
	fn fully_enclosed_reverse_drawn_mesh_cell_still_fills() {
		let points = [DVec2::new(4., 3.), DVec2::new(6., 3.), DVec2::new(5., 5.), DVec2::new(0., 0.), DVec2::new(10., 0.), DVec2::new(5., 10.)];
		let segments = [(0, 2), (2, 1), (1, 0), (3, 4), (4, 5), (5, 3), (0, 3), (1, 4), (2, 5)];
		let vector = build_vector(&points, &segments);
		assert!(vector.is_branching());
		assert!(drawn_signed_area(&vector, 0..3) < 0.);

		let faces = vector.construct_faces();
		assert_eq!(faces.len(), 4);
		assert!(faces.iter().any(|face| face.winding(kurbo::Point::new(5., 4.)) != 0), "the enclosed hub cell should be filled");
	}

	/// A donut whose hole is bridged to the outer contour: every hole wall faces the single
	/// surrounding ring, so its reverse winding still reads as a deliberate hole.
	#[test]
	fn bridged_donut_hole_stays_empty() {
		let points = [
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(3., 3.),
			DVec2::new(3., 7.),
			DVec2::new(7., 7.),
			DVec2::new(7., 3.),
		];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4)];
		let vector = build_vector(&points, &segments);
		assert!(vector.is_branching());
		assert!(drawn_signed_area(&vector, 4..8) < 0.);

		let faces = vector.construct_faces();
		assert_eq!(faces.len(), 1);
		assert_ne!(faces[0].winding(kurbo::Point::new(8., 5.)), 0, "the ring should be filled");
		assert_eq!(faces[0].winding(kurbo::Point::new(5., 5.)), 0, "the bridged hole should stay empty");
	}

	/// A boolean-style hole nested under a contour that a pen-drawn chord has turned into a mesh:
	/// the hole must survive normalization (its container is no longer a simple loop) and still subtract.
	#[test]
	fn hole_survives_when_its_container_becomes_a_mesh() {
		let points = [
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(1., 1.),
			DVec2::new(1., 3.),
			DVec2::new(3., 3.),
			DVec2::new(3., 1.),
		];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (1, 3), (4, 5), (5, 6), (6, 7), (7, 4)];
		let mut vector = build_vector(&points, &segments);
		assert!(drawn_signed_area(&vector, 5..9) < 0.);

		vector.normalize_winding_directions();
		assert!(drawn_signed_area(&vector, 5..9) < 0., "normalization must not flip a hole nested under mesh structure");

		let faces = vector.construct_faces();
		assert_eq!(faces.len(), 2);
		assert!(faces.iter().all(|face| face.winding(kurbo::Point::new(2., 2.)) == 0), "the hole should stay empty");
		assert!(faces.iter().any(|face| face.winding(kurbo::Point::new(0.5, 0.5)) != 0), "the triangle around the hole should be filled");
		assert!(faces.iter().any(|face| face.winding(kurbo::Point::new(8., 8.)) != 0), "the other triangle should be filled");
	}

	/// A spur walked out and back within a face bounds no area, so it can neither fill a reverse-wound
	/// loop's interior nor punch anything out of it.
	#[test]
	fn spur_does_not_rescue_a_negative_space_loop() {
		let points = [DVec2::new(0., 0.), DVec2::new(0., 10.), DVec2::new(10., 10.), DVec2::new(10., 0.), DVec2::new(5., 5.)];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 4)];
		let vector = build_vector(&points, &segments);

		let faces = vector.construct_faces();
		assert!(faces.is_empty(), "a reverse-wound loop with an interior spur should produce no filled faces");
	}

	/// Shoelace sum over the drawn direction of a range of line segments, so tests can read the
	/// stored winding directly rather than through a path walker that may pick its own direction.
	fn drawn_signed_area(vector: &Vector, segment_range: std::ops::Range<usize>) -> f64 {
		segment_range
			.map(|segment_index| {
				let start = vector.point_domain.positions()[vector.segment_domain.start_point()[segment_index]];
				let end = vector.point_domain.positions()[vector.segment_domain.end_point()[segment_index]];
				start.perp_dot(end)
			})
			.sum::<f64>()
			/ 2.
	}

	#[test]
	fn normalization_flips_a_reverse_wound_loop() {
		let points = [DVec2::new(0., 0.), DVec2::new(0., 10.), DVec2::new(10., 10.), DVec2::new(10., 0.)];
		let mut vector = build_vector(&points, &[(0, 1), (1, 2), (2, 3), (3, 0)]);
		assert!(drawn_signed_area(&vector, 0..4) < 0.);
		vector.normalize_winding_directions();

		assert!(drawn_signed_area(&vector, 0..4) > 0., "a lone loop should wind positively after normalization");
	}

	#[test]
	fn normalization_gives_nested_loops_alternating_winding() {
		let points = [
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(3., 3.),
			DVec2::new(7., 3.),
			DVec2::new(7., 7.),
			DVec2::new(3., 7.),
		];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4)];
		let mut vector = build_vector(&points, &segments);
		vector.normalize_winding_directions();

		assert!(drawn_signed_area(&vector, 0..4) > 0., "the outer loop should wind positively");
		assert!(drawn_signed_area(&vector, 4..8) < 0., "the nested loop should wind negatively");
	}

	/// Loops passing through a branch point are welded or mesh structure whose drawn winding is meaningful,
	/// so normalization must not touch them.
	#[test]
	fn normalization_leaves_branching_structure_untouched() {
		let points = [DVec2::new(0., 0.), DVec2::new(4., 0.), DVec2::new(2., 3.), DVec2::new(-4., 0.), DVec2::new(-2., -3.)];
		let segments = [(0, 1), (1, 2), (2, 0), (0, 3), (3, 4), (4, 0)];
		let mut vector = build_vector(&points, &segments);

		let starts_before = vector.segment_domain.start_point().to_vec();
		let ends_before = vector.segment_domain.end_point().to_vec();
		vector.normalize_winding_directions();

		assert_eq!(vector.segment_domain.start_point(), starts_before.as_slice());
		assert_eq!(vector.segment_domain.end_point(), ends_before.as_slice());
	}

	/// Pruning dead-end spurs first lets the loop they hang off still be found and flipped,
	/// while the spur segment itself keeps its direction.
	#[test]
	fn normalization_flips_a_loop_with_a_spur_attached() {
		let points = [DVec2::new(0., 0.), DVec2::new(0., 10.), DVec2::new(10., 10.), DVec2::new(10., 0.), DVec2::new(5., 5.)];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 4)];
		let mut vector = build_vector(&points, &segments);
		vector.normalize_winding_directions();

		assert_eq!(vector.segment_domain.start_point()[0], 1, "the reverse-wound loop should be flipped");
		assert_eq!(vector.segment_domain.start_point()[4], 0, "the spur should keep its drawn direction");
		assert_eq!(vector.segment_domain.end_point()[4], 4);
	}
}
