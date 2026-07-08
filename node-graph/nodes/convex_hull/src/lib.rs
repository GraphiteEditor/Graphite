use core_types::Ctx;
use core_types::list::{ATTR_EDITOR_MERGED_LAYERS, ATTR_TRANSFORM, Item, List};
use glam::DAffine2;
use graphic_types::vector_types::subpath::Subpath;
use graphic_types::vector_types::vector::PointId;
use graphic_types::vector_types::vector::algorithms::merge_by_distance::MergeByDistanceExt;
use graphic_types::{Graphic, IntoGraphicList, Vector};
use vector_types::kurbo::{Affine, CubicBez, Line as KurboLine, ParamCurve, PathSeg as KurboPathSeg};

mod hull;
use hull::{HullSegment, MonotoneArc, convex_hull_loops, split_loops_into_arcs};

/// Check if a CubicBez is degenerate (all control points at essentially the same location).
fn is_degenerate_cubic(cb: &CubicBez) -> bool {
	const EPS_SQ: f64 = 1e-20;
	let d03 = cb.p3 - cb.p0;
	let d01 = cb.p1 - cb.p0;
	let d02 = cb.p2 - cb.p0;
	(d03.x * d03.x + d03.y * d03.y) < EPS_SQ && (d01.x * d01.x + d01.y * d01.y) < EPS_SQ && (d02.x * d02.x + d02.y * d02.y) < EPS_SQ
}

/// Convert a `Subpath<PointId>` into a closed loop of `CubicBez` segments in world space.
/// Open subpaths are closed with a line segment; degenerate segments are dropped.
fn subpath_to_loop(subpath: &Subpath<PointId>, transform: DAffine2) -> Vec<CubicBez> {
	let affine = Affine::new(transform.to_cols_array());
	subpath.iter_closed().map(|seg| (affine * seg).to_cubic()).filter(|cb| !is_degenerate_cubic(cb)).collect()
}

/// Convert hull segments back into a `Subpath<PointId>`.
fn hull_segments_to_subpath(segments: &[HullSegment], arcs: &[MonotoneArc]) -> Option<Subpath<PointId>> {
	let mut kurbo_segs: Vec<KurboPathSeg> = segments
		.iter()
		.map(|seg| match seg {
			HullSegment::Arc { arc_index, t_start, t_end } => {
				let sub = arcs[*arc_index].bezier.subsegment(*t_start..*t_end);
				KurboPathSeg::Cubic(sub)
			}
			HullSegment::Line { start, end, .. } => KurboPathSeg::Line(KurboLine::new(*start, *end)),
		})
		.collect();

	if kurbo_segs.is_empty() {
		return None;
	}

	// Subpath::from_beziers requires at least 2 segments for a closed path.
	// If we have only 1, split it at the midpoint.
	if kurbo_segs.len() == 1 {
		let cb = kurbo_segs[0].to_cubic();
		kurbo_segs = vec![KurboPathSeg::Cubic(cb.subsegment(0.0..0.5)), KurboPathSeg::Cubic(cb.subsegment(0.5..1.0))];
	}

	Some(Subpath::from_beziers(&kurbo_segs, true))
}

/// Exact convex hull of a set of closed loops, as a subpath.
fn compute_hull_subpath(loops: &[&[CubicBez]]) -> Option<Subpath<PointId>> {
	if loops.is_empty() {
		return None;
	}
	let arcs = split_loops_into_arcs(loops);
	let hull_segments = convex_hull_loops(loops);
	hull_segments_to_subpath(&hull_segments, &arcs)
}

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
async fn convex_hull<I: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Vector>)] content: I) -> List<Vector> {
	let content = content.into_graphic_list();
	let flattened: List<Vector> = content.clone().into_flattened_list();
	if flattened.is_empty() {
		return List::default();
	}

	// Collect every subpath of every element as a closed loop in world space.
	// The hull library handles multiple loops, occlusion, nesting, arbitrary
	// winding, and self-intersections directly, so no union or winding
	// normalization is needed.
	let mut loops: Vec<Vec<CubicBez>> = Vec::new();
	for index in 0..flattened.len() {
		let Some(element) = flattened.element(index) else { continue };
		let transform: DAffine2 = flattened.attribute_cloned_or_default(ATTR_TRANSFORM, index);
		for subpath in element.stroke_bezier_paths() {
			let segs = subpath_to_loop(&subpath, transform);
			if !segs.is_empty() {
				loops.push(segs);
			}
		}
	}

	let loop_refs: Vec<&[CubicBez]> = loops.iter().map(|l| l.as_slice()).collect();
	let Some(hull_subpath) = compute_hull_subpath(&loop_refs) else {
		// Degenerate input (e.g. only zero-length segments): pass through.
		return flattened;
	};

	// Carry the first input item's paint attributes onto the hull. The hull
	// geometry is already in world space, so the cloned transform attribute
	// must be reset to identity or it would be applied a second time.
	let paint_attributes = flattened.clone_item_attributes(0);
	let hull_vector = Vector::from_subpath(hull_subpath);
	let mut result = List::new_from_item(Item::from_parts(hull_vector, paint_attributes));
	result.set_attribute(ATTR_TRANSFORM, 0, DAffine2::IDENTITY);

	// Snapshot the input layers as `editor:merged_layers` so the renderer can
	// recurse into them and keep the original layers' overlays and click
	// targets in place (same pattern as Boolean Operation and Flatten Path).
	// No transform pre-compensation is needed since item 0's transform is
	// identity.
	result.set_attribute(ATTR_EDITOR_MERGED_LAYERS, 0, content);

	if let Some(element) = result.element_mut(0) {
		element.merge_by_distance_spatial(DAffine2::IDENTITY, 0.0001);
	}

	result
}

#[cfg(test)]
mod tests {
	use super::*;
	use vector_types::kurbo::Point as KurboPoint;

	fn circle_at(cx: f64, cy: f64, r: f64) -> Vec<CubicBez> {
		let k = 0.5522847498 * r;
		vec![
			CubicBez::new((cx + r, cy), (cx + r, cy + k), (cx + k, cy + r), (cx, cy + r)),
			CubicBez::new((cx, cy + r), (cx - k, cy + r), (cx - r, cy + k), (cx - r, cy)),
			CubicBez::new((cx - r, cy), (cx - r, cy - k), (cx - k, cy - r), (cx, cy - r)),
			CubicBez::new((cx, cy - r), (cx + k, cy - r), (cx + r, cy - k), (cx + r, cy)),
		]
	}

	fn assert_closed_and_contains(subpath: &Subpath<PointId>, points: &[(f64, f64)]) {
		assert!(subpath.closed());
		assert!(subpath.manipulator_groups().len() >= 2);
		// Sampled containment check against the hull polygon.
		let poly: Vec<KurboPoint> = subpath.iter_closed().flat_map(|seg| (0..32).map(move |k| seg.eval(k as f64 / 32.0))).collect();
		for &(px, py) in points {
			// Point-in-polygon via ray casting.
			let mut inside = false;
			for i in 0..poly.len() {
				let a = poly[i];
				let b = poly[(i + 1) % poly.len()];
				if (a.y > py) != (b.y > py) && px < a.x + (b.x - a.x) * (py - a.y) / (b.y - a.y) {
					inside = !inside;
				}
			}
			assert!(inside, "point ({px}, {py}) not inside hull");
		}
	}

	#[test]
	fn hull_of_two_disjoint_circles_spans_both() {
		let a = circle_at(0.0, 0.0, 1.0);
		let b = circle_at(5.0, 0.0, 1.0);
		let hull = compute_hull_subpath(&[&a, &b]).unwrap();
		// Previously (union + Graham bridging) two disjoint shapes fell through
		// the >= 3 anchor guard and the hull covered only one of them.
		assert_closed_and_contains(&hull, &[(0.0, 0.0), (5.0, 0.0), (2.5, 0.5)]);
	}

	#[test]
	fn hull_of_nested_circles_is_outer() {
		let outer = circle_at(0.0, 0.0, 2.0);
		let inner = circle_at(0.3, 0.1, 1.0);
		let hull = compute_hull_subpath(&[&outer, &inner]).unwrap();
		assert_closed_and_contains(&hull, &[(0.0, 0.0), (1.9, 0.0), (0.0, -1.9)]);
	}

	#[test]
	fn hull_handles_degenerate_line_parameterization() {
		// Rectangle with zero-derivative endpoints, as produced by real paths.
		let rect = vec![
			CubicBez::new((0.0, 0.0), (0.0, 0.0), (4.0, 0.0), (4.0, 0.0)),
			CubicBez::new((4.0, 0.0), (4.0, 0.0), (4.0, 2.0), (4.0, 2.0)),
			CubicBez::new((4.0, 2.0), (4.0, 2.0), (0.0, 2.0), (0.0, 2.0)),
			CubicBez::new((0.0, 2.0), (0.0, 2.0), (0.0, 0.0), (0.0, 0.0)),
		];
		let hull = compute_hull_subpath(&[&rect]).unwrap();
		assert_closed_and_contains(&hull, &[(2.0, 1.0), (0.1, 0.1), (3.9, 1.9)]);
	}
}
