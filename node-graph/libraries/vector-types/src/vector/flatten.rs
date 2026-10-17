use super::misc::{BezierHandles, HandleId, HandleType, segment_to_handles};
use super::path::{CurveType, Path, curve_type_at, push_path};
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
			}
		}

		working.colinear = vector.colinear_pairs_with(paths, &connectivity);

		working
	}

	/// Weaves the working form back into a pool and paths, gathering consecutive segments that meet head to tail into one path.
	pub fn reweave(self, vector: &mut Vector, paths: &mut List<Path>) {
		*paths = List::new();

		// The pool is built as plain arrays and handed to the list whole, anchors first and each segment's handles after
		let anchor_count = self.point_ids.len().min(self.point_positions.len());
		let mut positions = self.point_positions;
		positions.truncate(anchor_count);
		let mut point_ids = self.point_ids;
		point_ids.truncate(anchor_count);

		let mut runs: Vec<Run> = Vec::new();

		// Only segments a G1 pair names need their anchors recorded, to narrow the pairs to smooth flags below
		let paired: FxHashSet<SegmentId> = self.colinear.iter().flat_map(|pair| pair.map(|handle| handle.segment)).collect();
		let mut segment_anchors = FxHashMap::default();

		let ends = self.segment_start.iter().zip(&self.segment_end);
		for ((&id, (&start, &end)), handles) in self.segment_ids.iter().zip(ends).zip(&self.segment_handles) {
			if start >= anchor_count || end >= anchor_count {
				continue;
			}

			let (curve_type, handle_positions) = match *handles {
				BezierHandles::Linear => (CurveType::Linear, Vec::new()),
				BezierHandles::Quadratic { handle } => (CurveType::Quadratic, vec![handle]),
				BezierHandles::Cubic { handle_start, handle_end } => (CurveType::Cubic, vec![handle_start, handle_end]),
			};

			// Runs stay in segment order, since the order spans are woven in decides how subpaths are walked.
			// A run carries on through a junction and gives way only once it has closed.
			let continues_run = runs
				.last()
				.is_some_and(|(run_curve_type, references, _)| *run_curve_type == curve_type && references.last() == Some(&(start as u32)) && references.first() != references.last());
			if !continues_run {
				runs.push((curve_type, vec![start as u32], Vec::new()));
			}

			let Some((_, references, ids)) = runs.last_mut() else { continue };
			for position in handle_positions {
				references.push(positions.len() as u32);
				positions.push(position);
				point_ids.push(PointId::UNNAMED);
			}
			references.push(end as u32);
			ids.push(id);

			if paired.contains(&id) {
				segment_anchors.insert(id, (start as u32, end as u32));
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
}

// TODO: Eventually remove this document upgrade code
/// Builds storage from the legacy flat arrays: anchors with IDs and positions, segments with IDs,
/// endpoint indices, and absolute handles, plus G1 pairs. Used by document migration.
pub fn vector_from_legacy_parts(
	point_ids: Vec<PointId>,
	point_positions: Vec<DVec2>,
	segment_ids: Vec<SegmentId>,
	segment_start: Vec<usize>,
	segment_end: Vec<usize>,
	segment_handles: Vec<BezierHandles>,
	colinear: Vec<[HandleId; 2]>,
) -> (Vector, List<Path>) {
	let working = WorkingVector {
		point_ids,
		point_positions,
		segment_ids,
		segment_start,
		segment_end,
		segment_handles,
		colinear,
	};

	let (mut vector, mut paths) = (Vector::default(), List::new());
	working.reweave(&mut vector, &mut paths);

	(vector, paths)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::list::{ATTR_CURVE_TYPE, ATTR_ID, Item};

	fn working(points: usize, segments: &[(usize, usize, BezierHandles)]) -> WorkingVector {
		WorkingVector {
			point_ids: (0..points).map(|_| PointId::generate()).collect(),
			point_positions: (0..points).map(|index| DVec2::new(index as f64, 0.)).collect(),
			segment_ids: segments.iter().map(|_| SegmentId::generate()).collect(),
			segment_start: segments.iter().map(|&(start, ..)| start).collect(),
			segment_end: segments.iter().map(|&(_, end, _)| end).collect(),
			segment_handles: segments.iter().map(|(.., handles)| *handles).collect(),
			colinear: Vec::new(),
		}
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

	// Beveling appends its corner segments after the sides, which keep that order and still walk as one closed contour
	#[test]
	fn reweave_keeps_segments_appended_out_of_order_in_order() {
		let sides = [(0, 1), (2, 3), (4, 5), (6, 7)];
		let corners = [(1, 2), (3, 4), (5, 6), (7, 0)];
		let segments: Vec<_> = sides.into_iter().chain(corners).map(|(start, end)| (start, end, BezierHandles::Linear)).collect();

		let (vector, paths) = woven(working(8, &segments));

		assert_eq!(paths.len(), 8);
		assert_eq!(references(&paths, 3), vec![6, 7]);
		assert_eq!(references(&paths, 4), vec![1, 2]);

		let contours: Vec<_> = vector.stroke_bezpath_iter(&paths).collect();
		assert_eq!(contours.len(), 1);
		assert_eq!(contours[0].elements().len(), 10);
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
