use super::misc::dvec2_to_point;
use super::path::{CurveType, Path, append_segment, curve_type_at};
use core_types::list::List;
use glam::DVec2;
use kurbo::BezPath;

/// One path's part in a contour, crossed against its stored direction when reversed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContourPart {
	pub path: u32,
	pub reversed: bool,
}

/// Paths stitched end to end into one outline, as [`contours`] describes.
/// A closed contour arrives back at the point it left from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contour {
	pub parts: Vec<ContourPart>,
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
/// path when that one sets out from where it ends, and otherwise into the one path welded to its end where exactly two
/// path ends meet and nothing passes through. Every other meeting is a junction, where paths end. A closed path is
/// always a contour of its own, and a path that cannot be drawn whole takes no part.
pub fn contours(point_count: usize, paths: &List<Path>) -> Vec<Contour> {
	contours_with(point_count, paths, &curve_types(paths))
}

/// The curve type of every path, read once so the stitching below never looks one up per path.
fn curve_types(paths: &List<Path>) -> Vec<CurveType> {
	(0..paths.len()).map(|index| curve_type_at(paths, index)).collect()
}

fn contours_with(point_count: usize, paths: &List<Path>, curve_types: &[CurveType]) -> Vec<Contour> {
	let welds = Welds::new(point_count, paths, curve_types);
	let ends_of = |path: u32| match paths.element(path as usize)?.0.iter_element_values().as_slice() {
		[start, .., end] if welds.drawable[path as usize] => Some((*start as u32, *end as u32)),
		_ => None,
	};

	// The points a part is crossed from and to, in the direction the contour runs along it
	let crossing_ends = |part: ContourPart| ends_of(part.path).map(|(start, end)| if part.reversed { (end, start) } else { (start, end) });

	// The stored neighbor that carries on from this part at the point: onward, the one setting out from it, and backward, the one arriving at it
	let consecutive = |part: ContourPart, point: u32, onward: bool| {
		let step = if onward != part.reversed { 1 } else { -1 };
		let candidate = ContourPart {
			path: u32::try_from(part.path as i64 + step).ok()?,
			reversed: part.reversed,
		};
		let (from, to) = crossing_ends(candidate)?;

		(from != to && if onward { from } else { to } == point).then_some(candidate)
	};

	// The one path welded to this part's end at the point, crossed away from it
	let welded = |part: ContourPart, point: u32, onward: bool| {
		let (path, arriving) = welds.other_end(point, (part.path, onward != part.reversed))?;

		Some(ContourPart { path, reversed: arriving == onward })
	};

	let mut consumed = vec![false; paths.len()];
	let mut found = Vec::new();

	for seed in 0..paths.len() as u32 {
		if consumed[seed as usize] {
			continue;
		}
		let Some((start, end)) = ends_of(seed) else { continue };
		consumed[seed as usize] = true;

		let mut parts = vec![ContourPart { path: seed, reversed: false }];
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
			let unconsumed = |part: &ContourPart| !consumed[part.path as usize];
			let Some(next) = consecutive(last, exit, true).filter(unconsumed).or_else(|| welded(last, exit, true).filter(unconsumed)) else {
				break;
			};
			let Some((_, to)) = crossing_ends(next) else { break };

			consumed[next.path as usize] = true;
			parts.push(next);
			(exit, last) = (to, next);
		}

		// Backward from the seed's start, so the contour begins where the stitched paths run out or meet its end
		let (mut entry, mut first) = (start, parts[0]);
		let mut before = Vec::new();
		while !closed {
			let unconsumed = |part: &ContourPart| !consumed[part.path as usize];
			let Some(previous) = consecutive(first, entry, false).filter(unconsumed).or_else(|| welded(first, entry, false).filter(unconsumed)) else {
				break;
			};
			let Some((from, _)) = crossing_ends(previous) else { break };

			consumed[previous.path as usize] = true;
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

/// Emits one contour as a bezpath, or `None` when its entry point is not in the pool.
fn contour_to_bezpath(pool: &List<DVec2>, paths: &List<Path>, curve_types: &[CurveType], contour: &Contour) -> Option<BezPath> {
	let mut bezpath = BezPath::new();
	bezpath.move_to(dvec2_to_point(*pool.element(contour.entry as usize)?));

	for part in &contour.parts {
		let path = paths.element(part.path as usize)?;
		let curve_type = *curve_types.get(part.path as usize)?;
		let count = path.segment_count(curve_type);

		for step in 0..count {
			let span = if part.reversed { count - 1 - step } else { step };
			let Some(segment) = path.segment(pool, curve_type, span) else { continue };

			append_segment(&mut bezpath, if part.reversed { segment.reverse() } else { segment });
		}
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

#[cfg(test)]
mod tests {
	use super::super::path::pool_and_paths_from_bezpath;
	use super::*;
	use core_types::list::Item;
	use kurbo::PathEl;

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

	/// The anchors each contour visits in order, with whether it closed.
	fn stitched(point_count: usize, paths: &List<Path>) -> Vec<(Vec<u32>, bool)> {
		contours(point_count, paths)
			.into_iter()
			.map(|contour| {
				let mut anchors = vec![contour.entry];
				for part in &contour.parts {
					let path = paths.element(part.path as usize).expect("the part names a path");
					let mut references: Vec<u32> = path.references().collect();
					if part.reversed {
						references.reverse();
					}
					anchors.extend(references.into_iter().skip(1));
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

	// Consecutive paths meeting head to tail are one stroke even through a junction, while the path passing through it stays whole
	#[test]
	fn consecutive_paths_carry_on_through_a_junction() {
		let (_, paths) = linear(5, &[&[0, 1], &[1, 2, 0], &[3, 1, 4]]);

		assert_eq!(stitched(5, &paths), [(vec![0, 1, 2], true), (vec![3, 1, 4], false)]);
	}

	// Crossing a stored path backward carries on backward into the path stored before it, here through the junction at point 1
	#[test]
	fn a_backward_crossing_carries_on_into_the_path_before() {
		let (_, paths) = linear(8, &[&[5, 2], &[0, 1], &[1, 2], &[7, 1]]);

		assert_eq!(stitched(8, &paths), [(vec![5, 2, 1, 0], false), (vec![7, 1], false)]);
	}

	// Two halves both running top to bottom, welded at both ends, render as one outline with the second half crossed backwards
	#[test]
	fn heart_halves_stitch_into_one_outline() {
		let (_, paths) = linear(4, &[&[0, 1, 2], &[0, 3, 2]]);

		assert_eq!(stitched(4, &paths), [(vec![0, 1, 2, 3], true)]);
		assert!(contours(4, &paths)[0].parts[1].reversed);
	}

	// A walk that stops at a junction still closes when stitching backward from its start arrives at that junction
	#[test]
	fn a_contour_closes_when_the_walk_back_meets_its_end() {
		let (_, paths) = linear(5, &[&[0, 1], &[2, 0], &[1, 2], &[3, 2, 4]]);

		assert_eq!(stitched(5, &paths), [(vec![2, 0, 1], true), (vec![3, 2, 4], false)]);
	}

	// A closed path is never stitched onto the path stored before it, even when it sets out from that path's end
	#[test]
	fn a_closed_path_is_never_stitched_on() {
		let (_, paths) = linear(4, &[&[3, 0], &[0, 1, 2, 0]]);

		assert_eq!(stitched(4, &paths), [(vec![3, 0], false), (vec![0, 1, 2], true)]);
	}

	// A path naming a point outside the pool, or ending partway through a segment, is left out whole
	#[test]
	fn a_path_that_cannot_be_drawn_whole_is_left_out() {
		let (pool, paths) = linear(3, &[&[0, 1, 9], &[1, 2]]);

		assert_eq!(stitched(3, &paths), [(vec![1, 2], false)]);
		assert_eq!(contour_bezpaths(&pool, &paths).len(), 1);
	}
}
