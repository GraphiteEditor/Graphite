//! Geometry for the Voronoi and Delaunay nodes.
//!
//! Both diagrams are derived from a single Delaunay triangulation (computed by the `delaunator` crate). The Voronoi
//! diagram is the geometric dual of that triangulation: each Voronoi vertex is the circumcenter of a Delaunay triangle,
//! and each Voronoi edge connects the circumcenters of two triangles that share a Delaunay edge.
//!
//! Each function here reduces a diagram to a set of closed polygons (one per Delaunay triangle or per Voronoi cell). The
//! nodes then assemble those polygons into vector geometry, either as separate filled subpaths or as a shared mesh of
//! welded points and segments. Voronoi cells around the convex hull are unbounded, so they are clipped to the convex hull
//! of the input sites, which also bounds the whole diagram to a finite region.

use delaunator::{EMPTY, Point, triangulate};
use glam::DVec2;

/// Computes the Delaunay triangulation of `sites`, returning each triangle as a triple of indices into `sites`.
///
/// Returns an empty vector when there are fewer than three points or they are all colinear (no triangle exists).
pub fn delaunay_triangles(sites: &[DVec2]) -> Vec<[usize; 3]> {
	let points: Vec<Point> = sites.iter().map(|p| Point { x: p.x, y: p.y }).collect();
	let triangulation = triangulate(&points);
	triangulation.triangles.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect()
}

/// Computes the Voronoi cell of every site, each clipped to the convex hull of `sites`.
///
/// Returns one closed polygon per site that produces a non-empty cell (degenerate or fully-clipped cells are omitted,
/// so the result may be shorter than `sites`). Returns an empty vector when no triangulation exists (fewer than three points or all colinear).
pub fn voronoi_cells(sites: &[DVec2]) -> Vec<Vec<DVec2>> {
	voronoi_cells_per_site(sites).0.into_iter().flatten().collect()
}

/// Applies Lloyd's relaxation: each step moves every interior site to the centroid of its Voronoi cell, yielding a more
/// even (centroidal) point distribution. A fractional `iterations` runs the whole-number steps and then blends each site
/// partway toward the result of one more step, so the relaxation can be animated smoothly. Returns the sites unchanged when
/// `iterations` is 0 or no diagram can be formed.
///
/// The convex-hull (perimeter) sites are pinned so the point cloud's outline is preserved. Otherwise, clipping the
/// unbounded perimeter cells would drag those sites around (inward for the convex hull, or outward into the corners of a
/// fixed bounding box), distorting the shape over successive iterations.
pub fn relax_sites(sites: &[DVec2], iterations: f64) -> Vec<DVec2> {
	const MAX_STEPS_FOR_SAFETY: f64 = 1000.;
	let iterations = iterations.clamp(0., MAX_STEPS_FOR_SAFETY);
	let whole_steps = iterations.floor();
	let fraction = iterations - whole_steps;

	let mut current = sites.to_vec();
	for _ in 0..whole_steps as u32 {
		current = relax_once(&current);
	}

	// Blend each site partway toward one further step for the fractional remainder.
	if fraction > 0. {
		let next = relax_once(&current);
		for (point, target) in current.iter_mut().zip(next) {
			*point = point.lerp(target, fraction);
		}
	}

	current
}

/// Performs a single Lloyd relaxation step: moves every interior site to its Voronoi cell centroid,
/// leaving the pinned convex-hull (perimeter) sites in place.
fn relax_once(sites: &[DVec2]) -> Vec<DVec2> {
	let (cells, is_hull) = voronoi_cells_per_site(sites);
	let mut relaxed = sites.to_vec();
	for ((site, cell), on_hull) in relaxed.iter_mut().zip(cells).zip(is_hull) {
		if on_hull {
			continue;
		}
		if let Some(centroid) = cell.as_deref().and_then(polygon_centroid) {
			*site = centroid;
		}
	}
	relaxed
}

/// The area-weighted centroid of a simple polygon, or `None` if it has fewer than three vertices or zero area.
fn polygon_centroid(polygon: &[DVec2]) -> Option<DVec2> {
	if polygon.len() < 3 {
		return None;
	}
	let mut double_area = 0.;
	let mut weighted = DVec2::ZERO;
	for i in 0..polygon.len() {
		let a = polygon[i];
		let b = polygon[(i + 1) % polygon.len()];
		let cross = a.perp_dot(b);
		double_area += cross;
		weighted += (a + b) * cross;
	}
	(double_area.abs() >= f64::EPSILON).then(|| weighted / (3. * double_area))
}

/// Computes each site's clipped Voronoi cell, aligned with `sites` (index `i` is the cell of `sites[i]`), together with a
/// per-site flag marking the convex-hull (perimeter) sites. A cell is `None` when the site has no incident triangle
/// (e.g. a coincident duplicate) or its cell vanishes after clipping.
fn voronoi_cells_per_site(sites: &[DVec2]) -> (Vec<Option<Vec<DVec2>>>, Vec<bool>) {
	let points: Vec<Point> = sites.iter().map(|p| Point { x: p.x, y: p.y }).collect();
	let triangulation = triangulate(&points);
	if triangulation.triangles.is_empty() {
		return (vec![None; sites.len()], vec![false; sites.len()]);
	}

	let triangles = &triangulation.triangles;
	let halfedges = &triangulation.halfedges;
	let hull_indices = &triangulation.hull;

	// Mark which sites lie on the convex hull (the diagram's perimeter).
	let mut is_hull = vec![false; sites.len()];
	for &index in hull_indices {
		is_hull[index] = true;
	}

	// One Voronoi vertex per Delaunay triangle.
	let circumcenters: Vec<DVec2> = triangles.chunks_exact(3).map(|t| circumcenter(sites[t[0]], sites[t[1]], sites[t[2]])).collect();

	// The convex hull polygon, which clips the diagram to a finite region.
	let hull: Vec<DVec2> = hull_indices.iter().map(|&i| sites[i]).collect();

	// `inedges[p]` is a half-edge ending at site `p`, preferring a hull half-edge so a hull cell's walk starts on the boundary.
	let mut inedges = vec![EMPTY; sites.len()];
	for edge in 0..triangles.len() {
		let endpoint = triangles[next_halfedge(edge)];
		if halfedges[edge] == EMPTY || inedges[endpoint] == EMPTY {
			inedges[endpoint] = edge;
		}
	}

	// Outward ray directions for the two hull edges meeting at each hull site, used to project its unbounded cell outward.
	// Both are zero for interior sites.
	let mut ray_in = vec![DVec2::ZERO; sites.len()];
	let mut ray_out = vec![DVec2::ZERO; sites.len()];
	if let Some(&last) = hull_indices.last() {
		let mut previous = last;
		for &current in hull_indices {
			let p0 = sites[previous];
			let p1 = sites[current];
			// Perpendicular to the hull edge `previous -> current`, pointing away from the hull interior.
			let perpendicular = DVec2::new(p0.y - p1.y, p1.x - p0.x);
			ray_out[previous] = perpendicular;
			ray_in[current] = perpendicular;
			previous = current;
		}
	}

	// Length to extend unbounded cell rays so they reach past the hull before clipping trims them back to it.
	let far = bounding_diagonal(&hull) * 10. + 1.;

	let cells = (0..sites.len())
		.map(|site| {
			let mut polygon = cell_polygon(site, halfedges, &circumcenters, &inedges)?;

			// A hull site's cell is unbounded; cap its open ends with far points along the outward hull-edge normals so the
			// convex-hull clip below closes it off at the boundary.
			let unbounded = ray_in[site] != DVec2::ZERO || ray_out[site] != DVec2::ZERO;
			if unbounded {
				if let Some(&first) = polygon.first() {
					polygon.insert(0, first + ray_in[site].normalize_or_zero() * far);
				}
				if let Some(&last) = polygon.last() {
					polygon.push(last + ray_out[site].normalize_or_zero() * far);
				}
			}

			let clipped = clip_to_convex(&polygon, &hull);
			(clipped.len() >= 3).then_some(clipped)
		})
		.collect();

	(cells, is_hull)
}

/// The circumcenter of a triangle, computed relative to `a` for numerical stability. Falls back to the centroid for a
/// degenerate (colinear) triangle.
fn circumcenter(a: DVec2, b: DVec2, c: DVec2) -> DVec2 {
	let d = b - a;
	let e = c - a;
	let determinant = d.x * e.y - d.y * e.x;
	if determinant.abs() < f64::EPSILON {
		return (a + b + c) / 3.;
	}
	let factor = 0.5 / determinant;
	let bl = d.length_squared();
	let cl = e.length_squared();
	DVec2::new(a.x + (e.y * bl - d.y * cl) * factor, a.y + (d.x * cl - e.x * bl) * factor)
}

/// Walks the Delaunay triangles incident to `site` and collects their circumcenters in order, forming the site's
/// Voronoi cell polygon. The polygon is closed for interior sites and open (a fan ending at the hull) for hull sites.
/// Returns `None` for a site with no incident triangle (e.g. a coincident duplicate point).
fn cell_polygon(site: usize, halfedges: &[usize], circumcenters: &[DVec2], inedges: &[usize]) -> Option<Vec<DVec2>> {
	let start = inedges[site];
	if start == EMPTY {
		return None;
	}

	let mut polygon = Vec::new();
	let mut edge = start;
	loop {
		polygon.push(circumcenters[edge / 3]);
		edge = halfedges[next_halfedge(edge)];
		if edge == EMPTY || edge == start {
			break;
		}
	}

	Some(polygon)
}

/// The next half-edge within the same triangle (triangles store three consecutive half-edges).
fn next_halfedge(edge: usize) -> usize {
	if edge % 3 == 2 { edge - 2 } else { edge + 1 }
}

/// Clips `subject` to the convex polygon `clip` using the Sutherland–Hodgman algorithm. The clip polygon may wind either way.
/// (The subject doesn't need to be convex.) Returns the clipped polygon (empty if it lies entirely outside the clip region).
fn clip_to_convex(subject: &[DVec2], clip: &[DVec2]) -> Vec<DVec2> {
	if clip.len() < 3 {
		return Vec::new();
	}

	// Normalize the clip polygon to counter-clockwise so "inside" is consistently to the left of each directed edge.
	let mut clip = clip.to_vec();
	if signed_area(&clip) < 0. {
		clip.reverse();
	}

	let mut output = subject.to_vec();
	for i in 0..clip.len() {
		if output.is_empty() {
			break;
		}

		let edge_start = clip[i];
		let edge_end = clip[(i + 1) % clip.len()];
		let edge = edge_end - edge_start;
		let inside = |p: DVec2| edge.x * (p.y - edge_start.y) - edge.y * (p.x - edge_start.x) >= 0.;

		let input = std::mem::take(&mut output);
		for j in 0..input.len() {
			let current = input[j];
			let previous = input[(j + input.len() - 1) % input.len()];
			let current_inside = inside(current);
			let previous_inside = inside(previous);

			if current_inside {
				if !previous_inside && let Some(crossing) = line_intersection(previous, current, edge_start, edge_end) {
					output.push(crossing);
				}
				output.push(current);
			} else if previous_inside && let Some(crossing) = line_intersection(previous, current, edge_start, edge_end) {
				output.push(crossing);
			}
		}
	}

	output
}

/// The signed area of a polygon (positive for counter-clockwise winding).
fn signed_area(polygon: &[DVec2]) -> f64 {
	let mut area = 0.;
	for i in 0..polygon.len() {
		let a = polygon[i];
		let b = polygon[(i + 1) % polygon.len()];
		area += a.x * b.y - b.x * a.y;
	}
	area / 2.
}

/// The intersection point of the segment `p1 -> p2` with the infinite line through `a` and `b`, or `None` if parallel.
fn line_intersection(p1: DVec2, p2: DVec2, a: DVec2, b: DVec2) -> Option<DVec2> {
	let r = p2 - p1;
	let s = b - a;
	let denominator = r.x * s.y - r.y * s.x;
	if denominator.abs() < f64::EPSILON {
		return None;
	}
	let t = ((a.x - p1.x) * s.y - (a.y - p1.y) * s.x) / denominator;
	Some(p1 + r * t)
}

/// The diagonal length of the axis-aligned bounding box of `points`.
fn bounding_diagonal(points: &[DVec2]) -> f64 {
	let mut min = DVec2::splat(f64::MAX);
	let mut max = DVec2::splat(f64::MIN);
	for &p in points {
		min = min.min(p);
		max = max.max(p);
	}
	let diagonal = (max - min).length();
	if diagonal.is_finite() && diagonal > 0. { diagonal } else { 0. }
}

#[cfg(test)]
mod tests {
	use super::*;

	fn square_with_center() -> Vec<DVec2> {
		vec![DVec2::new(0., 0.), DVec2::new(10., 0.), DVec2::new(10., 10.), DVec2::new(0., 10.), DVec2::new(5., 5.)]
	}

	#[test]
	fn delaunay_triangles_wind_counter_clockwise() {
		// `delaunator` returns clockwise triangles, so `delaunay_triangles` keeps that order, but `voronoi_cells` are
		// counter-clockwise. This documents the raw orientation; the Delaunay node reverses it to match the cells.
		let sites = square_with_center();
		for t in delaunay_triangles(&sites) {
			let poly = [sites[t[0]], sites[t[1]], sites[t[2]]];
			assert!(signed_area(&poly) < 0., "delaunator triangles are expected to be clockwise");
		}
		for cell in voronoi_cells(&sites) {
			assert!(signed_area(&cell) > 0., "voronoi cells are expected to be counter-clockwise");
		}
	}

	#[test]
	fn delaunay_triangulates_square() {
		let triangles = delaunay_triangles(&square_with_center());
		// Four corner-to-center triangles tessellate the square.
		assert_eq!(triangles.len(), 4);
		for triangle in triangles {
			for index in triangle {
				assert!(index < 5);
			}
		}
	}

	#[test]
	fn delaunay_degenerate_inputs_produce_no_triangles() {
		assert!(delaunay_triangles(&[]).is_empty());
		assert!(delaunay_triangles(&[DVec2::new(1., 1.)]).is_empty());
		assert!(delaunay_triangles(&[DVec2::new(0., 0.), DVec2::new(1., 1.)]).is_empty());
		// Colinear points have no triangulation.
		let colinear = vec![DVec2::new(0., 0.), DVec2::new(1., 1.), DVec2::new(2., 2.)];
		assert!(delaunay_triangles(&colinear).is_empty());
	}

	#[test]
	fn voronoi_cells_tile_the_hull() {
		// The clipped cells partition the convex hull, so their (counter-clockwise, positive) areas sum to the hull's area
		// (100 for the 10x10 square). If the outward projection direction were inverted, the boundary cells would collapse
		// inward and the total would fall well short of 100.
		let sites = square_with_center();
		let total: f64 = voronoi_cells(&sites).iter().map(|cell| signed_area(cell)).sum();
		assert!((total - 100.).abs() < 1e-6, "cells should tile the hull (area 100), got {total}");
	}

	#[test]
	fn voronoi_cells_stay_within_the_hull() {
		let sites = square_with_center();
		let cells = voronoi_cells(&sites);
		assert!(!cells.is_empty());
		// Clipping to the hull keeps every vertex inside the input bounds (with a small tolerance for float error).
		for cell in &cells {
			assert!(cell.len() >= 3);
			for &vertex in cell {
				assert!(vertex.x >= -1e-6 && vertex.x <= 10. + 1e-6, "x out of bounds: {}", vertex.x);
				assert!(vertex.y >= -1e-6 && vertex.y <= 10. + 1e-6, "y out of bounds: {}", vertex.y);
			}
		}
	}

	#[test]
	fn relaxation_with_zero_iterations_is_identity() {
		let sites = square_with_center();
		assert_eq!(relax_sites(&sites, 0.), sites);
	}

	#[test]
	fn relaxation_moves_points_and_keeps_them_in_the_hull() {
		// Add a point clustered near the center; relaxation should redistribute the points without leaving the hull.
		let mut sites = square_with_center();
		sites.push(DVec2::new(5.5, 4.5));
		let relaxed = relax_sites(&sites, 3.);

		assert_eq!(relaxed.len(), sites.len());
		assert_ne!(relaxed, sites, "relaxation should move the points");
		for &point in &relaxed {
			assert!(point.x >= -1e-6 && point.x <= 10. + 1e-6, "x out of bounds: {}", point.x);
			assert!(point.y >= -1e-6 && point.y <= 10. + 1e-6, "y out of bounds: {}", point.y);
		}
	}

	#[test]
	fn relaxation_pins_the_convex_hull() {
		// The four corners form the convex hull and must stay fixed; the interior points must relax.
		let sites = vec![
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(3., 3.),
			DVec2::new(7., 4.),
		];
		let relaxed = relax_sites(&sites, 4.);

		for i in 0..4 {
			assert_eq!(relaxed[i], sites[i], "convex hull point {i} should be pinned");
		}
		assert!(relaxed[4] != sites[4] || relaxed[5] != sites[5], "interior points should relax");
	}

	#[test]
	fn relaxation_interpolates_fractional_iterations() {
		let sites = vec![
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(3., 4.),
			DVec2::new(7., 6.),
		];

		// A fractional count lands exactly midway between the two bracketing whole-step results, exercising both the
		// pure-fraction path (0.5) and the whole-steps-then-fraction path (2.5).
		for whole in [0., 2.] {
			let lower = relax_sites(&sites, whole);
			let upper = relax_sites(&sites, whole + 1.);
			let half = relax_sites(&sites, whole + 0.5);
			for i in 0..sites.len() {
				let expected = (lower[i] + upper[i]) / 2.;
				assert!((half[i] - expected).length() < 1e-9, "index {i} at {whole}.5: {half:?} vs {expected:?}", half = half[i]);
			}
		}
	}

	#[test]
	fn relaxation_leaves_degenerate_input_unchanged() {
		// Fewer than three points cannot form a diagram, so relaxation is a no-op.
		let sites = vec![DVec2::new(0., 0.), DVec2::new(1., 1.)];
		assert_eq!(relax_sites(&sites, 5.), sites);
	}

	#[test]
	fn relaxation_clamps_extreme_iteration_counts() {
		let sites = vec![
			DVec2::new(0., 0.),
			DVec2::new(10., 0.),
			DVec2::new(10., 10.),
			DVec2::new(0., 10.),
			DVec2::new(3., 4.),
			DVec2::new(7., 6.),
		];
		// A huge or infinite count must clamp to the converged result rather than hang on a billions-long loop.
		let converged = relax_sites(&sites, 1000.);
		assert_eq!(relax_sites(&sites, 1e9), converged);
		assert_eq!(relax_sites(&sites, f64::INFINITY), converged);
		// NaN and negative counts resolve to zero steps, leaving the sites unchanged.
		assert_eq!(relax_sites(&sites, f64::NAN), sites);
		assert_eq!(relax_sites(&sites, -5.), sites);
	}

	#[test]
	fn voronoi_center_cell_is_bounded() {
		// A ring of points around a center yields a finite cell for the center site.
		let mut sites = vec![DVec2::new(0., 0.)];
		for i in 0..6 {
			let angle = i as f64 / 6. * std::f64::consts::TAU;
			sites.push(DVec2::new(angle.cos() * 10., angle.sin() * 10.));
		}
		let cells = voronoi_cells(&sites);
		assert!(!cells.is_empty());
		// Every cell is a finite polygon with no runaway coordinates.
		for cell in &cells {
			for &vertex in cell {
				assert!(vertex.length() < 100., "unbounded cell vertex: {vertex:?}");
			}
		}
	}
}
