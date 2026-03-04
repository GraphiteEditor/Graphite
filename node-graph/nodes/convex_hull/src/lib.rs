use core_types::Ctx;
use core_types::table::{Table, TableRow, TableRowRef};
use glam::{DAffine2, DVec2};
use graphic_types::Vector;
use graphic_types::vector_types::subpath::{ManipulatorGroup, PathSegPoints, Subpath, pathseg_points};
use graphic_types::vector_types::vector::PointId;
use graphic_types::vector_types::vector::algorithms::merge_by_distance::MergeByDistanceExt;
pub use path_bool as path_bool_lib;
use path_bool::{FillRule, PathBooleanOperation};
use std::ops::Mul;

use ::convex_hull::{HullSegment, MonotoneArc, convex_hull as compute_convex_hull, split_at_inflections};
use kurbo::{CubicBez, Line as KurboLine, ParamCurve, PathSeg as KurboPathSeg, Point as KurboPoint};

// ─── Graham's Scan Convex Hull ───

/// Compute the convex hull of a set of 2D points using Graham's scan.
/// Returns points in counter-clockwise order.
fn graham_scan_hull(points: &[DVec2]) -> Vec<DVec2> {
	if points.len() <= 2 {
		return points.to_vec();
	}

	// Find the lowest-y point (leftmost if tied)
	let mut pivot_idx = 0;
	for (i, p) in points.iter().enumerate() {
		if p.y < points[pivot_idx].y || (p.y == points[pivot_idx].y && p.x < points[pivot_idx].x) {
			pivot_idx = i;
		}
	}
	let pivot = points[pivot_idx];

	// Sort remaining points by polar angle from pivot
	let mut indexed: Vec<(usize, DVec2)> = points.iter().copied().enumerate().filter(|&(i, _)| i != pivot_idx).collect();
	indexed.sort_by(|&(_, a), &(_, b)| {
		let da = a - pivot;
		let db = b - pivot;
		let angle_a = da.y.atan2(da.x);
		let angle_b = db.y.atan2(db.x);
		angle_a.partial_cmp(&angle_b).unwrap().then_with(|| {
			// If same angle, closer point first
			da.length_squared().partial_cmp(&db.length_squared()).unwrap()
		})
	});

	// Build hull using cross-product left-turn test
	let mut hull = vec![pivot];
	for (_, p) in indexed {
		while hull.len() >= 2 {
			let a = hull[hull.len() - 2];
			let b = hull[hull.len() - 1];
			let cross = (b - a).perp_dot(p - b);
			if cross <= 0.0 {
				hull.pop();
			} else {
				break;
			}
		}
		hull.push(p);
	}

	hull
}

// ─── Kurbo PathSeg → CubicBez Conversion ───

/// Convert any `kurbo::PathSeg` to a `CubicBez`.
fn pathseg_to_cubicbez(seg: KurboPathSeg) -> CubicBez {
	match seg {
		KurboPathSeg::Cubic(cb) => cb,
		KurboPathSeg::Quad(qb) => {
			// Degree elevation: quadratic → cubic
			let p0 = qb.p0;
			let p3 = qb.p2;
			let q1 = qb.p1;
			let p1 = KurboPoint::new(p0.x + 2.0 / 3.0 * (q1.x - p0.x), p0.y + 2.0 / 3.0 * (q1.y - p0.y));
			let p2 = KurboPoint::new(p3.x + 2.0 / 3.0 * (q1.x - p3.x), p3.y + 2.0 / 3.0 * (q1.y - p3.y));
			CubicBez::new(p0, p1, p2, p3)
		}
		KurboPathSeg::Line(l) => {
			// Place control points at 1/3 and 2/3 along the line
			let p0 = l.p0;
			let p3 = l.p1;
			let p1 = KurboPoint::new(p0.x + (p3.x - p0.x) / 3.0, p0.y + (p3.y - p0.y) / 3.0);
			let p2 = KurboPoint::new(p0.x + 2.0 * (p3.x - p0.x) / 3.0, p0.y + 2.0 * (p3.y - p0.y) / 3.0);
			CubicBez::new(p0, p1, p2, p3)
		}
	}
}

// ─── Subpath → Vec<CubicBez> Conversion ───

/// Check if a CubicBez is degenerate (all control points at essentially the same location).
fn is_degenerate_cubic(cb: &CubicBez) -> bool {
	const EPS_SQ: f64 = 1e-20;
	let d03 = cb.p3 - cb.p0;
	let d01 = cb.p1 - cb.p0;
	let d02 = cb.p2 - cb.p0;
	(d03.x * d03.x + d03.y * d03.y) < EPS_SQ && (d01.x * d01.x + d01.y * d01.y) < EPS_SQ && (d02.x * d02.x + d02.y * d02.y) < EPS_SQ
}

/// Convert a `Subpath<PointId>` into a `Vec<CubicBez>` for the convex hull library.
/// Filters out degenerate zero-length segments.
fn subpath_to_cubicbez_vec(subpath: &Subpath<PointId>) -> Vec<CubicBez> {
	subpath.iter().map(pathseg_to_cubicbez).filter(|cb| !is_degenerate_cubic(cb)).collect()
}

// ─── Winding Direction ───

/// Compute the signed area of a closed cubic bezier path by sampling.
/// Positive = CCW in standard math coords, Negative = CW.
fn signed_area_of_cubic_path(segments: &[CubicBez]) -> f64 {
	let mut area = 0.0;
	let n = 16;
	for seg in segments {
		for i in 0..n {
			let t0 = i as f64 / n as f64;
			let t1 = (i + 1) as f64 / n as f64;
			let p0 = seg.eval(t0);
			let p1 = seg.eval(t1);
			area += p0.x * p1.y - p1.x * p0.y;
		}
	}
	area / 2.0
}

/// Reverse a cubic bezier path (reverse segment order + swap endpoints within each segment).
fn reverse_cubic_path(segments: &[CubicBez]) -> Vec<CubicBez> {
	segments.iter().rev().map(|cb| CubicBez::new(cb.p3, cb.p2, cb.p1, cb.p0)).collect()
}

// ─── Select Outer Subpath ───

/// Select the outermost subpath from a Vector by choosing the one with the largest absolute area.
fn select_outer_subpath(vector: &Vector) -> Option<Subpath<PointId>> {
	vector.stroke_bezier_paths().max_by(|a, b| {
		let area_a = a.area_centroid_and_area(None, None).map(|(_, area)| area.abs()).unwrap_or(0.0);
		let area_b = b.area_centroid_and_area(None, None).map(|(_, area)| area.abs()).unwrap_or(0.0);
		area_a.partial_cmp(&area_b).unwrap_or(std::cmp::Ordering::Equal)
	})
}

// ─── Hull Segments → Subpath ───

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
		let seg = kurbo_segs[0];
		match seg {
			KurboPathSeg::Cubic(cb) => {
				let first_half = cb.subsegment(0.0..0.5);
				let second_half = cb.subsegment(0.5..1.0);
				kurbo_segs = vec![KurboPathSeg::Cubic(first_half), KurboPathSeg::Cubic(second_half)];
			}
			KurboPathSeg::Line(l) => {
				let mid = KurboPoint::new((l.p0.x + l.p1.x) / 2.0, (l.p0.y + l.p1.y) / 2.0);
				kurbo_segs = vec![KurboPathSeg::Line(KurboLine::new(l.p0, mid)), KurboPathSeg::Line(KurboLine::new(mid, l.p1))];
			}
			KurboPathSeg::Quad(qb) => {
				let cb = pathseg_to_cubicbez(KurboPathSeg::Quad(qb));
				let first_half = cb.subsegment(0.0..0.5);
				let second_half = cb.subsegment(0.5..1.0);
				kurbo_segs = vec![KurboPathSeg::Cubic(first_half), KurboPathSeg::Cubic(second_half)];
			}
		}
	}

	Some(Subpath::from_beziers(&kurbo_segs, true))
}

// ─── Main Node ───

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
async fn convex_hull(_: impl Ctx, content: Table<Vector>) -> Table<Vector> {
	// Handle empty input
	if content.is_empty() {
		return Table::default();
	}

	// Step 1: Collect one representative point per subpath (in world space)
	let mut hull_points: Vec<DVec2> = Vec::new();
	for row in content.iter() {
		let transform = *row.transform;
		for subpath in row.element.stroke_bezier_paths() {
			if let Some(first) = subpath.manipulator_groups().first() {
				hull_points.push(transform.transform_point2(first.anchor));
			}
		}
	}

	// Step 2: Union all input shapes
	let mut result_vector_table = union(content.iter());

	// Step 3: Flatten union result to world space (apply transform, set to IDENTITY)
	let style;
	{
		let Some(result_row) = result_vector_table.iter_mut().next() else {
			return Table::default();
		};
		let transform = *result_row.transform;
		*result_row.transform = DAffine2::IDENTITY;
		Vector::transform(result_row.element, transform);
		result_row.element.style.set_stroke_transform(DAffine2::IDENTITY);

		// Step 4: Save style
		style = result_row.element.style.clone();
	}

	// Step 5: If the union has multiple disjoint subpaths AND we have ≥3 hull points,
	// build a polyline convex hull and boolean-union it with the result to connect everything.
	let subpath_count = result_vector_table.iter().next().map(|r| r.element.stroke_bezier_paths().count()).unwrap_or(0);
	log::debug!("subpath_count: {}", subpath_count);

	if subpath_count > 1 && hull_points.len() >= 3 {
		let poly_points = graham_scan_hull(&hull_points);
		if poly_points.len() >= 3 {
			// Build a polyline subpath from the hull points
			let poly_subpath = Subpath::<PointId>::from_anchors(poly_points.into_iter(), true);
			let poly_vector = Vector::from_subpath(poly_subpath);

			// Boolean union the current result with the polyline
			let current_vector = &result_vector_table.iter().next().unwrap().element;
			let upper_path = to_path(current_vector, DAffine2::IDENTITY);
			let lower_path = to_path(&poly_vector, DAffine2::IDENTITY);

			#[allow(unused_unsafe)]
			let union_result_paths = unsafe { boolean_union(upper_path, lower_path) };
			let union_result = from_path(&union_result_paths);

			// Replace the result vector's geometry
			let result_row = result_vector_table.iter_mut().next().unwrap();
			result_row.element.colinear_manipulators = union_result.colinear_manipulators;
			result_row.element.point_domain = union_result.point_domain;
			result_row.element.segment_domain = union_result.segment_domain;
			result_row.element.region_domain = union_result.region_domain;
		}
	}

	// Step 6: Select the outer boundary subpath (largest by area)
	let outer_subpath = {
		let result_row = result_vector_table.iter().next().unwrap();
		select_outer_subpath(result_row.element)
	};

	let Some(outer_subpath) = outer_subpath else {
		return result_vector_table;
	};

	// Step 7: Convert to Vec<CubicBez>
	let cubic_segments = subpath_to_cubicbez_vec(&outer_subpath);
	if cubic_segments.is_empty() {
		return result_vector_table;
	}

	// The hull library expects CCW winding. Graphite paths are typically CW in screen coords
	// (Y-down), so we reverse if the signed area is negative (CW in math coords).
	let cubic_segments = if signed_area_of_cubic_path(&cubic_segments) < 0.0 {
		reverse_cubic_path(&cubic_segments)
	} else {
		cubic_segments
	};

	log::debug!("path: {:?}", cubic_segments);

	// Step 8: Run the curved convex hull algorithm
	let arcs = split_at_inflections(&cubic_segments);
	log::debug!("arcs: {:?}", arcs);
	let hull_segments = compute_convex_hull(&cubic_segments);
	log::debug!("segments: {:?}", hull_segments);

	if hull_segments.is_empty() {
		// Fallback: return the union result as-is
		return result_vector_table;
	}

	// Step 9: Reconstruct hull as Subpath
	let Some(hull_subpath) = hull_segments_to_subpath(&hull_segments, &arcs) else {
		return result_vector_table;
	};
	log::debug!("hull_subpath: {:?}", hull_subpath);

	// Step 10: Create Vector from hull subpath, apply saved style
	let mut hull_vector = Vector::from_subpath(hull_subpath);
	hull_vector.style = style;

	// Step 11: Build result table
	let mut result: Table<Vector> = Table::new_from_element(hull_vector);
	if let Some(row) = result.iter_mut().next() {
		// Step 11: Clean up with merge_by_distance_spatial
		row.element.merge_by_distance_spatial(*row.transform, 0.0001);
	}

	result
}

// ─── Boolean Operations (shared helpers) ───

fn union<'a>(vector: impl DoubleEndedIterator<Item = TableRowRef<'a, Vector>>) -> Table<Vector> {
	// Reverse the vector table rows so that the result style is the style of the first vector row
	let mut vector_reversed = vector.rev();

	let mut result_vector_table = Table::new_from_row(vector_reversed.next().map(|x| x.into_cloned()).unwrap_or_default());
	let mut first_row = result_vector_table.iter_mut().next().expect("Expected the one row we just pushed");

	// Loop over all vector table rows and union it with the result
	let default = TableRow::default();
	let mut second_vector = Some(vector_reversed.next().unwrap_or(default.as_ref()));
	while let Some(lower_vector) = second_vector {
		let transform_of_lower_into_space_of_upper = first_row.transform.inverse() * *lower_vector.transform;

		let result = &mut first_row.element;

		let upper_path_string = to_path(result, DAffine2::IDENTITY);
		let lower_path_string = to_path(lower_vector.element, transform_of_lower_into_space_of_upper);

		#[allow(unused_unsafe)]
		let boolean_operation_string = unsafe { boolean_union(upper_path_string, lower_path_string) };
		let boolean_operation_result = from_path(&boolean_operation_string);

		result.colinear_manipulators = boolean_operation_result.colinear_manipulators;
		result.point_domain = boolean_operation_result.point_domain;
		result.segment_domain = boolean_operation_result.segment_domain;
		result.region_domain = boolean_operation_result.region_domain;

		second_vector = vector_reversed.next();
	}

	result_vector_table
}

fn to_path(vector: &Vector, transform: DAffine2) -> Vec<path_bool::PathSegment> {
	let mut path = Vec::new();
	for subpath in vector.stroke_bezier_paths() {
		to_path_segments(&mut path, &subpath, transform);
	}
	path
}

fn to_path_segments(path: &mut Vec<path_bool::PathSegment>, subpath: &Subpath<PointId>, transform: DAffine2) {
	use path_bool::PathSegment;
	let mut global_start = None;
	let mut global_end = DVec2::ZERO;

	for bezier in subpath.iter() {
		const EPS: f64 = 1e-8;
		let transform_point = |pos: DVec2| transform.transform_point2(pos).mul(EPS.recip()).round().mul(EPS);

		let PathSegPoints { p0, p1, p2, p3 } = pathseg_points(bezier);

		let p0 = transform_point(p0);
		let p1 = p1.map(transform_point);
		let p2 = p2.map(transform_point);
		let p3 = transform_point(p3);

		if global_start.is_none() {
			global_start = Some(p0);
		}
		global_end = p3;

		let segment = match (p1, p2) {
			(None, None) => PathSegment::Line(p0, p3),
			(None, Some(p2)) | (Some(p2), None) => PathSegment::Quadratic(p0, p2, p3),
			(Some(p1), Some(p2)) => PathSegment::Cubic(p0, p1, p2, p3),
		};

		path.push(segment);
	}
	if let Some(start) = global_start {
		path.push(PathSegment::Line(global_end, start));
	}
}

fn from_path(path_data: &[Path]) -> Vector {
	const EPSILON: f64 = 1e-5;

	fn is_close(a: DVec2, b: DVec2) -> bool {
		(a - b).length_squared() < EPSILON * EPSILON
	}

	let mut all_subpaths = Vec::new();

	for path in path_data.iter().filter(|path| !path.is_empty()) {
		let cubics: Vec<[DVec2; 4]> = path.iter().map(|segment| segment.to_cubic()).collect();
		let mut manipulators_list = Vec::new();
		let mut current_start = None;

		for (index, cubic) in cubics.iter().enumerate() {
			let [start, handle1, handle2, end] = *cubic;

			if current_start.is_none() || !is_close(start, current_start.unwrap()) {
				// Start a new subpath
				if !manipulators_list.is_empty() {
					all_subpaths.push(Subpath::new(std::mem::take(&mut manipulators_list), true));
				}
				// Use the correct in-handle (None) and out-handle for the start point
				manipulators_list.push(ManipulatorGroup::new(start, None, Some(handle1)));
			} else {
				// Update the out-handle of the previous point
				if let Some(last) = manipulators_list.last_mut() {
					last.out_handle = Some(handle1);
				}
			}

			// Add the end point with the correct in-handle and out-handle (None)
			manipulators_list.push(ManipulatorGroup::new(end, Some(handle2), None));

			current_start = Some(end);

			// Check if this is the last segment
			if index == cubics.len() - 1 {
				all_subpaths.push(Subpath::new(manipulators_list, true));
				manipulators_list = Vec::new(); // Reset manipulators for the next path
			}
		}
	}

	Vector::from_subpaths(all_subpaths, false)
}

type Path = Vec<path_bool::PathSegment>;

fn boolean_union(a: Path, b: Path) -> Vec<Path> {
	path_bool(a, b, PathBooleanOperation::Union)
}

fn path_bool(a: Path, b: Path, op: PathBooleanOperation) -> Vec<Path> {
	match path_bool::path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, op) {
		Ok(results) => results,
		Err(e) => {
			let a_path = path_bool::path_to_path_data(&a, 0.001);
			let b_path = path_bool::path_to_path_data(&b, 0.001);
			log::error!("Boolean error {e:?} encountered while processing {a_path}\n {op:?}\n {b_path}");
			Vec::new()
		}
	}
}

pub fn boolean_intersect(a: Path, b: Path) -> Vec<Path> {
	path_bool(a, b, PathBooleanOperation::Intersection)
}
