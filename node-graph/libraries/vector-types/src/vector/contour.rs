use super::misc::dvec2_to_point;
use super::path::{Path, append_segment, curve_type_at};
use core_types::list::List;
use glam::DVec2;
use kurbo::BezPath;

/// One span of a path as a subpath walks it, running against its stored direction when reversed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
	pub path: u32,
	pub span: u32,
	pub reversed: bool,
}

/// Spans walked end to end across the anchors they share, from one pool point to another.
/// A closed subpath arrives back at the point it left from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subpath {
	pub steps: Vec<Step>,
	pub closed: bool,
	pub entry: u32,
	pub exit: u32,
}

/// A span flattened out of its path, naming the anchors at its two ends.
struct Edge {
	path: u32,
	span: u32,
	start: u32,
	end: u32,
}

/// Each anchor's span ends in the order the spans were woven, consumed as the walk uses them up.
/// An end is named by twice its edge's index, plus one for the end the span arrives at.
struct SpanEnds {
	offsets: Vec<usize>,
	ends: Vec<usize>,
	cursors: Vec<usize>,
	used: Vec<bool>,
	remaining: Vec<usize>,
}

impl SpanEnds {
	fn new(point_count: usize, edges: &[Edge]) -> Self {
		let mut offsets = vec![0; point_count + 1];
		for edge in edges {
			offsets[edge.start as usize + 1] += 1;
			offsets[edge.end as usize + 1] += 1;
		}
		for point in 0..point_count {
			offsets[point + 1] += offsets[point];
		}

		let mut ends = vec![0; edges.len() * 2];
		let mut fill = offsets.clone();
		for (index, edge) in edges.iter().enumerate() {
			for (point, end) in [(edge.start as usize, index * 2), (edge.end as usize, index * 2 + 1)] {
				ends[fill[point]] = end;
				fill[point] += 1;
			}
		}

		let remaining = (0..point_count).map(|point| offsets[point + 1] - offsets[point]).collect();

		Self {
			cursors: offsets[..point_count].to_vec(),
			offsets,
			ends,
			used: vec![false; edges.len() * 2],
			remaining,
		}
	}

	/// Uses up the earliest woven span end still waiting at the point.
	fn take_earliest(&mut self, point: usize) -> Option<usize> {
		while self.cursors[point] < self.offsets[point + 1] {
			let end = self.ends[self.cursors[point]];
			self.cursors[point] += 1;

			if !self.used[end] {
				self.used[end] = true;
				self.remaining[point] -= 1;
				return Some(end);
			}
		}

		None
	}

	/// Uses up one particular span end, which is the far side of a span the walk just crossed.
	fn take(&mut self, point: usize, end: usize) {
		if !self.used[end] {
			self.used[end] = true;
			self.remaining[point] -= 1;
		}
	}
}

/// Walks every span into subpaths in O(points + spans): open ones first, each from a point one span end meets,
/// then the rest from the lowest point with spans left. A walk follows the earliest woven span waiting at each
/// point, backwards when that span ends there, and closes on arriving back where it began.
pub fn subpaths(point_count: usize, paths: &List<Path>) -> Vec<Subpath> {
	let mut edges = Vec::new();
	for (path_index, path) in paths.iter_element_values().enumerate() {
		let curve_type = curve_type_at(paths, path_index);
		let stride = curve_type.stride();

		for span in 0..path.segment_count(curve_type) {
			let references = (span * stride..=(span + 1) * stride).map(|index| path.reference(index));
			let Some(references) = references.collect::<Option<Vec<u32>>>() else { continue };

			// A span naming a point outside the pool cannot be drawn, so it takes no part in the walk
			if references.iter().any(|&reference| reference as usize >= point_count) {
				continue;
			}
			let (Some(&start), Some(&end)) = (references.first(), references.last()) else { continue };

			edges.push(Edge {
				path: path_index as u32,
				span: span as u32,
				start,
				end,
			});
		}
	}

	let mut span_ends = SpanEnds::new(point_count, &edges);
	let mut found = Vec::new();
	let (mut skip, mut open_ends_done) = (0, false);

	loop {
		let mut entry = None;
		if !open_ends_done {
			entry = (skip..point_count).find(|&point| span_ends.remaining[point] == 1);
			open_ends_done = entry.is_none();
			skip = entry.map_or(0, |point| point + 1);
		}
		if entry.is_none() {
			entry = (skip..point_count).find(|&point| span_ends.remaining[point] > 0);
			skip = entry.unwrap_or(point_count);
		}
		let Some(entry) = entry else { break };

		let (mut point, mut steps, mut closed) = (entry, Vec::new(), false);
		while let Some(end) = span_ends.take_earliest(point) {
			let edge = &edges[end / 2];
			let reversed = end % 2 == 1;
			let next = if reversed { edge.start } else { edge.end } as usize;

			steps.push(Step {
				path: edge.path,
				span: edge.span,
				reversed,
			});
			span_ends.take(next, end ^ 1);
			point = next;

			if next == entry {
				closed = true;
				break;
			}
		}

		found.push(Subpath {
			steps,
			closed,
			entry: entry as u32,
			exit: point as u32,
		});
	}

	found
}

/// Emits one subpath as a bezpath, or `None` when it has no drawable geometry.
pub fn subpath_to_bezpath(pool: &List<DVec2>, paths: &List<Path>, subpath: &Subpath) -> Option<BezPath> {
	let mut bezpath = BezPath::new();
	bezpath.move_to(dvec2_to_point(*pool.element(subpath.entry as usize)?));

	for step in &subpath.steps {
		let path = paths.element(step.path as usize)?;
		let segment = path.segment(pool, curve_type_at(paths, step.path as usize), step.span as usize)?;

		append_segment(&mut bezpath, if step.reversed { segment.reverse() } else { segment });
	}

	if subpath.closed {
		bezpath.close_path();
	}

	Some(bezpath)
}

/// Assembles the paths into contours for rendering, one bezpath per subpath in the order they are walked.
pub fn contours(pool: &List<DVec2>, paths: &List<Path>) -> Vec<BezPath> {
	subpaths(pool.len(), paths).iter().filter_map(|subpath| subpath_to_bezpath(pool, paths, subpath)).collect()
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

	/// The anchors each subpath visits, with whether it closed.
	fn walked(point_count: usize, paths: &List<Path>) -> Vec<(Vec<u32>, bool)> {
		subpaths(point_count, paths)
			.into_iter()
			.map(|subpath| {
				let mut anchors = vec![subpath.entry];
				for step in &subpath.steps {
					let path = paths.element(step.path as usize).expect("the step names a path");
					let (start, end) = (path.reference(step.span as usize), path.reference(step.span as usize + 1));
					anchors.push(if step.reversed { start } else { end }.expect("the span has both ends"));
				}
				if subpath.closed {
					anchors.pop();
				}

				(anchors, subpath.closed)
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

		let assembled = contours(&pool, &paths);
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

		assert_eq!(contours(&pool, &paths).len(), 2);
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

		let assembled = contours(&pool, &paths);
		assert_eq!(assembled.len(), 1);
		assert!(matches!(assembled[0].elements(), [PathEl::MoveTo(_), PathEl::LineTo(_), PathEl::CurveTo(..)]));
	}

	// A loop leaves from its lowest point along the earliest span there, which it crosses backwards when that span ends at the point
	#[test]
	fn loop_walks_backwards_when_its_earliest_span_arrives() {
		let (_, paths) = linear(4, &[&[1, 2, 3, 0, 1]]);

		assert_eq!(walked(4, &paths), [(vec![0, 3, 2, 1], true)]);
	}

	// An open subpath leaves from its lower end even when that is where it was woven to
	#[test]
	fn open_subpath_leaves_from_its_lower_end() {
		let (_, paths) = linear(4, &[&[0, 1, 2], &[3, 0]]);

		assert_eq!(walked(4, &paths), [(vec![2, 1, 0, 3], false)]);
	}

	// Open subpaths lead, however high their points, and loops follow in the order of their lowest points
	#[test]
	fn open_subpaths_lead_the_loops() {
		let (_, paths) = linear(8, &[&[5, 6, 7, 5], &[0, 1, 2, 0], &[3, 4]]);

		assert_eq!(walked(8, &paths), [(vec![3, 4], false), (vec![0, 1, 2], true), (vec![5, 6, 7], true)]);
	}

	// Loops that touch an outline are walked apart from it, since the outline's own spans were woven first
	#[test]
	fn touching_loops_walk_apart_from_the_outline() {
		let (_, paths) = linear(6, &[&[0, 1, 2, 3, 0], &[1, 4, 1], &[3, 5, 3]]);

		assert_eq!(walked(6, &paths), [(vec![0, 1, 2, 3], true), (vec![1, 4], true), (vec![3, 5], true)]);
	}

	// A walk carries on through a junction along its earliest span and ends where none are left
	#[test]
	fn junction_is_walked_through_along_its_earliest_span() {
		let (_, paths) = linear(4, &[&[0, 3], &[1, 3], &[2, 3]]);

		assert_eq!(walked(4, &paths), [(vec![0, 3, 1], false), (vec![2, 3], false)]);
	}

	// A walk that arrives back at a point other than the one it left from stays open
	#[test]
	fn loop_on_a_tail_stays_open() {
		let (_, paths) = linear(4, &[&[0, 1, 2, 3, 1]]);

		assert_eq!(walked(4, &paths), [(vec![0, 1, 2, 3, 1], false)]);
	}

	// A span naming a point outside the pool is left out of the walk
	#[test]
	fn dangling_span_is_left_out() {
		let (_, paths) = linear(3, &[&[0, 1, 9], &[1, 2]]);

		assert_eq!(walked(3, &paths), [(vec![0, 1, 2], false)]);
	}
}
