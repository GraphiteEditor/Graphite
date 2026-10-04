use crate::vector::addressing::{self, Span};
use crate::vector::contour::contours;
use crate::vector::misc::{ManipulatorGroup, Tangent, point_to_dvec2, segment_to_handles};
use crate::vector::path::{CurveType, Path, append_segment, curve_type_at};
use crate::vector::vector_types::Vector;
use core_types::list::{ATTR_ID, Item, List};
use dyn_any::DynAny;
use fixedbitset::FixedBitSet;
use glam::DVec2;
use kurbo::{ParamCurve, PathSeg};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

/// A simple macro for creating strongly typed ids (to avoid confusion when passing around ids).
macro_rules! create_ids {
	($($id:ident),*) => {
		$(
			#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Ord, Eq, Hash, graphene_hash::CacheHash, DynAny)]
			#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
			/// A strongly typed ID, defaulting to [`Self::ZERO`].
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

				/// Adds one to the current value and returns it.
				pub fn next_id(&mut self) -> Self {
					self.0 += 1;
					*self
				}
			}
		)*
	};
}

create_ids! { PointId, SegmentId }

impl PointId {
	/// The filler naming nothing, carried by pool points that are handles rather than anchors.
	pub const UNNAMED: PointId = PointId(u64::MAX);
}

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
		self.0 = Some(i);
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

/// A flat view of every resolvable span as an edge of the segment graph, so graph algorithms can
/// address segments by one index. Rebuilt on demand rather than stored.
pub(crate) struct SegmentGraph {
	pub spans: Vec<Span>,
	pub ids: Vec<SegmentId>,
	pub start_point: Vec<usize>,
	pub end_point: Vec<usize>,
	pub segments: Vec<PathSeg>,
	pub point_count: usize,
}

impl SegmentGraph {
	pub fn new(vector: &Vector, paths: &List<Path>) -> Self {
		let mut graph = Self {
			spans: Vec::new(),
			ids: Vec::new(),
			start_point: Vec::new(),
			end_point: Vec::new(),
			segments: Vec::new(),
			point_count: vector.point_count(),
		};

		for span in addressing::spans(paths) {
			let Some(path) = paths.element(span.path as usize) else { continue };
			let Some((start, end)) = addressing::span_endpoints(paths, span) else { continue };
			let Some(segment) = path.segment(&vector.0, curve_type_at(paths, span.path as usize), span.index as usize) else {
				continue;
			};

			graph.spans.push(span);
			graph.ids.push(addressing::segment_id_of(paths, span).unwrap_or_default());
			graph.start_point.push(start as usize);
			graph.end_point.push(end as usize);
			graph.segments.push(segment);
		}

		graph
	}

	pub fn len(&self) -> usize {
		self.spans.len()
	}

	fn segment(&self, index: usize, reversed: bool) -> PathSeg {
		if reversed { self.segments[index].reverse() } else { self.segments[index] }
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

impl SegmentGraph {
	/// Computes the direction-of-travel tangent at one endpoint of a segment.
	/// Uses the "first distinct control point" pattern: iterates through the Bezier control points
	/// from the anchor outward, returning the direction to the first one that differs in position.
	/// This handles zero-length handles by finding the tangent direction in the limit.
	/// Returns `DVec2::ZERO` if all control points coincide (fully degenerate segment).
	fn segment_tangent_at_endpoint(&self, segment_index: usize, at_start: bool) -> DVec2 {
		let (points, count) = match self.segments[segment_index] {
			PathSeg::Line(line) => ([point_to_dvec2(line.p0), point_to_dvec2(line.p1), DVec2::ZERO, DVec2::ZERO], 2),
			PathSeg::Quad(quad) => ([point_to_dvec2(quad.p0), point_to_dvec2(quad.p1), point_to_dvec2(quad.p2), DVec2::ZERO], 3),
			PathSeg::Cubic(cubic) => ([point_to_dvec2(cubic.p0), point_to_dvec2(cubic.p1), point_to_dvec2(cubic.p2), point_to_dvec2(cubic.p3)], 4),
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
	pub fn point_tangent(&self, point_index: usize) -> Option<DVec2> {
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
		let tangent1 = self.segment_tangent_at_endpoint(segment_index, at_start).try_normalize();

		if connection_count == 1 {
			return tangent1;
		}

		// Compute the direction-of-travel tangent for the second connection
		let (segment_index, at_start) = connections[1];
		let tangent2 = self.segment_tangent_at_endpoint(segment_index, at_start).try_normalize();

		// Average the two normalized tangents
		let average = tangent1? + tangent2?;

		// If the tangents are nearly opposite (straight-through), use t1 directly
		if average.length_squared() < (f64::EPSILON * 1e3).powi(2) {
			return tangent1;
		}

		average.try_normalize()
	}

	/// Walks every face orbit of the segment graph and traces each one's path and signed area.
	fn face_orbits(&self) -> FaceOrbits {
		let mut adjacency: Vec<Vec<FaceSide>> = vec![Vec::new(); self.point_count];
		for (segment_index, (&start, &end)) in self.start_point.iter().zip(&self.end_point).enumerate() {
			adjacency[start].push(FaceSide { segment_index, reversed: false });
			adjacency[end].push(FaceSide { segment_index, reversed: true });
		}

		for neighbors in &mut adjacency {
			neighbors.sort_by(|a, b| {
				let angle = [a, b].map(|side| {
					let tangent = self.segment(side.segment_index, side.reversed).tangent_at_start();
					tangent.y.atan2(tangent.x)
				});
				angle[0].partial_cmp(&angle[1]).unwrap_or(std::cmp::Ordering::Equal)
			})
		}

		let mut faces: Faces = Faces::new();
		let mut seen = FaceSideSet::new(self.len());

		for segment_index in 0..self.len() {
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
		let mut side_to_orbit = vec![usize::MAX; self.len() * 2];
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
		let segment_count = self.len();
		let point_count = self.point_count;

		let mut degree = vec![0_usize; point_count];
		let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); point_count];
		for (segment_index, (&start, &end)) in self.start_point.iter().zip(&self.end_point).enumerate() {
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
			for endpoint in [self.start_point[segment_index], self.end_point[segment_index]] {
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

		path.move_to(self.segment(first_side.segment_index, first_side.reversed).start());
		for side in sides {
			append_segment(&mut path, self.segment(side.segment_index, side.reversed));
		}

		path
	}

	fn construct_face(&self, adjacency: &[Vec<FaceSide>], first: FaceSide, faces: &mut Faces, seen: &mut FaceSideSet) -> Option<()> {
		faces.start_new_face();
		let max_iterations = self.len() * 2;
		let mut side = first;
		for _iteration in 1..max_iterations {
			if seen.contains(side) {
				return None;
			}
			seen.insert(side);
			faces.add_side(side);
			let next_vertex = if side.reversed {
				self.start_point[side.segment_index]
			} else {
				self.end_point[side.segment_index]
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
				.all(|side| core_degree[self.start_point[side.segment_index]] == 2 && core_degree[self.end_point[side.segment_index]] == 2);
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
}

impl Vector {
	/// Whether any point has more than two segment ends meeting at it, making the vector a mesh.
	pub fn is_branching(&self, paths: &List<Path>) -> bool {
		// Tally the segment ends at each anchor in one pass over the references, stopping at the first junction
		let mut spokes = vec![0_u8; self.point_count()];
		for (path_index, path) in paths.iter_element_values().enumerate() {
			let stride = curve_type_at(paths, path_index).stride();
			let references = path.0.iter_element_values().as_slice();
			let last = references.len().saturating_sub(1);

			for (index, &point) in references.iter().enumerate().step_by(stride) {
				let Some(count) = spokes.get_mut(point as usize) else { continue };
				*count = count.saturating_add(u8::from(index > 0) + u8::from(index < last));
				if *count > 2 {
					return true;
				}
			}
		}

		false
	}

	/// Determines if face-by-face fill rendering should be used. Branching vectors are meshes, whose
	/// bounded faces are found and filled individually rather than filling the stroke path directly.
	pub fn use_face_fill(&self, paths: &List<Path>) -> bool {
		self.is_branching(paths)
	}

	/// Returns the fillable faces of the segment graph, one closed path per bounded face.
	pub fn construct_faces(&self, paths: &List<Path>) -> Vec<kurbo::BezPath> {
		SegmentGraph::new(self, paths).construct_faces()
	}

	/// Computes the average tangent direction at a point based on its 1 or 2 connected segments.
	pub fn point_tangent(&self, paths: &List<Path>, point_index: usize) -> Option<DVec2> {
		SegmentGraph::new(self, paths).point_tangent(point_index)
	}

	/// Every segment's stable ID, in span order.
	pub fn segment_ids(&self, paths: &List<Path>) -> Vec<SegmentId> {
		addressing::spans(paths).map(|span| addressing::segment_id_of(paths, span).unwrap_or_default()).collect()
	}

	pub fn auto_join_paths(&self, paths: &List<Path>) -> Vec<FoundSubpath> {
		let graph = SegmentGraph::new(self, paths);
		let segments = (0..graph.len()).map(|index| HalfEdge::new(graph.ids[index], graph.start_point[index], graph.end_point[index], false));

		let mut found: Vec<FoundSubpath> = Vec::new();
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
				found.push(FoundSubpath::from_segment(seg_ref));
				current_path = found.last_mut();
			}

			previous = Some((start, end));
		}

		// Second pass. Try to join paths together.
		let mut paths_to_join = found;
		let mut joined_paths = Vec::new();

		loop {
			let mut prev_index: Option<usize> = None;
			let original_len = paths_to_join.len();

			for current in paths_to_join.into_iter() {
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
			paths_to_join = joined_paths;
			joined_paths = Vec::new();
		}
	}

	/// Construct a [`kurbo::BezPath`] from an iterator of segments with (segment, start point, end point) independently of discontinuities.
	pub fn bezpath_from_segments_ignore_discontinuities(&self, segments: impl Iterator<Item = (PathSeg, usize, usize)>) -> Option<kurbo::BezPath> {
		let handle = |segment: PathSeg, from_start: bool| {
			let handles = segment_to_handles(&segment);
			if from_start { handles.start() } else { handles.end() }
		};

		let mut first_point = None;
		let mut manipulators_list = Vec::new();
		let mut last: Option<(usize, PathSeg)> = None;

		for (segment, start, end) in segments {
			first_point = Some(first_point.unwrap_or(start));

			manipulators_list.push(ManipulatorGroup {
				anchor: self.position(start)?,
				in_handle: last.and_then(|(_, segment)| handle(segment, false)),
				out_handle: handle(segment, true),
				id: self.point_id_at(start as u32).unwrap_or_default(),
			});

			last = Some((end, segment));
		}

		let closed = manipulators_list.len() > 1 && last.map(|(point, _)| point) == first_point;

		if let Some((end, last_segment)) = last {
			if closed {
				manipulators_list[0].in_handle = handle(last_segment, false);
			} else {
				manipulators_list.push(ManipulatorGroup {
					anchor: self.position(end)?,
					in_handle: handle(last_segment, false),
					out_handle: None,
					id: self.point_id_at(end as u32).unwrap_or_default(),
				});
			}
		}

		Some(crate::vector::misc::bezpath_from_manipulator_groups(&manipulators_list, closed))
	}

	/// Construct and return an iterator of `(Vec<ManipulatorGroup>, bool)` for each stroke.
	/// The boolean in the tuple indicates if the path is closed.
	/// The groups pair index-for-index with the contours emission, both stitching the same contours.
	pub fn stroke_manipulator_groups(&self, paths: &List<Path>) -> impl Iterator<Item = (Vec<ManipulatorGroup>, bool)> {
		let mut groups = Vec::new();

		let curve_types: Vec<CurveType> = (0..paths.len()).map(|index| curve_type_at(paths, index)).collect();
		let ids = self.0.iter_attribute_values::<PointId>(ATTR_ID).map(|ids| ids.as_slice()).unwrap_or_default();
		let id_at = |point: u32| ids.get(point as usize).copied().unwrap_or_default();

		for contour in contours(self.point_count(), paths) {
			let mut manipulators_list: Vec<ManipulatorGroup> = Vec::new();
			let mut in_handle: Option<DVec2> = None;

			for part in &contour.parts {
				let Some(path) = paths.element(part.path as usize) else { continue };
				let Some(&curve_type) = curve_types.get(part.path as usize) else { continue };
				let (stride, count) = (curve_type.stride(), path.segment_count(curve_type));

				for step in 0..count {
					let span = if part.reversed { count - 1 - step } else { step };
					let Some(segment) = path.segment(&self.0, curve_type, span) else { continue };
					let (Some(start), Some(end)) = (path.reference(span * stride), path.reference(span * stride + stride)) else {
						continue;
					};

					// A quadratic's one handle leaves its anchor whichever way the span is crossed
					let handles = segment_to_handles(&segment);
					let handles = if part.reversed { handles.reversed() } else { handles };
					let anchor = if part.reversed { end } else { start };

					manipulators_list.push(ManipulatorGroup {
						anchor: point_to_dvec2(if part.reversed { segment.end() } else { segment.start() }),
						in_handle,
						out_handle: handles.start(),
						id: id_at(anchor),
					});

					in_handle = handles.end();
				}
			}

			if contour.closed {
				if let Some(first) = manipulators_list.first_mut() {
					first.in_handle = in_handle;
				}
			} else if let Some(position) = self.position(contour.exit as usize) {
				manipulators_list.push(ManipulatorGroup {
					anchor: position,
					in_handle,
					out_handle: None,
					id: id_at(contour.exit),
				});
			}

			if !manipulators_list.is_empty() {
				groups.push((manipulators_list, contour.closed));
			}
		}

		groups.into_iter()
	}

	/// Normalizes the winding direction of every simple closed loop so nesting depth alone decides fill:
	/// loops at even depth wind positive and loops nested inside them wind negative, regardless of the direction they happened
	/// to be drawn in. Loops passing through branch points, and open runs including dead-end spurs, are left untouched.
	pub fn normalize_winding_directions(&mut self, paths: &mut List<Path>) {
		let graph = SegmentGraph::new(self, paths);
		let segment_count = graph.len();

		// Strip dead-end spurs first so a spur hanging off a loop doesn't disguise it
		let (degree, pruned, adjacency) = graph.two_core();

		// Walk out each simple loop: a chain of surviving segments passing only through degree-2 points.
		// Chains that reach a branch point are mesh or welded structure and keep their drawn directions.
		let mut visited = pruned.clone();
		let mut loops: Vec<Vec<(usize, bool)>> = Vec::new();
		for first_segment in 0..segment_count {
			if visited[first_segment] {
				continue;
			}
			let start = graph.start_point[first_segment];
			let end = graph.end_point[first_segment];
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

				let walked_reversed = graph.end_point[next_segment] == current_point;
				current_point = if walked_reversed { graph.start_point[next_segment] } else { graph.end_point[next_segment] };
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
				path.move_to(graph.segment(first_segment, first_reversed).start());
				for &(segment_index, walked_reversed) in loop_sides {
					append_segment(&mut path, graph.segment(segment_index, walked_reversed));
				}
				path.close_path();
				path
			})
			.collect();

		// Nesting depth counts the bounded face regions covering each loop, so containment still registers when the
		// surrounding contour is branching mesh structure rather than a simple loop. The bounding box check keeps
		// the containment tests from being `O(loops × faces × segments)` on many-loop paths like converted text.
		let orbits = graph.face_orbits();
		let bounded_orbits: Vec<(&kurbo::BezPath, kurbo::Rect, HashSet<usize>)> = (0..orbits.face_count())
			.filter(|&orbit_index| orbits.areas[orbit_index] <= 0.)
			.map(|orbit_index| {
				let path = &orbits.paths[orbit_index];
				let segment_set = orbits.face_sides(orbit_index).iter().map(|side| side.segment_index).collect();
				(path, kurbo::Shape::bounding_box(path), segment_set)
			})
			.collect();

		// Collect the loops drawn with a winding that disagrees with their nesting parity
		let mut reversals: Vec<&Vec<(usize, bool)>> = Vec::new();
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
				reversals.push(loop_sides);
			}
		}

		let reversed: HashSet<(u32, u32)> = reversals
			.into_iter()
			.flatten()
			.map(|&(segment_index, _)| (graph.spans[segment_index].path, graph.spans[segment_index].index))
			.collect();
		if !reversed.is_empty() {
			*paths = reverse_spans_in_place(paths, &reversed);
		}
	}
}

/// Reweaves the paths with the given `(path, span)` pairs reversed. Reversed spans that run on from one into the next, within
/// a path or across consecutive paths, reverse together as one stretch, so a flipped loop with its pieces stored together is
/// still read from the same anchor.
fn reverse_spans_in_place(paths: &List<Path>, reversed: &HashSet<(u32, u32)>) -> List<Path> {
	let mut rewoven = List::new();

	// Reversed pieces of the stretch being gathered, output in the opposite order once it ends
	let mut stretch: Vec<Item<Path>> = Vec::new();
	let mut stretch_end = None;

	for path_index in 0..paths.len() {
		let Some(mut template) = paths.clone_item(path_index) else { continue };
		let curve_type = curve_type_at(paths, path_index);
		let stride = curve_type.stride();
		let span_count = template.element().segment_count(curve_type);
		let mut flips: Vec<bool> = (0..span_count).map(|span| reversed.contains(&(path_index as u32, span as u32))).collect();
		let run_end = |flips: &[bool], start: usize| (start + 1..span_count).find(|&span| flips[span] != flips[start]).unwrap_or(span_count);

		// A reversed run arriving back at the anchor it leaves from turns around where it sits, which keeps the path whole
		let mut start = 0;
		while start < span_count {
			let end = run_end(&flips, start);
			let references = &template.element().0;
			if flips[start] && references.element(start * stride) == references.element(end * stride) {
				template.element_mut().reverse_spans(curve_type, start..end);
				flips[start..end].fill(false);
			}
			start = end;
		}

		if !flips.contains(&true) {
			rewoven.extend(stretch.drain(..).rev().collect());
			rewoven.push(template);
			continue;
		}

		// Cut the path into pieces wherever it changes between kept and reversed spans
		let path = template.element();
		let mut start = 0;
		while start < span_count {
			let end = run_end(&flips, start);

			let mut piece = Item::from_parts(path.clone_span_range(curve_type, start, end), template.attributes().clone());
			let references = &piece.element().0;
			let (first, last) = (references.element(0).copied(), references.element(references.len().saturating_sub(1)).copied());

			// A reversed piece joins the stretch only by setting out from where the stretch has reached
			if !flips[start] || stretch_end != first {
				rewoven.extend(stretch.drain(..).rev().collect());
			}

			if flips[start] {
				piece.element_mut().reverse_spans(curve_type, 0..end - start);
				stretch.push(piece);
				stretch_end = last;
			} else {
				rewoven.push(piece);
			}

			start = end;
		}
	}
	rewoven.extend(stretch.drain(..).rev().collect());

	rewoven
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::vector::path::CurveType;
	use core_types::list::ATTR_CURVE_TYPE;
	use kurbo::Shape;

	fn build_vector(points: &[DVec2], segments: &[(usize, usize)]) -> (Vector, List<Path>) {
		let mut vector = Vector::default();
		let mut point_id = PointId::ZERO;
		for (index, &position) in points.iter().enumerate() {
			vector.0.push(Item::new_from_element(position));
			vector.0.set_attribute(ATTR_ID, index, point_id.next_id());
		}

		let mut paths = List::new();
		let mut segment_id = SegmentId::ZERO;
		for &(start, end) in segments {
			let mut path = Path::from_references([start as u32, end as u32]);
			path.0.set_attribute(ATTR_ID, 0, segment_id.next_id());
			paths.push(Item::new_from_element(path).with_attribute(ATTR_CURVE_TYPE, CurveType::Linear));
		}

		(vector, paths)
	}

	// A morph pairs manipulators by position, so a loop is read from the anchor it is stored from, in its stored direction
	#[test]
	fn a_closed_loop_keeps_its_stored_start_and_direction() {
		let (vector, mut paths) = build_vector(&[DVec2::new(0., 10.), DVec2::new(0., 0.), DVec2::new(10., 0.)], &[]);

		let mut path = Path::from_references([1, 2, 0, 1]);
		let mut segment_id = SegmentId::ZERO;
		for span in 0..3 {
			path.0.set_attribute(ATTR_ID, span, segment_id.next_id());
		}
		paths.push(Item::new_from_element(path).with_attribute(ATTR_CURVE_TYPE, CurveType::Linear));

		let (manipulators, closed) = vector.stroke_manipulator_groups(&paths).next().expect("the loop is one contour");
		assert!(closed);
		let anchors: Vec<DVec2> = manipulators.iter().map(|manipulator| manipulator.anchor).collect();
		assert_eq!(anchors, vec![DVec2::new(0., 0.), DVec2::new(10., 0.), DVec2::new(0., 10.)]);
	}

	/// Manipulator groups and contours are two views of the same stitching, so they must agree on order and
	/// geometry. Click targets, the snapper's point recovery, and outline stitching all rely on it.
	#[test]
	fn manipulator_groups_and_contours_agree() {
		use crate::vector::misc::bezpath_from_manipulator_groups;
		use crate::vector::path::pool_and_paths_from_bezpath;

		// A closed contour woven before an open one, which is the order both views keep
		let mut source = kurbo::BezPath::new();
		source.move_to((0., 0.));
		source.line_to((10., 0.));
		source.line_to((10., 10.));
		source.close_path();
		source.move_to((20., 20.));
		source.line_to((30., 20.));
		source.curve_to((35., 25.), (35., 30.), (30., 35.));

		let (pool, paths) = pool_and_paths_from_bezpath(&source);
		let vector = Vector(pool);

		let groups: Vec<_> = vector.stroke_manipulator_groups(&paths).collect();
		let contours = vector.stroke_bezpath_iter(&paths).collect::<Vec<_>>();
		assert_eq!(groups.len(), 2);
		assert_eq!(contours.len(), 2);

		// Each group re-emits its own contour, in stored order
		assert!(groups[0].1, "the closed loop should be emitted first");
		assert!(!groups[1].1, "the open run should follow");
		for (index, (manipulators, closed)) in groups.iter().enumerate() {
			let rebuilt = bezpath_from_manipulator_groups(manipulators, *closed);
			assert_eq!(rebuilt.elements(), contours[index].elements(), "group {index} should describe contour {index}");
		}
	}

	/// Two paths welded head to head, so the contour must cross the second one backward and still
	/// emit anchors and geometry in travel order.
	#[test]
	fn contour_crosses_a_welded_path_backward() {
		use crate::vector::misc::bezpath_from_manipulator_groups;

		let points = [DVec2::new(0., 0.), DVec2::new(10., 0.), DVec2::new(20., 0.)];
		let (vector, paths) = build_vector(&points, &[(0, 1), (2, 1)]);

		let contours = vector.stroke_bezpath_iter(&paths).collect::<Vec<_>>();
		assert_eq!(contours.len(), 1, "the shared endpoint welds both paths into one contour");

		let groups: Vec<_> = vector.stroke_manipulator_groups(&paths).collect();
		assert_eq!(groups.len(), 1);
		let (manipulators, closed) = &groups[0];
		assert!(!closed);
		assert_eq!(manipulators.iter().map(|group| group.anchor).collect::<Vec<_>>(), points.to_vec());

		assert_eq!(bezpath_from_manipulator_groups(manipulators, *closed).elements(), contours[0].elements());
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
		let (vector, paths) = build_vector(&points, &segments);
		assert!(vector.is_branching(&paths));

		let faces = vector.construct_faces(&paths);
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
		let (vector, paths) = build_vector(&points, &segments);

		let faces = vector.construct_faces(&paths);
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
		let (vector, paths) = build_vector(&points, &segments);
		assert!(vector.use_face_fill(&paths));

		let faces = vector.construct_faces(&paths);
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
		let (vector, paths) = build_vector(&points, &segments);
		assert!(vector.is_branching(&paths));

		let faces = vector.construct_faces(&paths);
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
		let (vector, paths) = build_vector(&points, &segments);
		assert!(vector.is_branching(&paths));

		let faces = vector.construct_faces(&paths);
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
		let (vector, paths) = build_vector(&points, &segments);
		assert!(vector.is_branching(&paths));

		let faces = vector.construct_faces(&paths);
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
		let (vector, paths) = build_vector(&points, &segments);
		assert!(vector.is_branching(&paths));
		assert!(drawn_signed_area(&vector, &paths, 0..3) < 0.);

		let faces = vector.construct_faces(&paths);
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
		let (vector, paths) = build_vector(&points, &segments);
		assert!(vector.is_branching(&paths));
		assert!(drawn_signed_area(&vector, &paths, 4..8) < 0.);

		let faces = vector.construct_faces(&paths);
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
		let (mut vector, mut paths) = build_vector(&points, &segments);
		assert!(drawn_signed_area(&vector, &paths, 5..9) < 0.);

		vector.normalize_winding_directions(&mut paths);
		assert!(drawn_signed_area(&vector, &paths, 5..9) < 0., "normalization must not flip a hole nested under mesh structure");

		let faces = vector.construct_faces(&paths);
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
		let (vector, paths) = build_vector(&points, &segments);

		let faces = vector.construct_faces(&paths);
		assert!(faces.is_empty(), "a reverse-wound loop with an interior spur should produce no filled faces");
	}

	/// Shoelace sum over the stored direction of a range of line segments, so tests read each segment's winding as stored.
	fn drawn_signed_area(vector: &Vector, paths: &List<Path>, segment_range: std::ops::Range<usize>) -> f64 {
		let graph = SegmentGraph::new(vector, paths);
		segment_range
			.map(|segment_index| {
				let start = vector.position(graph.start_point[segment_index]).unwrap();
				let end = vector.position(graph.end_point[segment_index]).unwrap();
				start.perp_dot(end)
			})
			.sum::<f64>()
			/ 2.
	}

	#[test]
	fn normalization_flips_a_reverse_wound_loop() {
		let points = [DVec2::new(0., 0.), DVec2::new(0., 10.), DVec2::new(10., 10.), DVec2::new(10., 0.)];
		let (mut vector, mut paths) = build_vector(&points, &[(0, 1), (1, 2), (2, 3), (3, 0)]);
		assert!(drawn_signed_area(&vector, &paths, 0..4) < 0.);
		vector.normalize_winding_directions(&mut paths);

		assert!(drawn_signed_area(&vector, &paths, 0..4) > 0., "a lone loop should wind positively after normalization");
	}

	fn references(paths: &List<Path>) -> Vec<Vec<u32>> {
		paths.iter_element_values().map(|path| path.references().collect()).collect()
	}

	fn span_ids(paths: &List<Path>, path_index: usize, span_count: usize) -> Vec<SegmentId> {
		let path = paths.element(path_index).unwrap();
		(0..span_count).map(|span| path.0.attribute_cloned_or_default(ATTR_ID, span)).collect()
	}

	// A morph or a dash pattern reads a loop from where it is stored, so flipping it must not move that anchor
	#[test]
	fn normalization_flips_a_closed_path_from_the_same_start() {
		let points = [DVec2::new(0., 0.), DVec2::new(0., 10.), DVec2::new(10., 10.), DVec2::new(10., 0.)];
		let (mut vector, mut paths) = build_vector(&points, &[]);

		let mut path = Path::from_references([0, 1, 2, 3, 0]);
		let mut segment_id = SegmentId::ZERO;
		let ids: Vec<SegmentId> = (0..4).map(|_| segment_id.next_id()).collect();
		for (span, &id) in ids.iter().enumerate() {
			path.0.set_attribute(ATTR_ID, span, id);
		}
		paths.push(Item::new_from_element(path).with_attribute(ATTR_CURVE_TYPE, CurveType::Linear));

		vector.normalize_winding_directions(&mut paths);

		assert_eq!(references(&paths), vec![vec![0, 3, 2, 1, 0]]);
		assert_eq!(span_ids(&paths, 0, 4), ids.iter().rev().copied().collect::<Vec<_>>(), "each span keeps its ID");
	}

	#[test]
	fn normalization_flips_a_loop_of_several_paths_from_the_same_start() {
		let points = [DVec2::new(0., 0.), DVec2::new(0., 10.), DVec2::new(10., 10.), DVec2::new(10., 0.)];
		let (mut vector, mut paths) = build_vector(&points, &[]);

		for references in [[0, 1, 2], [2, 3, 0]] {
			let path = Path::from_references(references);
			paths.push(Item::new_from_element(path).with_attribute(ATTR_CURVE_TYPE, CurveType::Linear));
		}

		vector.normalize_winding_directions(&mut paths);

		assert_eq!(references(&paths), vec![vec![0, 3, 2], vec![2, 1, 0]]);
		let contours = contours(vector.0.len(), &paths);
		assert_eq!(contours.len(), 1);
		assert!(contours[0].closed);
		assert_eq!(contours[0].entry, 0);
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
		let (mut vector, mut paths) = build_vector(&points, &segments);
		vector.normalize_winding_directions(&mut paths);

		assert!(drawn_signed_area(&vector, &paths, 0..4) > 0., "the outer loop should wind positively");
		assert!(drawn_signed_area(&vector, &paths, 4..8) < 0., "the nested loop should wind negatively");
	}

	/// Loops passing through a branch point are welded or mesh structure whose drawn winding is meaningful,
	/// so normalization must not touch them.
	#[test]
	fn normalization_leaves_branching_structure_untouched() {
		let points = [DVec2::new(0., 0.), DVec2::new(4., 0.), DVec2::new(2., 3.), DVec2::new(-4., 0.), DVec2::new(-2., -3.)];
		let segments = [(0, 1), (1, 2), (2, 0), (0, 3), (3, 4), (4, 0)];
		let (mut vector, mut paths) = build_vector(&points, &segments);

		let before = SegmentGraph::new(&vector, &paths);
		let (starts_before, ends_before) = (before.start_point.clone(), before.end_point.clone());
		vector.normalize_winding_directions(&mut paths);

		let after = SegmentGraph::new(&vector, &paths);
		assert_eq!(after.start_point, starts_before);
		assert_eq!(after.end_point, ends_before);
	}

	/// Pruning dead-end spurs first lets the loop they hang off still be found and flipped,
	/// while the spur segment itself keeps its direction.
	#[test]
	fn normalization_flips_a_loop_with_a_spur_attached() {
		let points = [DVec2::new(0., 0.), DVec2::new(0., 10.), DVec2::new(10., 10.), DVec2::new(10., 0.), DVec2::new(5., 5.)];
		let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 4)];
		let (mut vector, mut paths) = build_vector(&points, &segments);
		vector.normalize_winding_directions(&mut paths);

		assert!(drawn_signed_area(&vector, &paths, 0..4) > 0., "the reverse-wound loop should be flipped");
		let graph = SegmentGraph::new(&vector, &paths);
		let spur = graph.spans.iter().position(|span| span.path == 4).expect("the spur keeps its own path");
		assert_eq!((graph.start_point[spur], graph.end_point[spur]), (0, 4), "the spur should keep its drawn direction");
	}

	/// A loop covering only the middle spans of one longer path reverses where it sits, leaving the path whole.
	#[test]
	fn normalization_flips_a_loop_inside_a_longer_path() {
		let points = [
			DVec2::new(-5., 0.),
			DVec2::new(0., 0.),
			DVec2::new(0., 10.),
			DVec2::new(10., 10.),
			DVec2::new(10., 0.),
			DVec2::new(15., 0.),
		];
		let mut path = Path::from_references([0, 1, 2, 3, 4, 1, 5]);
		let mut segment_id = SegmentId::ZERO;
		for span in 0..6 {
			path.0.set_attribute(ATTR_ID, span, segment_id.next_id());
		}

		let mut vector = Vector::default();
		let mut point_id = PointId::ZERO;
		for (index, &position) in points.iter().enumerate() {
			vector.0.push(Item::new_from_element(position));
			vector.0.set_attribute(ATTR_ID, index, point_id.next_id());
		}
		let mut paths = List::new();
		paths.push(Item::new_from_element(path).with_attribute(ATTR_CURVE_TYPE, CurveType::Linear));

		vector.normalize_winding_directions(&mut paths);

		// The reverse-wound inner loop (1 -> 2 -> 3 -> 4 -> 1) flips while both spur spans keep their direction
		let graph = SegmentGraph::new(&vector, &paths);
		let loop_area = (0..graph.len())
			.filter(|&index| graph.start_point[index] != 0 && graph.end_point[index] != 0 && graph.start_point[index] != 5 && graph.end_point[index] != 5)
			.map(|index| {
				let start = vector.position(graph.start_point[index]).unwrap();
				let end = vector.position(graph.end_point[index]).unwrap();
				start.perp_dot(end)
			})
			.sum::<f64>()
			/ 2.;
		assert!(loop_area > 0., "the inner loop should wind positively after normalization");
		assert_eq!(references(&paths), vec![vec![0, 1, 4, 3, 2, 1, 5]], "the path stays whole around its flipped loop");
	}
}
