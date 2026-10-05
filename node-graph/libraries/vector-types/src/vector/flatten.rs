use super::misc::{BezierHandles, HandleId, HandleType, segment_to_handles};
use super::path::{CurveType, Path, continues_previous_path, curve_type_at, push_path};
use super::vector_attributes::{PointId, SegmentId};
use super::vector_types::Vector;
use core_types::list::{ATTR_ID, List};
use glam::DVec2;
use rustc_hash::{FxHashMap, FxHashSet};

/// One path under construction: its curve type, the pool references it weaves, and the IDs of the spans between them.
type Run = (CurveType, Vec<u32>, Vec<SegmentId>);

/// The legacy flat form that modifications and merge passes are expressed against: anchors with IDs,
/// segments with IDs and absolute handles, and G1 pairs. Callers flatten storage into this, edit it,
/// and weave it back.
#[derive(Default)]
pub struct WorkingVector {
	pub point_ids: Vec<PointId>,
	pub point_positions: Vec<DVec2>,
	pub segment_ids: Vec<SegmentId>,
	pub segment_start: Vec<usize>,
	pub segment_end: Vec<usize>,
	pub segment_handles: Vec<BezierHandles>,
	/// Which segments open a path of their own, kept as path boundaries when rewoven. Missing entries count as false.
	pub segment_begins_path: Vec<bool>,
	pub colinear: Vec<[HandleId; 2]>,
}

impl WorkingVector {
	/// Reads the pool and paths into the working form, one working point per anchor or free point.
	pub fn flatten(vector: &Vector, paths: &List<Path>) -> Self {
		let mut working = Self::default();
		let connectivity = vector.connectivity(paths);
		let pool_ids = vector.0.iter_attribute_values::<PointId>(ATTR_ID).map(|ids| ids.as_slice()).unwrap_or_default();

		let mut pool_to_working = vec![None; vector.point_count()];
		for (index, slot) in pool_to_working.iter_mut().enumerate() {
			if !connectivity.is_handle(index as u32) {
				*slot = Some(working.point_ids.len());
				working.point_ids.push(pool_ids.get(index).copied().unwrap_or_default());
				working.point_positions.push(vector.position(index).unwrap_or_default());
			}
		}
		let working_point = |pool_index: u32| pool_to_working.get(pool_index as usize).copied().flatten();

		// One pass so a span that fails to resolve leaves every array at the same length
		for (path_index, path) in paths.iter_element_values().enumerate() {
			let curve_type = curve_type_at(paths, path_index);
			let stride = curve_type.stride();
			let span_ids = path.0.iter_attribute_values::<SegmentId>(ATTR_ID).map(|ids| ids.as_slice()).unwrap_or_default();

			// A path that only carries on the one before it in another curve type is no beginning, so the two rejoin if their types ever match
			let mut begins_path = !continues_previous_path(paths, path_index);
			for span in 0..path.segment_count(curve_type) {
				let Some(segment) = path.segment(&vector.0, curve_type, span) else { continue };
				let (Some(start), Some(end)) = (path.reference(span * stride), path.reference(span * stride + stride)) else {
					continue;
				};
				let (Some(start), Some(end)) = (working_point(start), working_point(end)) else { continue };

				working.segment_ids.push(span_ids.get(span * stride).copied().unwrap_or_default());
				working.segment_handles.push(segment_to_handles(&segment));
				working.segment_start.push(start);
				working.segment_end.push(end);
				working.segment_begins_path.push(begins_path);
				begins_path = false;
			}
		}

		working.colinear = vector.colinear_pairs_with(paths, &connectivity);

		working
	}

	/// The number of segments that every parallel segment array holds.
	fn segment_count(&self) -> usize {
		[self.segment_ids.len(), self.segment_start.len(), self.segment_end.len(), self.segment_handles.len()]
			.into_iter()
			.min()
			.unwrap_or(0)
	}

	/// Hands each removed segment's mark as the beginning of a path to the next kept segment, which begins what is left of that path.
	pub(crate) fn carry_path_beginnings(&mut self, keep: &[bool]) {
		let mut carried = false;
		for (begins, &kept) in self.segment_begins_path.iter_mut().zip(keep) {
			if !kept {
				carried |= *begins;
			} else if carried {
				*begins = true;
				carried = false;
			}
		}
	}

	/// Weaves the working form back into a pool and paths in O(points + segments). Segments chain head to tail into paths:
	/// each runs on into the segment stored after it when that one sets out from its end, and segments left unlinked pair
	/// up where they are the only two loose ends at an anchor. A segment marked as beginning a path joins nothing before it.
	pub fn reweave(self, vector: &mut Vector, paths: &mut List<Path>) {
		*paths = List::new();

		let segment_count = self.segment_count();

		// The pool is built as plain arrays and handed to the list whole, anchors first and each segment's handles after
		let anchor_count = self.point_ids.len().min(self.point_positions.len());
		let mut positions = self.point_positions;
		positions.truncate(anchor_count);
		let mut point_ids = self.point_ids;
		point_ids.truncate(anchor_count);

		let drawable = |index: usize| self.segment_start[index] < anchor_count && self.segment_end[index] < anchor_count;
		let begins_path = |index: usize| self.segment_begins_path.get(index).copied().unwrap_or(false);

		// Each segment's successor and predecessor in its chain, at most one of each. A segment carries on into the one
		// stored after it when that one sets out from its end.
		const NONE: u32 = u32::MAX;
		let (mut successor, mut predecessor) = (vec![NONE; segment_count], vec![NONE; segment_count]);
		for index in (1..segment_count).filter(|&index| drawable(index) && drawable(index - 1)) {
			if self.segment_start[index] == self.segment_end[index - 1] && !begins_path(index) {
				successor[index - 1] = index as u32;
				predecessor[index] = (index - 1) as u32;
			}
		}

		// Segments left unlinked pair up where one arriving and one setting out are the only such two at an anchor, which
		// rejoins a path around segments that were replaced and so are stored out of order
		let (mut loose_arrivals, mut loose_departures) = (vec![(0_u8, NONE); anchor_count], vec![(0_u8, NONE); anchor_count]);
		for index in (0..segment_count).filter(|&index| drawable(index)) {
			if successor[index] == NONE {
				let (count, arrival) = &mut loose_arrivals[self.segment_end[index]];
				(*count, *arrival) = (count.saturating_add(1), index as u32);
			}
			if predecessor[index] == NONE && !begins_path(index) {
				let (count, departure) = &mut loose_departures[self.segment_start[index]];
				(*count, *departure) = (count.saturating_add(1), index as u32);
			}
		}
		for (&(arrivals, arrival), &(departures, departure)) in loose_arrivals.iter().zip(&loose_departures) {
			if arrivals == 1 && departures == 1 && arrival != departure {
				successor[arrival as usize] = departure;
				predecessor[departure as usize] = arrival;
			}
		}

		// Only segments a G1 pair names need their anchors recorded, to narrow the pairs to smooth flags below
		let paired: FxHashSet<SegmentId> = self.colinear.iter().flat_map(|pair| pair.map(|handle| handle.segment)).collect();
		let mut segment_anchors = FxHashMap::default();

		let curve_type_of = |index: usize| match self.segment_handles[index] {
			BezierHandles::Linear => CurveType::Linear,
			BezierHandles::Quadratic { .. } => CurveType::Quadratic,
			BezierHandles::Cubic { .. } => CurveType::Cubic,
		};

		// Each chain is woven where its earliest stored segment sits, so paths keep their order. A path holds one curve
		// type, so a chain changing type starts another path right after, as does one arriving back where it set out.
		let mut runs: Vec<Run> = Vec::new();
		let mut woven = vec![false; segment_count];
		let mut chain_start = usize::MAX;
		for seed in (0..segment_count).filter(|&seed| drawable(seed)) {
			if woven[seed] {
				continue;
			}

			// Back to where the chain begins, which for a loop is the segment the search set out from
			let mut index = seed;
			loop {
				let previous = predecessor[index] as usize;
				if previous >= segment_count || woven[previous] {
					break;
				}
				index = previous;
				if index == seed {
					break;
				}
			}

			// Setting out in another curve type from where the last path ended reads back as carrying it on, so it is woven that way
			let carries_on = runs
				.last()
				.is_some_and(|(curve_type, references, _)| *curve_type != curve_type_of(index) && references.last() == Some(&(self.segment_start[index] as u32)));
			if !carries_on {
				chain_start = self.segment_start[index];
			}
			let mut continues_run = false;
			loop {
				woven[index] = true;
				let (id, start, end) = (self.segment_ids[index], self.segment_start[index], self.segment_end[index]);
				let (curve_type, handle_positions) = match self.segment_handles[index] {
					BezierHandles::Linear => (CurveType::Linear, [None, None]),
					BezierHandles::Quadratic { handle } => (CurveType::Quadratic, [Some(handle), None]),
					BezierHandles::Cubic { handle_start, handle_end } => (CurveType::Cubic, [Some(handle_start), Some(handle_end)]),
				};

				if !continues_run || runs.last().is_none_or(|(run_curve_type, ..)| *run_curve_type != curve_type) {
					runs.push((curve_type, vec![start as u32], Vec::new()));
				}
				continues_run = end != chain_start;

				let Some((_, references, ids)) = runs.last_mut() else { break };
				for position in handle_positions.into_iter().flatten() {
					references.push(positions.len() as u32);
					positions.push(position);
					point_ids.push(PointId::UNNAMED);
				}
				references.push(end as u32);
				ids.push(id);

				if paired.contains(&id) {
					segment_anchors.insert(id, (start as u32, end as u32));
				}

				let next = successor[index] as usize;
				if next >= segment_count || woven[next] {
					break;
				}
				index = next;
			}
		}

		vector.0 = List::new_from_elements(positions);
		if !point_ids.is_empty() {
			vector.0.set_attribute_values(ATTR_ID, point_ids);
		}

		for (curve_type, references, ids) in runs {
			push_path(paths, curve_type, references, ids);
		}

		// G1 pairs narrow to the smooth flag on the anchor both handles face, and other pairings are dropped
		let anchor = |handle: HandleId| {
			let &(start, end) = segment_anchors.get(&handle.segment)?;
			Some(if handle.ty == HandleType::Primary { start } else { end })
		};
		let smooth_anchors: Vec<u32> = self
			.colinear
			.into_iter()
			.filter_map(|[first, second]| anchor(first).zip(anchor(second)))
			.filter_map(|(first, second)| (first == second).then_some(first))
			.collect();

		vector.set_smooth_at_indices(paths, smooth_anchors, true);
	}

	/// Reorders and reorients the segments so each continues the one it meets head to tail, taking the straightest
	/// route through every junction, and marks where each chain begins. This bakes path structure into a segment soup once.
	pub fn chain_by_straightest_route(&mut self) {
		let segment_count = self.segment_count();
		let point_count = self.point_positions.len();

		// Every segment end at each point, named by twice the segment's index plus one for its arriving end
		let mut ends_at: Vec<Vec<usize>> = vec![Vec::new(); point_count];
		for index in 0..segment_count {
			let (start, end) = (self.segment_start[index], self.segment_end[index]);
			if start < point_count && end < point_count {
				ends_at[start].push(index * 2);
				ends_at[end].push(index * 2 + 1);
			}
		}

		// The direction a segment leaves its anchor in from the given end, toward the nearest control point not sitting on the anchor
		let direction = |end: usize| {
			let (index, arriving) = (end / 2, end % 2 == 1);
			let (start, finish) = (self.point_positions[self.segment_start[index]], self.point_positions[self.segment_end[index]]);
			let handles = self.segment_handles[index];
			let (anchor, toward) = if arriving {
				(finish, [handles.end(), handles.start(), Some(start)])
			} else {
				(start, [handles.start(), handles.end(), Some(finish)])
			};

			let along = toward.into_iter().flatten().map(|point| point - anchor).find(|along| along.length_squared() > 1e-12);
			along.unwrap_or_default().normalize_or_zero()
		};

		// Pair off the ends meeting at each point, the straightest pair through a junction first, in O(k² log k) for k ends there
		let mut partner = vec![usize::MAX; segment_count * 2];
		for ends in ends_at.iter().filter(|ends| ends.len() >= 2) {
			let directions: Vec<DVec2> = ends.iter().map(|&end| direction(end)).collect();

			// Straightness is rounded so pairs differing only by float error are equally straight, leaving the earlier stored ends first
			let mut pairs: Vec<(i64, usize, usize)> = (0..ends.len())
				.flat_map(|i| (i + 1..ends.len()).map(move |j| (i, j)))
				.map(|(i, j)| ((-directions[i].dot(directions[j]) * 1e9).round() as i64, i, j))
				.collect();
			pairs.sort_by(|a, b| b.0.cmp(&a.0).then((a.1, a.2).cmp(&(b.1, b.2))));

			let mut paired = vec![false; ends.len()];
			for (_, i, j) in pairs {
				if paired[i] || paired[j] {
					continue;
				}
				paired[i] = true;
				paired[j] = true;
				partner[ends[i]] = ends[j];
				partner[ends[j]] = ends[i];
			}
		}

		// Follow the pairings into chains, each seeded by the earliest unvisited segment and keeping its direction
		let mut visited = vec![false; segment_count];
		let mut order: Vec<(usize, bool)> = Vec::with_capacity(segment_count);
		let mut begins_path = Vec::with_capacity(segment_count);
		for seed in 0..segment_count {
			if std::mem::replace(&mut visited[seed], true) {
				continue;
			}

			let mut chain = vec![(seed, false)];
			let mut closed = false;
			let mut end = seed * 2 + 1;
			loop {
				let next = partner[end];
				if next == usize::MAX {
					break;
				}
				let segment = next / 2;
				if segment == seed {
					closed = true;
					break;
				}
				if std::mem::replace(&mut visited[segment], true) {
					break;
				}

				// Entering at the segment's arriving end crosses it backwards
				let reversed = next % 2 == 1;
				chain.push((segment, reversed));
				end = if reversed { segment * 2 } else { segment * 2 + 1 };
			}

			// An open chain also grows backwards from the seed's start, which is then where it begins
			let mut before = Vec::new();
			let mut end = seed * 2;
			if !closed {
				loop {
					let next = partner[end];
					if next == usize::MAX {
						break;
					}
					let segment = next / 2;
					if std::mem::replace(&mut visited[segment], true) {
						break;
					}

					// Joining at the segment's arriving end means it runs forward into the chain
					let reversed = next % 2 == 0;
					before.push((segment, reversed));
					end = if reversed { segment * 2 + 1 } else { segment * 2 };
				}
			}
			before.reverse();

			begins_path.extend((0..before.len() + chain.len()).map(|position| position == 0));
			order.append(&mut before);
			order.append(&mut chain);
		}

		// Rebuild the segment arrays in chain order, turning around the segments that run against their chain
		let mut reversed_ids = FxHashSet::default();
		let (mut ids, mut starts, mut ends, mut handles) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
		for &(segment, reversed) in &order {
			ids.push(self.segment_ids[segment]);
			let (start, end) = (self.segment_start[segment], self.segment_end[segment]);
			if reversed {
				reversed_ids.insert(self.segment_ids[segment]);
				starts.push(end);
				ends.push(start);
				handles.push(self.segment_handles[segment].reversed());
			} else {
				starts.push(start);
				ends.push(end);
				handles.push(self.segment_handles[segment]);
			}
		}
		self.segment_ids = ids;
		self.segment_start = starts;
		self.segment_end = ends;
		self.segment_handles = handles;
		self.segment_begins_path = begins_path;

		// A turned segment's handles swap sides, so the G1 pairs naming them follow
		for handle in self.colinear.iter_mut().flatten() {
			if reversed_ids.contains(&handle.segment) {
				handle.ty = match handle.ty {
					HandleType::Primary => HandleType::End,
					HandleType::End => HandleType::Primary,
				};
			}
		}
	}
}

// TODO: Eventually remove this document upgrade code
/// Builds storage from the legacy flat arrays: anchors with IDs and positions, segments with IDs, endpoint indices,
/// and absolute handles, plus G1 pairs. Legacy segments carry no path structure, so they are chained by the straightest
/// route through each junction before weaving. Used by document migration.
pub fn vector_from_legacy_parts(
	point_ids: Vec<PointId>,
	point_positions: Vec<DVec2>,
	segment_ids: Vec<SegmentId>,
	segment_start: Vec<usize>,
	segment_end: Vec<usize>,
	segment_handles: Vec<BezierHandles>,
	colinear: Vec<[HandleId; 2]>,
) -> (Vector, List<Path>) {
	let mut working = WorkingVector {
		point_ids,
		point_positions,
		segment_ids,
		segment_start,
		segment_end,
		segment_handles,
		segment_begins_path: Vec::new(),
		colinear,
	};
	working.chain_by_straightest_route();

	let (mut vector, mut paths) = (Vector::default(), List::new());
	working.reweave(&mut vector, &mut paths);

	(vector, paths)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::list::{ATTR_CURVE_TYPE, ATTR_ID, ATTR_SMOOTH, Item};

	fn working(points: usize, segments: &[(usize, usize, BezierHandles)]) -> WorkingVector {
		WorkingVector {
			point_ids: (0..points).map(|_| PointId::generate()).collect(),
			point_positions: (0..points).map(|index| DVec2::new(index as f64, 0.)).collect(),
			segment_ids: segments.iter().map(|_| SegmentId::generate()).collect(),
			segment_start: segments.iter().map(|&(start, ..)| start).collect(),
			segment_end: segments.iter().map(|&(_, end, _)| end).collect(),
			segment_handles: segments.iter().map(|(.., handles)| *handles).collect(),
			segment_begins_path: Vec::new(),
			colinear: Vec::new(),
		}
	}

	fn working_at(positions: &[DVec2], segments: &[(usize, usize, BezierHandles)]) -> WorkingVector {
		let mut working = working(positions.len(), segments);
		working.point_positions = positions.to_vec();

		working
	}

	fn woven(working: WorkingVector) -> (Vector, List<Path>) {
		let (mut vector, mut paths) = (Vector::default(), List::new());
		working.reweave(&mut vector, &mut paths);

		(vector, paths)
	}

	fn references(paths: &List<Path>, index: usize) -> Vec<u32> {
		paths.element(index).expect("the path was woven").references().collect()
	}

	// Four sides that meet head to tail become one closed path, its last reference naming its first
	#[test]
	fn reweave_gathers_a_rectangle_into_one_path() {
		let sides = [
			(0, 1, BezierHandles::Linear),
			(1, 2, BezierHandles::Linear),
			(2, 3, BezierHandles::Linear),
			(3, 0, BezierHandles::Linear),
		];
		let (_, paths) = woven(working(4, &sides));

		assert_eq!(paths.len(), 1);
		assert_eq!(references(&paths, 0), vec![0, 1, 2, 3, 0]);

		let rectangle = paths.element(0).expect("the run became a path");
		assert!(rectangle.is_closed());
		assert_eq!(rectangle.segment_count(CurveType::Linear), 4);
	}

	// A run carries on through a junction, and the third segment leaving that anchor starts its own path
	#[test]
	fn reweave_carries_a_run_through_a_junction() {
		let spokes = [(0, 1, BezierHandles::Linear), (1, 2, BezierHandles::Linear), (1, 3, BezierHandles::Linear)];
		let (_, paths) = woven(working(4, &spokes));

		assert_eq!(paths.len(), 2);
		assert_eq!(references(&paths, 0), vec![0, 1, 2]);
		assert_eq!(references(&paths, 1), vec![1, 3]);
	}

	// Two loops meeting at one anchor stay apart, since a run gives way once it has closed
	#[test]
	fn reweave_keeps_loops_meeting_at_an_anchor_apart() {
		let lobes = [
			(0, 1, BezierHandles::Linear),
			(1, 0, BezierHandles::Linear),
			(0, 2, BezierHandles::Linear),
			(2, 0, BezierHandles::Linear),
		];
		let (_, paths) = woven(working(3, &lobes));

		assert_eq!(paths.len(), 2);
		assert_eq!(references(&paths, 0), vec![0, 1, 0]);
		assert_eq!(references(&paths, 1), vec![0, 2, 0]);
	}

	// One path weaves one stride, so a curve following a line starts its own
	#[test]
	fn reweave_breaks_a_run_at_a_verb_change() {
		let handles = BezierHandles::Cubic {
			handle_start: DVec2::new(1., 1.),
			handle_end: DVec2::new(2., 1.),
		};
		let (_, paths) = woven(working(3, &[(0, 1, BezierHandles::Linear), (1, 2, handles)]));

		assert_eq!(paths.len(), 2);
		assert_eq!(curve_type_at(&paths, 0), CurveType::Linear);
		assert_eq!(curve_type_at(&paths, 1), CurveType::Cubic);
	}

	// Consecutive curves weave into one path, each span's handles filling the references between its anchors
	#[test]
	fn reweave_gathers_cubic_handles_into_one_path() {
		let handles = |offset: f64| BezierHandles::Cubic {
			handle_start: DVec2::new(offset, 1.),
			handle_end: DVec2::new(offset + 1., 1.),
		};
		let (vector, paths) = woven(working(3, &[(0, 1, handles(0.)), (1, 2, handles(2.))]));

		assert_eq!(paths.len(), 1);
		assert_eq!(references(&paths, 0), vec![0, 3, 4, 1, 5, 6, 2]);
		assert_eq!(vector.point_count(), 7);
		assert_eq!(paths.element(0).expect("the run became a path").segment_count(CurveType::Cubic), 2);
	}

	// Beveling appends its corner segments after the sides, and each still finds the one side setting out from its end
	#[test]
	fn reweave_chains_segments_stored_out_of_order() {
		let sides = [(0, 1), (2, 3), (4, 5), (6, 7)];
		let corners = [(1, 2), (3, 4), (5, 6), (7, 0)];
		let segments: Vec<_> = sides.into_iter().chain(corners).map(|(start, end)| (start, end, BezierHandles::Linear)).collect();

		let (_, paths) = woven(working(8, &segments));

		assert_eq!(paths.len(), 1);
		assert_eq!(references(&paths, 0), vec![0, 1, 2, 3, 4, 5, 6, 7, 0]);
	}

	// A point inserted into a path stores its two halves last, and they still weave back into the one path
	#[test]
	fn reweave_keeps_a_path_whole_around_a_replaced_segment() {
		let soup = [(0, 1), (2, 3), (1, 4), (4, 2)].map(|(start, end)| (start, end, BezierHandles::Linear));
		let (_, paths) = woven(working(5, &soup));

		assert_eq!(paths.len(), 1);
		assert_eq!(references(&paths, 0), vec![0, 1, 4, 2, 3]);
	}

	// Where several segments meet, only the one stored next carries a chain on, so a spur ending at a junction stays a spur
	#[test]
	fn reweave_follows_stored_order_through_a_junction() {
		let soup = [(3, 1), (0, 1), (1, 2)].map(|(start, end)| (start, end, BezierHandles::Linear));
		let (_, paths) = woven(working(4, &soup));

		assert_eq!(paths.len(), 2);
		assert_eq!(references(&paths, 0), vec![3, 1]);
		assert_eq!(references(&paths, 1), vec![0, 1, 2]);
	}

	// A chain that closes partway keeps weaving, so the loops and the tail after them all come out
	#[test]
	fn reweave_weaves_all_of_a_chain_that_closes_midway() {
		let soup = [(2, 0), (0, 3), (0, 1), (1, 0), (0, 2)].map(|(start, end)| (start, end, BezierHandles::Linear));
		let (_, paths) = woven(working(4, &soup));

		assert_eq!(paths.len(), 3);
		assert_eq!(references(&paths, 0), vec![0, 1, 0]);
		assert_eq!(references(&paths, 1), vec![0, 2, 0]);
		assert_eq!(references(&paths, 2), vec![0, 3]);
	}

	// A point inserted next to a junction stores its halves last, and the path still runs through where another path begins
	#[test]
	fn reweave_rejoins_a_replaced_segment_beside_a_junction() {
		let mut soup = working(5, &[(0, 1), (1, 3), (1, 4), (4, 2)].map(|(start, end)| (start, end, BezierHandles::Linear)));
		soup.segment_begins_path = vec![true, true, false, false];
		let (_, paths) = woven(soup);

		assert_eq!(paths.len(), 2);
		assert_eq!(references(&paths, 0), vec![0, 1, 4, 2]);
		assert_eq!(references(&paths, 1), vec![1, 3]);
	}

	// No soup loses a segment to the weave, and weaving what was just woven changes nothing
	#[test]
	fn reweave_keeps_every_segment_of_any_soup() {
		let mut state = 0x9E37_79B9_7F4A_7C15_u64;
		let mut below = |limit: usize| {
			state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
			(state >> 33) as usize % limit
		};

		for _ in 0..2000 {
			let points = 1 + below(6);
			let segments: Vec<_> = (0..below(10))
				.map(|_| {
					let handles = match below(3) {
						0 => BezierHandles::Linear,
						1 => BezierHandles::Quadratic {
							handle: DVec2::new(below(5) as f64, 1.),
						},
						_ => BezierHandles::Cubic {
							handle_start: DVec2::new(below(5) as f64, 1.),
							handle_end: DVec2::new(below(5) as f64, -1.),
						},
					};
					(below(points), below(points), handles)
				})
				.collect();
			let mut soup = working(points, &segments);
			soup.segment_begins_path = (0..segments.len()).map(|_| below(4) == 0).collect();

			let (vector, paths) = woven(soup);
			let spans: usize = paths.iter_element_values().enumerate().map(|(index, path)| path.segment_count(curve_type_at(&paths, index))).sum();
			assert_eq!(spans, segments.len(), "{segments:?}");

			let (_, again) = woven(WorkingVector::flatten(&vector, &paths));
			assert_eq!(again, paths, "{segments:?}");
		}
	}

	// A path that exists only because the curve type changed rejoins the one before it once their types match
	#[test]
	fn flatten_lets_a_type_change_rejoin() {
		let curve = BezierHandles::Cubic {
			handle_start: DVec2::new(1., 1.),
			handle_end: DVec2::new(2., 1.),
		};
		let (vector, paths) = woven(working(4, &[(0, 1, BezierHandles::Linear), (1, 2, curve), (2, 3, BezierHandles::Linear)]));
		assert_eq!(paths.len(), 3);

		let mut flattened = WorkingVector::flatten(&vector, &paths);
		assert_eq!(flattened.segment_begins_path, vec![true, false, false]);

		flattened.segment_handles[1] = BezierHandles::Linear;
		let (_, straightened) = woven(flattened);
		assert_eq!(straightened.len(), 1);
		assert_eq!(references(&straightened, 0), vec![0, 1, 2, 3]);
	}

	// Segments meeting tail to tail are never turned around to chain them
	#[test]
	fn reweave_never_reverses_a_segment() {
		let soup = [(0, 1), (2, 1)].map(|(start, end)| (start, end, BezierHandles::Linear));
		let (_, paths) = woven(working(3, &soup));

		assert_eq!(paths.len(), 2);
		assert_eq!(references(&paths, 0), vec![0, 1]);
		assert_eq!(references(&paths, 1), vec![2, 1]);
	}

	// Separate shapes stay where they were, so nothing downstream sees geometry shuffle
	#[test]
	fn reweave_keeps_untouched_shapes_in_order() {
		let triangle = [(0, 1), (1, 2), (2, 0)];
		let open_pair = [(3, 4), (4, 5)];
		let segments: Vec<_> = triangle.into_iter().chain(open_pair).map(|(start, end)| (start, end, BezierHandles::Linear)).collect();

		let (_, paths) = woven(working(6, &segments));

		assert_eq!(paths.len(), 2);
		assert_eq!(references(&paths, 0), vec![0, 1, 2, 0]);
		assert_eq!(references(&paths, 1), vec![3, 4, 5]);
	}

	// A span naming a point the pool does not have drops out of every array at once, keeping them aligned
	#[test]
	fn flatten_drops_an_unresolvable_span_from_every_array() {
		let mut vector = Vector::default();
		for index in 0..2 {
			vector.0.push(Item::new_from_element(DVec2::new(index as f64, 0.)).with_attribute(ATTR_ID, PointId::generate()));
		}

		let mut path = Path::from_references([99, 0, 1]);
		let drawn = SegmentId::generate();
		path.0.set_attribute(ATTR_ID, 0, SegmentId::generate());
		path.0.set_attribute(ATTR_ID, 1, drawn);

		let mut paths = List::new();
		paths.push(Item::new_from_element(path).with_attribute(ATTR_CURVE_TYPE, CurveType::Linear));

		let working = WorkingVector::flatten(&vector, &paths);
		assert_eq!(working.segment_ids, vec![drawn]);
		assert_eq!(working.segment_start, vec![0]);
		assert_eq!(working.segment_end, vec![1]);
	}

	// A segment naming a point the pool lacks is dropped, leaving the run around it woven as one path
	#[test]
	fn unresolvable_segment_leaves_the_run_intact() {
		let mut working = working(3, &[(0, 1, BezierHandles::Linear), (1, 2, BezierHandles::Linear)]);
		working.segment_ids.push(SegmentId::generate());
		working.segment_start.push(1);
		working.segment_end.push(99);
		working.segment_handles.push(BezierHandles::Linear);

		let (_, paths) = woven(working);
		assert_eq!(paths.len(), 1);
		assert_eq!(references(&paths, 0), [0, 1, 2]);
	}

	// Flattened paths welded head to tail stay apart when rewoven, since a path boundary is data of its own
	#[test]
	fn reweave_keeps_flattened_paths_apart() {
		let mut vector = Vector::default();
		for index in 0..4 {
			vector.0.push(Item::new_from_element(DVec2::new(index as f64, 0.)).with_attribute(ATTR_ID, PointId::generate()));
		}
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1, 2], [SegmentId::generate(), SegmentId::generate()]);
		push_path(&mut paths, CurveType::Linear, [2, 3], [SegmentId::generate()]);

		let working = WorkingVector::flatten(&vector, &paths);
		assert_eq!(working.segment_begins_path, vec![true, false, true]);

		let (mut rewoven, mut rewoven_paths) = (Vector::default(), List::new());
		working.reweave(&mut rewoven, &mut rewoven_paths);
		assert_eq!(rewoven_paths.len(), 2);
		assert_eq!(references(&rewoven_paths, 0), [0, 1, 2]);
		assert_eq!(references(&rewoven_paths, 1), [2, 3]);
	}

	// The bar of a T keeps going through the junction, being the straightest route, and the stem starts its own path
	#[test]
	fn straightest_route_continues_through_a_junction() {
		let positions = [DVec2::new(0., 0.), DVec2::new(1., 0.), DVec2::new(2., 0.), DVec2::new(1., 1.)];
		let soup = [(1, 3, BezierHandles::Linear), (0, 1, BezierHandles::Linear), (1, 2, BezierHandles::Linear)];
		let mut working = working_at(&positions, &soup);

		working.chain_by_straightest_route();
		assert_eq!(working.segment_begins_path, vec![true, true, false]);

		let (_, paths) = woven(working);
		assert_eq!(paths.len(), 2);
		assert_eq!(references(&paths, 0), [1, 3]);
		assert_eq!(references(&paths, 1), [0, 1, 2]);
	}

	// A curve whose end handle is retracted onto the junction arrives along its other handle, so it carries on downward rather than along its chord
	#[test]
	fn straightest_route_reads_a_retracted_handle_from_the_other_one() {
		let positions = [DVec2::new(-10., 0.), DVec2::ZERO, DVec2::new(0., -10.), DVec2::new(10., 0.)];
		let curve = BezierHandles::Cubic {
			handle_start: DVec2::new(0., 10.),
			handle_end: DVec2::ZERO,
		};
		let soup = [(0, 1, curve), (1, 3, BezierHandles::Linear), (1, 2, BezierHandles::Linear)];
		let mut working = working_at(&positions, &soup);
		let ids = working.segment_ids.clone();

		working.chain_by_straightest_route();
		assert_eq!(working.segment_ids, vec![ids[0], ids[2], ids[1]]);
		assert_eq!(working.segment_begins_path, vec![true, false, true]);
	}

	// Four segments all drawn into one crossing pair off into two straight lines, turning around the ones drawn the other way
	#[test]
	fn crossing_lines_each_stay_whole() {
		let positions = [DVec2::new(-1., 0.), DVec2::new(1., 0.), DVec2::new(0., -1.), DVec2::new(0., 1.), DVec2::ZERO];
		let soup = [
			(0, 4, BezierHandles::Linear),
			(2, 4, BezierHandles::Linear),
			(1, 4, BezierHandles::Linear),
			(3, 4, BezierHandles::Linear),
		];
		let mut working = working_at(&positions, &soup);
		let ids = working.segment_ids.clone();

		working.chain_by_straightest_route();
		assert_eq!(working.segment_ids, vec![ids[0], ids[2], ids[1], ids[3]]);
		assert_eq!(working.segment_start, vec![0, 4, 2, 4]);
		assert_eq!(working.segment_end, vec![4, 1, 4, 3]);

		let (_, paths) = woven(working);
		assert_eq!(paths.len(), 2);
		assert_eq!(references(&paths, 0), [0, 4, 1]);
		assert_eq!(references(&paths, 1), [2, 4, 3]);
	}

	// A G1 pair follows its segment when that segment is turned around, so the shared anchor still comes out smooth
	#[test]
	fn a_turned_segment_keeps_its_smooth_pair() {
		let cubic = |from: DVec2, to: DVec2| BezierHandles::Cubic {
			handle_start: from + DVec2::new(1., 0.),
			handle_end: to - DVec2::new(1., 0.),
		};
		let positions = [DVec2::new(0., 0.), DVec2::new(3., 0.), DVec2::new(6., 0.)];
		let ids = [SegmentId::generate(), SegmentId::generate()];

		// The second segment runs back toward the shared anchor, so both handles facing it, one on each side, are stored as end handles
		let (_, paths) = vector_from_legacy_parts(
			(0..3).map(|_| PointId::generate()).collect(),
			positions.to_vec(),
			ids.to_vec(),
			vec![0, 2],
			vec![1, 1],
			vec![
				cubic(positions[0], positions[1]),
				BezierHandles::Cubic {
					handle_start: positions[2] - DVec2::X,
					handle_end: positions[1] + DVec2::X,
				},
			],
			vec![[HandleId::end(ids[0]), HandleId::end(ids[1])]],
		);

		assert_eq!(paths.len(), 1);
		assert_eq!(curve_type_at(&paths, 0), CurveType::Cubic);
		assert_eq!(references(&paths, 0), [0, 3, 4, 1, 5, 6, 2]);
		let path = paths.element(0).expect("the chain became one path");
		assert_eq!(path.0.attribute::<bool>(ATTR_SMOOTH, 3), Some(&true));
	}

	// Gathering runs stays invisible to the bridge: the same segments come back in the same order
	#[test]
	fn reweave_round_trips_through_flatten() {
		let sides = [
			(0, 1, BezierHandles::Linear),
			(1, 2, BezierHandles::Linear),
			(2, 3, BezierHandles::Linear),
			(3, 0, BezierHandles::Linear),
		];
		let original = working(4, &sides);
		let (point_ids, segment_ids) = (original.point_ids.clone(), original.segment_ids.clone());

		let (vector, paths) = woven(original);
		let round_tripped = WorkingVector::flatten(&vector, &paths);

		assert_eq!(round_tripped.point_ids, point_ids);
		assert_eq!(round_tripped.segment_ids, segment_ids);
		assert_eq!(round_tripped.segment_start, vec![0, 1, 2, 3]);
		assert_eq!(round_tripped.segment_end, vec![1, 2, 3, 0]);
	}
}
