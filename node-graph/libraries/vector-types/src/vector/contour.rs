use super::misc::dvec2_to_point;
use super::path::{CurveType, Path, append_segment, curve_types_of, path_flows};
use core_types::list::List;
use glam::DVec2;
use kurbo::{BezPath, PathSeg};

/// Paths stitched end to end into one outline, as [`contours`] describes.
/// A closed contour arrives back at the point it left from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contour {
	/// The stored paths it runs along in order, each in its stored direction.
	pub parts: Vec<u32>,
	pub closed: bool,
	pub entry: u32,
	pub exit: u32,
}

/// The path ends meeting at each pool point, kept up to the two a weld can stitch, beside every spoke there.
struct Welds {
	ends: Vec<[(u32, bool); 2]>,
	end_count: Vec<u8>,
	spokes: Vec<u8>,
	/// Which paths can be drawn whole: at least one segment, no partial one, and every reference inside the pool.
	drawable: Vec<bool>,
}

impl Welds {
	fn new(point_count: usize, paths: &List<Path>, curve_types: &[CurveType]) -> Self {
		let mut welds = Self {
			ends: vec![[(0, false); 2]; point_count],
			end_count: vec![0; point_count],
			spokes: vec![0; point_count],
			drawable: vec![false; paths.len()],
		};

		for (path_index, path) in paths.iter_element_values().enumerate() {
			let references = path.0.iter_element_values().as_slice();
			let stride = curve_types.get(path_index).copied().unwrap_or_default().stride();
			let last = references.len().saturating_sub(1);
			if last == 0 || !last.is_multiple_of(stride) || references.iter().any(|&point| point as usize >= point_count) {
				continue;
			}
			welds.drawable[path_index] = true;

			for (index, &point) in references.iter().enumerate().step_by(stride) {
				let Some(spokes) = welds.spokes.get_mut(point as usize) else { continue };
				*spokes = spokes.saturating_add(u8::from(index > 0) + u8::from(index < last));
			}

			// A closed path's two ends are one anchor, which welds with nothing
			let (start, end) = (references[0], references[last]);
			if start != end {
				welds.add_end(start as u32, path_index as u32, false);
				welds.add_end(end as u32, path_index as u32, true);
			}
		}

		welds
	}

	fn add_end(&mut self, point: u32, path: u32, arriving: bool) {
		let Some(count) = self.end_count.get_mut(point as usize) else { return };
		if let Some(slot) = self.ends[point as usize].get_mut(*count as usize) {
			*slot = (path, arriving);
		}
		*count = count.saturating_add(1);
	}

	/// The one other path end at the point, when exactly two path ends meet there and nothing passes through.
	fn other_end(&self, point: u32, this: (u32, bool)) -> Option<(u32, bool)> {
		let point = point as usize;
		if *self.end_count.get(point)? != 2 || *self.spokes.get(point)? != 2 {
			return None;
		}

		self.ends[point].iter().copied().find(|&end| end != this)
	}
}

/// Stitches the paths into contours in O(points + references), each seeded by the earliest stored path not yet taken
/// and running in that path's direction until it arrives back where it set out. A path runs on into the next stored
/// path when that one carries on its path flow, and otherwise into the one path setting out where it ends, if exactly
/// two path ends meet there and nothing passes through. Every other meeting is a junction, where paths end. A closed
/// path is always a contour of its own, and a path that cannot be drawn whole takes no part.
pub fn contours(point_count: usize, paths: &List<Path>) -> Vec<Contour> {
	contours_with(point_count, paths, &curve_types(paths))
}

/// The curve type of every path, read once so the stitching below never looks one up per path.
fn curve_types(paths: &List<Path>) -> Vec<CurveType> {
	curve_types_of(paths).collect()
}

fn contours_with(point_count: usize, paths: &List<Path>, curve_types: &[CurveType]) -> Vec<Contour> {
	let welds = Welds::new(point_count, paths, curve_types);
	let ends_of = |path: u32| match paths.element(path as usize)?.0.iter_element_values().as_slice() {
		[start, .., end] if welds.drawable[path as usize] => Some((*start as u32, *end as u32)),
		_ => None,
	};

	// The stored neighbor carrying on this path's flow: onward, the next setting out from the point, and backward, the previous arriving at it
	let in_flow = |path: u32, point: u32, onward: bool| {
		let neighbor = if onward { path.checked_add(1)? } else { path.checked_sub(1)? };
		let (from, to) = ends_of(neighbor)?;
		let changes_curve_type = curve_types.get(neighbor as usize) != curve_types.get(path as usize);

		(from != to && changes_curve_type && if onward { from } else { to } == point).then_some(neighbor)
	};

	// The one path welded to this one at the point, running the same way
	let welded = |path: u32, point: u32, onward: bool| {
		let (other, arriving) = welds.other_end(point, (path, onward))?;

		(arriving != onward).then_some(other)
	};

	let mut consumed = vec![false; paths.len()];
	let mut found = Vec::new();

	for seed in 0..paths.len() as u32 {
		if consumed[seed as usize] {
			continue;
		}
		let Some((start, end)) = ends_of(seed) else { continue };
		consumed[seed as usize] = true;

		let mut parts = vec![seed];
		if start == end {
			found.push(Contour {
				parts,
				closed: true,
				entry: start,
				exit: start,
			});
			continue;
		}

		// Onward from the seed's end until the contour arrives back where it set out or nothing carries on
		let (mut exit, mut last) = (end, parts[0]);
		let mut closed = false;
		loop {
			if exit == start {
				closed = true;
				break;
			}
			let unconsumed = |path: &u32| !consumed[*path as usize];
			let Some(next) = in_flow(last, exit, true).filter(unconsumed).or_else(|| welded(last, exit, true).filter(unconsumed)) else {
				break;
			};
			let Some((_, to)) = ends_of(next) else { break };

			consumed[next as usize] = true;
			parts.push(next);
			(exit, last) = (to, next);
		}

		// Backward from the seed's start, so the contour begins where the stitched paths run out or meet its end
		let (mut entry, mut first) = (start, parts[0]);
		let mut before = Vec::new();
		while !closed {
			let unconsumed = |path: &u32| !consumed[*path as usize];
			let Some(previous) = in_flow(first, entry, false).filter(unconsumed).or_else(|| welded(first, entry, false).filter(unconsumed)) else {
				break;
			};
			let Some((from, _)) = ends_of(previous) else { break };

			consumed[previous as usize] = true;
			before.push(previous);
			(entry, first) = (from, previous);
			closed = entry == exit;
		}
		before.reverse();
		before.append(&mut parts);

		found.push(Contour {
			parts: before,
			closed,
			entry,
			exit: if closed { entry } else { exit },
		});
	}

	found
}

/// The segments a contour runs along in order, each beside the stored path it comes from.
fn contour_segments<'a>(pool: &'a List<DVec2>, paths: &'a List<Path>, curve_types: &'a [CurveType], contour: &'a Contour) -> impl Iterator<Item = (u32, PathSeg)> + 'a {
	contour.parts.iter().flat_map(move |&part| {
		let path = paths.element(part as usize);
		let curve_type = curve_types.get(part as usize).copied().unwrap_or_default();
		let count = path.map_or(0, |path| path.segment_count(curve_type));

		(0..count).filter_map(move |span| Some((part, path?.segment(pool, curve_type, span)?)))
	})
}

/// Emits one contour as a bezpath, or `None` when its entry point is not in the pool.
fn contour_to_bezpath(pool: &List<DVec2>, paths: &List<Path>, curve_types: &[CurveType], contour: &Contour) -> Option<BezPath> {
	let mut bezpath = BezPath::new();
	bezpath.move_to(dvec2_to_point(*pool.element(contour.entry as usize)?));

	for (_, segment) in contour_segments(pool, paths, curve_types, contour) {
		append_segment(&mut bezpath, segment);
	}

	if contour.closed {
		bezpath.close_path();
	}

	Some(bezpath)
}

/// Every contour as a bezpath, in the order the paths are stored.
pub fn contour_bezpaths(pool: &List<DVec2>, paths: &List<Path>) -> Vec<BezPath> {
	let curve_types = curve_types(paths);
	contours_with(pool.len(), paths, &curve_types)
		.iter()
		.filter_map(|contour| contour_to_bezpath(pool, paths, &curve_types, contour))
		.collect()
}

/// One contour's segments in the order and direction it runs along them.
#[derive(Clone, Debug, PartialEq)]
pub struct OrientedContour {
	pub segments: Vec<PathSeg>,
	/// Where in `segments` the contour crosses from one path flow into another. A closed contour lists 0 when it arrives back at its start from another flow.
	pub flow_joins: Vec<usize>,
	pub closed: bool,
}

/// Every contour as the segments it runs along, in the order [`contours`] finds them, which is also the order Separate Paths splits them into.
pub fn oriented_contours(pool: &List<DVec2>, paths: &List<Path>) -> Vec<OrientedContour> {
	let curve_types = curve_types(paths);

	// The path flow each stored path belongs to
	let mut flow_of_path = vec![0; paths.len()];
	for (flow, range) in path_flows(paths).into_iter().enumerate() {
		if let Some(members) = flow_of_path.get_mut(range) {
			members.fill(flow);
		}
	}
	let flow_of = |path: u32| flow_of_path.get(path as usize).copied().unwrap_or_default();

	contours_with(pool.len(), paths, &curve_types)
		.iter()
		.map(|contour| {
			let mut segments = Vec::new();
			let mut flow_joins = Vec::new();
			let mut previous_flow = None;
			for (path, segment) in contour_segments(pool, paths, &curve_types, contour) {
				let flow = flow_of(path);
				if previous_flow.is_some_and(|previous| previous != flow) {
					flow_joins.push(segments.len());
				}
				previous_flow = Some(flow);

				segments.push(segment);
			}

			if contour.closed
				&& let (Some(&first), Some(&last)) = (contour.parts.first(), contour.parts.last())
				&& flow_of(first) != flow_of(last)
			{
				flow_joins.insert(0, 0);
			}

			OrientedContour {
				segments,
				flow_joins,
				closed: contour.closed,
			}
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::super::misc::point_to_dvec2;
	use super::super::path::{pool_and_paths_from_bezpath, push_path};
	use super::*;
	use core_types::list::Item;
	use kurbo::{ParamCurve, PathEl};

	fn bezpath_of(build: impl FnOnce(&mut BezPath)) -> BezPath {
		let mut bezpath = BezPath::new();
		build(&mut bezpath);

		bezpath
	}

	/// Linear paths over a pool of the given size, one path per list of references.
	fn linear(point_count: usize, paths: &[&[u32]]) -> (List<DVec2>, List<Path>) {
		let pool = (0..point_count).map(|index| Item::new_from_element(DVec2::new(index as f64, (index * index) as f64))).collect();
		let paths = paths.iter().map(|references| Item::new_from_element(Path::from_references(references.iter().copied()))).collect();

		(pool, paths)
	}

	/// Paths in the given curve types, one path per list of references.
	fn woven(paths: &[(CurveType, &[u32])]) -> List<Path> {
		let mut woven = List::new();
		for &(curve_type, references) in paths {
			push_path(&mut woven, curve_type, references.iter().copied(), []);
		}

		woven
	}

	/// The anchors each contour visits in order, with whether it closed.
	fn stitched(point_count: usize, paths: &List<Path>) -> Vec<(Vec<u32>, bool)> {
		contours(point_count, paths)
			.into_iter()
			.map(|contour| {
				let mut anchors = vec![contour.entry];
				for &part in &contour.parts {
					let path = paths.element(part as usize).expect("the part names a path");
					anchors.extend(path.references().skip(1));
				}
				if contour.closed {
					anchors.pop();
				}

				(anchors, contour.closed)
			})
			.collect()
	}

	// The three paths a mixed-verb subpath weaves into rejoin as the one contour the artist drew
	#[test]
	fn split_subpath_rejoins_as_one_contour() {
		let source = bezpath_of(|bezpath| {
			bezpath.move_to((0., 0.));
			bezpath.line_to((10., 0.));
			bezpath.curve_to((15., 5.), (15., 10.), (10., 15.));
			bezpath.close_path();
		});
		let (pool, paths) = pool_and_paths_from_bezpath(&source);
		assert_eq!(paths.len(), 3);

		let assembled = contour_bezpaths(&pool, &paths);
		assert_eq!(assembled.len(), 1);
		assert!(matches!(
			assembled[0].elements(),
			[PathEl::MoveTo(_), PathEl::LineTo(_), PathEl::CurveTo(..), PathEl::LineTo(_), PathEl::ClosePath]
		));
	}

	// Separate shapes stay separate contours
	#[test]
	fn disjoint_shapes_emit_separately() {
		let source = bezpath_of(|bezpath| {
			bezpath.move_to((0., 0.));
			bezpath.line_to((10., 0.));
			bezpath.line_to((10., 10.));
			bezpath.close_path();
			bezpath.move_to((20., 20.));
			bezpath.line_to((30., 20.));
			bezpath.close_path();
		});
		let (pool, paths) = pool_and_paths_from_bezpath(&source);

		assert_eq!(contour_bezpaths(&pool, &paths).len(), 2);
	}

	// An open subpath emits one contour that stays open
	#[test]
	fn open_subpath_joins_without_closing() {
		let source = bezpath_of(|bezpath| {
			bezpath.move_to((0., 0.));
			bezpath.line_to((10., 0.));
			bezpath.curve_to((15., 5.), (15., 10.), (10., 15.));
		});
		let (pool, paths) = pool_and_paths_from_bezpath(&source);
		assert_eq!(paths.len(), 2);

		let assembled = contour_bezpaths(&pool, &paths);
		assert_eq!(assembled.len(), 1);
		assert!(matches!(assembled[0].elements(), [PathEl::MoveTo(_), PathEl::LineTo(_), PathEl::CurveTo(..)]));
	}

	// A loop is read from the anchor it is stored from, in the direction it is stored in
	#[test]
	fn a_loop_keeps_its_stored_start_and_direction() {
		let (_, paths) = linear(4, &[&[1, 2, 3, 0, 1]]);

		assert_eq!(stitched(4, &paths), [(vec![1, 2, 3, 0], true)]);
	}

	// A path welded ahead of the first one is stitched on in front, running its own stored direction
	#[test]
	fn a_welded_path_is_stitched_on_in_front() {
		let (_, paths) = linear(4, &[&[0, 1, 2], &[3, 0]]);

		assert_eq!(stitched(4, &paths), [(vec![3, 0, 1, 2], false)]);
	}

	// Contours come out in the order their paths are stored, open or closed
	#[test]
	fn contours_follow_path_order() {
		let (_, paths) = linear(8, &[&[5, 6, 7, 5], &[0, 1, 2, 0], &[3, 4]]);

		assert_eq!(stitched(8, &paths), [(vec![5, 6, 7], true), (vec![0, 1, 2], true), (vec![3, 4], false)]);
	}

	// Loops that touch an outline are each their own contour, since a closed path welds with nothing
	#[test]
	fn touching_loops_stay_apart_from_the_outline() {
		let (_, paths) = linear(6, &[&[0, 1, 2, 3, 0], &[1, 4, 1], &[3, 5, 3]]);

		assert_eq!(stitched(6, &paths), [(vec![0, 1, 2, 3], true), (vec![1, 4], true), (vec![3, 5], true)]);
	}

	// Three paths meeting at one anchor make a junction, where every one of them ends
	#[test]
	fn every_path_ends_at_a_junction() {
		let (_, paths) = linear(4, &[&[0, 3], &[1, 3], &[2, 3]]);

		assert_eq!(stitched(4, &paths), [(vec![0, 3], false), (vec![1, 3], false), (vec![2, 3], false)]);
	}

	// A path passing through a junction stays whole, and a path ending there is not stitched onto it
	#[test]
	fn a_path_through_a_junction_stays_whole() {
		let (_, paths) = linear(5, &[&[0, 1, 2, 3, 1], &[4, 1]]);

		assert_eq!(stitched(5, &paths), [(vec![0, 1, 2, 3, 1], false), (vec![4, 1], false)]);
	}

	// Paths stored one after another still end at a junction they meet at, since only a path flow carries on through one
	#[test]
	fn stored_neighbors_end_at_a_junction() {
		let (_, paths) = linear(5, &[&[0, 1], &[1, 2], &[3, 1, 4]]);

		assert_eq!(stitched(5, &paths), [(vec![0, 1], false), (vec![1, 2], false), (vec![3, 1, 4], false)]);
	}

	// A path flow changing curve type at a junction carries on through it as one contour
	#[test]
	fn a_path_flow_carries_on_through_a_junction() {
		let paths = woven(&[(CurveType::Linear, &[0, 1]), (CurveType::Quadratic, &[1, 5, 2]), (CurveType::Linear, &[3, 1, 4])]);

		let parts: Vec<Vec<u32>> = contours(6, &paths).into_iter().map(|contour| contour.parts).collect();
		assert_eq!(parts, [vec![0, 1], vec![2]]);
	}

	// Paths both arriving at an anchor, or both leaving it, stay apart, since joining them would cross one backward
	#[test]
	fn paths_running_against_each_other_stay_apart() {
		let (_, arriving) = linear(3, &[&[0, 1], &[2, 1]]);
		assert_eq!(stitched(3, &arriving), [(vec![0, 1], false), (vec![2, 1], false)]);

		let (_, leaving) = linear(3, &[&[1, 0], &[1, 2]]);
		assert_eq!(stitched(3, &leaving), [(vec![1, 0], false), (vec![1, 2], false)]);
	}

	// Two halves running around an outline stitch into it, while two both running top to bottom stay two paths
	#[test]
	fn heart_halves_stitch_only_when_running_around_the_outline() {
		let (_, around) = linear(4, &[&[0, 1, 2], &[2, 3, 0]]);
		assert_eq!(stitched(4, &around), [(vec![0, 1, 2, 3], true)]);

		let (_, downward) = linear(4, &[&[0, 1, 2], &[0, 3, 2]]);
		assert_eq!(stitched(4, &downward), [(vec![0, 1, 2], false), (vec![0, 3, 2], false)]);
	}

	// A walk that stops at a junction still closes when stitching backward from its start arrives at that junction
	#[test]
	fn a_contour_closes_when_the_walk_back_meets_its_end() {
		let (_, paths) = linear(5, &[&[0, 1], &[2, 0], &[1, 2], &[3, 2, 4]]);

		assert_eq!(stitched(5, &paths), [(vec![2, 0, 1], true), (vec![3, 2, 4], false)]);
	}

	// A closed path is never stitched onto the path stored before it, even when it carries on that path's flow
	#[test]
	fn a_closed_path_is_never_stitched_on() {
		let paths = woven(&[(CurveType::Linear, &[3, 0]), (CurveType::Cubic, &[0, 1, 2, 0])]);

		assert_eq!(stitched(4, &paths), [(vec![3, 0], false), (vec![0, 1, 2], true)]);
	}

	// A path naming a point outside the pool, or ending partway through a segment, is left out whole
	#[test]
	fn a_path_that_cannot_be_drawn_whole_is_left_out() {
		let (pool, paths) = linear(3, &[&[0, 1, 9], &[1, 2]]);

		assert_eq!(stitched(3, &paths), [(vec![1, 2], false)]);
		assert_eq!(contour_bezpaths(&pool, &paths).len(), 1);
	}

	// A subpath split across curve types is one path flow, so it runs as one contour with no flow joins
	#[test]
	fn one_flow_in_several_curve_types_has_no_joins() {
		let source = bezpath_of(|bezpath| {
			bezpath.move_to((0., 0.));
			bezpath.line_to((10., 0.));
			bezpath.curve_to((15., 5.), (15., 10.), (10., 15.));
			bezpath.close_path();
		});
		let (pool, paths) = pool_and_paths_from_bezpath(&source);

		let oriented = oriented_contours(&pool, &paths);
		assert_eq!(oriented.len(), 1);
		assert_eq!(oriented[0].segments.len(), 3);
		assert!(oriented[0].closed);
		assert!(oriented[0].flow_joins.is_empty());
	}

	// Two halves welded at both ends join at each weld, with the contour running along their segments in order
	#[test]
	fn welded_flows_join_where_they_meet() {
		let (pool, paths) = linear(4, &[&[0, 1, 2], &[2, 3, 0]]);

		let oriented = oriented_contours(&pool, &paths);
		assert_eq!(oriented.len(), 1);
		assert_eq!(oriented[0].flow_joins, [0, 2]);

		let starts: Vec<DVec2> = oriented[0].segments.iter().map(|segment| point_to_dvec2(segment.start())).collect();
		let pool_positions: Vec<DVec2> = [0, 1, 2, 3].iter().map(|&index| *pool.element(index).expect("the point is in the pool")).collect();
		assert_eq!(starts, pool_positions);
	}

	// An open contour never joins at its start, only where a welded path carries it on
	#[test]
	fn an_open_contour_joins_only_inside() {
		let (pool, paths) = linear(4, &[&[0, 1, 2], &[3, 0]]);

		let oriented = oriented_contours(&pool, &paths);
		assert_eq!(oriented.len(), 1);
		assert!(!oriented[0].closed);
		assert_eq!(oriented[0].flow_joins, [1]);
	}
}
