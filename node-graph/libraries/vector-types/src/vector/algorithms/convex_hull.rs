//! Convex hull of Bezier path geometry.
//!
//! Unlike the classic convex hull of a point cloud or polygon, the hull of curved geometry keeps the convex
//! portions of the input curves and bridges between them with straight tangent lines, like a rubber band
//! stretched around the shapes. The result is built from three kinds of boundary pieces:
//! - Portions of input segments, cut exactly where the boundary departs from the curve
//! - Straight bridge lines connecting those portions, tangent to the curves they leave and enter
//! - Corner points (anchor points or free-floating points) that the rubber band bends around
//!
//! The algorithm proceeds in four stages:
//! 1. Normalize: split every curve at its inflections and cusps so each piece turns in only one direction,
//!    and reduce straight or degenerate segments to their extreme points.
//! 2. Discover structure: densely sample all pieces, take the polygonal hull of the samples, and read off
//!    which curve ranges and which corner points form the boundary and in what cyclic order.
//! 3. Refine: polish each transition between boundary pieces to an exact tangency using closed-form
//!    tangent-through-point solves (a quartic), so bridge lines touch the curves at true tangent points.
//! 4. Emit: cut the boundary ranges out of the original input segments (preserving their exact geometry
//!    and segment kind) and join them with the bridge lines into a single closed path.

use kurbo::common::solve_quadratic;
use kurbo::{BezPath, CubicBez, ParamCurve, ParamCurveDeriv, PathEl, PathSeg, Point, Vec2};

/// Parameter-space epsilon below which a curve span is considered empty.
const PARAM_EPSILON: f64 = 1e-9;
/// Iteration cap for the alternating tangency refinement between two curves.
const MAX_BITANGENT_ITERATIONS: usize = 32;
/// Parameter-space convergence tolerance for the tangency refinement.
const TANGENCY_TOLERANCE: f64 = 1e-13;

/// One curvature-monotone piece of an input segment, used as a candidate curve for the hull boundary.
struct ConvexArc {
	/// Cubic representation of this piece, used for all internal math (exact degree elevation for quadratic sources).
	cubic: CubicBez,
	/// Index into the input segment list identifying the segment this arc is a piece of.
	source: usize,
	/// Parameter range of the source segment covered by this piece.
	source_t0: f64,
	source_t1: f64,
	/// Number of sample intervals this arc contributes to hull structure discovery.
	sample_count: usize,
}

impl ConvexArc {
	/// Map a local parameter on this arc to a parameter on its source segment.
	fn to_source_t(&self, t: f64) -> f64 {
		self.source_t0 + t * (self.source_t1 - self.source_t0)
	}
}

/// Where a hull candidate sample came from.
#[derive(Clone, Copy)]
enum SampleTag {
	/// A sample on an arc at local parameter `t`.
	Arc { arc: usize, t: f64 },
	/// A standalone candidate point: a line endpoint, a floating anchor, or an extreme of degenerate geometry.
	Point,
}

/// A unique candidate position, carrying every sample that landed exactly on it (e.g. the shared
/// anchor where two segments join contributes the end sample of one arc and the start sample of the next).
struct HullVertex {
	position: Point,
	tags: Vec<SampleTag>,
}

/// Classification of one edge of the sampled hull polygon.
#[derive(Clone, Copy)]
enum EdgeLabel {
	/// Both endpoints are consecutive samples of the same arc, so the boundary follows that arc here.
	OnArc { arc: usize, t_start: f64, t_end: f64 },
	/// The boundary jumps between different pieces of geometry here.
	Bridge,
}

/// A maximal contiguous piece of input geometry lying on the hull boundary.
enum Contact {
	/// A parameter range of an arc. `t_in`/`t_out` are in boundary traversal order and may be descending
	/// when the hull walks the arc against its parametrization. A zero-span contact is a tangential
	/// touch at a single point (its final extent is determined by refinement).
	Arc { arc: usize, t_in: f64, t_out: f64 },
	/// A single point the boundary bends around: a corner anchor, line endpoint, or floating point.
	Point { position: Point },
}

/// Computes the convex hull of a collection of path segments plus free-floating points, returned as a
/// single closed path. Curved portions of the input that lie on the hull are preserved exactly (as cuts
/// of the original segments), connected by straight tangent lines. Returns an empty path for empty input,
/// and a degenerate path (a single anchor, or a single straight segment) for point-like or collinear input.
pub fn convex_hull_of_geometry(segments: &[PathSeg], loose_points: &[Point]) -> BezPath {
	// Stage 1: normalize the input into curvature-monotone arcs and standalone candidate points.
	let (mut arcs, mut points) = normalize_geometry(segments, loose_points);

	// Establish the overall scale so tolerances can be relative to the input's size
	let scale = geometry_scale(&arcs, &points);
	let distance_epsilon = (scale * 1e-9).max(f64::MIN_POSITIVE);
	assign_sample_counts(&mut arcs, scale);

	points.sort_by(|a, b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap_or(std::cmp::Ordering::Equal));
	points.dedup();

	// Trivial inputs that cannot form a polygonal hull
	if arcs.is_empty() {
		match points.len() {
			0 => return BezPath::new(),
			1 => return BezPath::from_vec(vec![PathEl::MoveTo(points[0])]),
			_ => {}
		}
	}

	// Stage 2: sample all candidate geometry and take the polygonal hull of the samples.
	let vertices = collect_hull_vertices(&arcs, &points);
	let hull = monotone_chain(&vertices);

	match hull.len() {
		0 => return BezPath::new(),
		1 => return BezPath::from_vec(vec![PathEl::MoveTo(vertices[hull[0]].position)]),
		2 => {
			// All input geometry is collinear, so the hull degenerates to a straight segment
			let (a, b) = (vertices[hull[0]].position, vertices[hull[1]].position);
			return BezPath::from_vec(vec![PathEl::MoveTo(a), PathEl::LineTo(b), PathEl::ClosePath]);
		}
		_ => {}
	}

	// Read off the cyclic sequence of arc ranges and corner points forming the boundary
	let labels = label_hull_edges(&vertices, &hull, &arcs);
	let mut contacts = extract_contacts(&vertices, &hull, &labels);

	// Stage 3: refine every transition between contacts to an exact tangency.
	refine_transitions(&mut contacts, &arcs, distance_epsilon);

	// Stage 4: emit the boundary as original-geometry cuts joined by bridge lines.
	emit_hull_path(&contacts, &arcs, segments, distance_epsilon)
}

/// Splits the input into curvature-monotone arcs and standalone candidate points.
/// Curved segments are split at inflections and cusps; lines and degenerate or collinear curves are
/// reduced to the extreme points of the straight line they trace.
fn normalize_geometry(segments: &[PathSeg], loose_points: &[Point]) -> (Vec<ConvexArc>, Vec<Point>) {
	let mut arcs = Vec::new();
	let mut points: Vec<Point> = loose_points.iter().copied().filter(|point| point.is_finite()).collect();

	for (source, segment) in segments.iter().enumerate() {
		let cubic = segment.to_cubic();
		if !(cubic.p0.is_finite() && cubic.p1.is_finite() && cubic.p2.is_finite() && cubic.p3.is_finite()) {
			continue;
		}

		if let PathSeg::Line(line) = segment {
			points.push(line.p0);
			points.push(line.p1);
			continue;
		}

		// The local scale of this segment, for relative degeneracy tests
		let spread = cubic
			.p0
			.distance(cubic.p1)
			.max(cubic.p0.distance(cubic.p2))
			.max(cubic.p0.distance(cubic.p3))
			.max(cubic.p1.distance(cubic.p3));

		// A point-like segment contributes only its position
		if spread < f64::MIN_POSITIVE.max(1e-12) {
			points.push(cubic.p0);
			continue;
		}

		// A curve with collinear control points traces a straight line (possibly overshooting its
		// endpoints), so it contributes the extreme points it reaches along that line
		if let Some(direction) = collinear_direction(&cubic, spread) {
			points.push(cubic.p0);
			points.push(cubic.p3);
			points.extend(straight_curve_interior_extremes(&cubic, direction));
			continue;
		}

		// Split at inflections so each piece turns in only one direction. Cusps satisfy the same
		// equation (the derivative vanishes, so its cross product with the second derivative is zero)
		// and are therefore split points too.
		let mut split_params: Vec<f64> = cubic.inflections().into_iter().filter(|t| (PARAM_EPSILON..1. - PARAM_EPSILON).contains(t)).collect();
		split_params.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
		split_params.dedup();

		let bounds: Vec<f64> = std::iter::once(0.).chain(split_params).chain(std::iter::once(1.)).collect();
		let first_new_arc = arcs.len();
		for window in bounds.windows(2) {
			let (t0, t1) = (window[0], window[1]);
			if t1 - t0 < PARAM_EPSILON {
				continue;
			}
			arcs.push(ConvexArc {
				cubic: cubic.subsegment(t0..t1),
				source,
				source_t0: t0,
				source_t1: t1,
				sample_count: 0,
			});
		}

		// Stitch the exact endpoint positions across the split pieces so junction samples merge
		// bit-exactly during hull vertex deduplication
		for arc_index in first_new_arc..arcs.len() {
			if arc_index > first_new_arc {
				arcs[arc_index].cubic.p0 = arcs[arc_index - 1].cubic.p3;
			}
		}
		if let Some(first) = arcs.get_mut(first_new_arc) {
			first.cubic.p0 = cubic.p0;
		}
		if let Some(last) = arcs.last_mut() {
			last.cubic.p3 = cubic.p3;
		}
	}

	(arcs, points)
}

/// If all four control points of a cubic lie on one line (meaning the curve itself is straight),
/// returns the direction of that line. Uses the longest available chord as the reference direction so
/// out-and-back curves with coincident endpoints are still judged correctly.
fn collinear_direction(cubic: &CubicBez, spread: f64) -> Option<Vec2> {
	let chords = [(cubic.p0, cubic.p3), (cubic.p0, cubic.p1), (cubic.p0, cubic.p2), (cubic.p1, cubic.p3)];
	let (base, tip) = chords
		.into_iter()
		.max_by(|a, b| a.0.distance_squared(a.1).partial_cmp(&b.0.distance_squared(b.1)).unwrap_or(std::cmp::Ordering::Equal))
		.unwrap_or((cubic.p0, cubic.p3));
	let direction = tip - base;

	let tolerance = spread * spread * 1e-12;
	let collinear = [cubic.p0, cubic.p1, cubic.p2, cubic.p3].into_iter().all(|point| direction.cross(point - base).abs() <= tolerance);
	collinear.then(|| direction.normalize())
}

/// The interior parametric extremes of a straight-line cubic (where the curve reverses direction along
/// its line and overshoots past its endpoints).
fn straight_curve_interior_extremes(cubic: &CubicBez, direction: Vec2) -> impl Iterator<Item = Point> + '_ {
	let derivative = cubic.deriv();
	let (d0, d1, d2) = (derivative.p0.to_vec2(), derivative.p1.to_vec2(), derivative.p2.to_vec2());

	// The derivative dotted with the line direction is a quadratic in Bernstein form; convert to power basis
	let (a, b, c) = (d0.dot(direction), d1.dot(direction), d2.dot(direction));
	solve_quadratic(a, 2. * (b - a), a - 2. * b + c)
		.into_iter()
		.filter(|t| (PARAM_EPSILON..1. - PARAM_EPSILON).contains(t))
		.map(|t| cubic.eval(t))
}

/// The overall size of the input, used to make tolerances scale-relative.
fn geometry_scale(arcs: &[ConvexArc], points: &[Point]) -> f64 {
	let mut min = Point::new(f64::INFINITY, f64::INFINITY);
	let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
	let mut include = |point: Point| {
		min = Point::new(min.x.min(point.x), min.y.min(point.y));
		max = Point::new(max.x.max(point.x), max.y.max(point.y));
	};

	for arc in arcs {
		for point in [arc.cubic.p0, arc.cubic.p1, arc.cubic.p2, arc.cubic.p3] {
			include(point);
		}
	}
	for &point in points {
		include(point);
	}

	if min.x > max.x { 0. } else { (max - min).hypot() }
}

/// Assigns each arc a sample density proportional to its size relative to the whole input, so large
/// features are resolved finely without spending thousands of samples on tiny ones.
fn assign_sample_counts(arcs: &mut [ConvexArc], scale: f64) {
	for arc in arcs {
		let control_polygon_length = (arc.cubic.p1 - arc.cubic.p0).hypot() + (arc.cubic.p2 - arc.cubic.p1).hypot() + (arc.cubic.p3 - arc.cubic.p2).hypot();
		let relative_size = if scale > 0. { control_polygon_length / scale } else { 0. };
		arc.sample_count = ((relative_size * 192.).ceil() as usize).clamp(16, 64);
	}
}

/// Samples every arc and merges samples landing on bit-identical positions into shared vertices, so
/// junction anchors carry the tags of both adjoining arcs.
fn collect_hull_vertices(arcs: &[ConvexArc], points: &[Point]) -> Vec<HullVertex> {
	use std::collections::HashMap;

	let mut vertices: Vec<HullVertex> = Vec::new();
	let mut index_by_position: HashMap<(u64, u64), usize> = HashMap::new();
	let mut add = |position: Point, tag: SampleTag| {
		let key = (position.x.to_bits(), position.y.to_bits());
		let index = *index_by_position.entry(key).or_insert_with(|| {
			vertices.push(HullVertex { position, tags: Vec::new() });
			vertices.len() - 1
		});
		vertices[index].tags.push(tag);
	};

	for (arc_index, arc) in arcs.iter().enumerate() {
		for k in 0..=arc.sample_count {
			let t = k as f64 / arc.sample_count as f64;
			// Endpoint samples use the exact control points so shared anchors merge bit-exactly
			let position = match k {
				0 => arc.cubic.p0,
				k if k == arc.sample_count => arc.cubic.p3,
				_ => arc.cubic.eval(t),
			};
			if position.is_finite() {
				add(position, SampleTag::Arc { arc: arc_index, t });
			}
		}
	}

	for &point in points {
		add(point, SampleTag::Point);
	}

	vertices
}

/// Andrew's monotone chain convex hull over the candidate vertices. Returns indices into `vertices` in
/// counterclockwise order (positive signed area), with collinear intermediate points dropped.
fn monotone_chain(vertices: &[HullVertex]) -> Vec<usize> {
	let mut order: Vec<usize> = (0..vertices.len()).collect();
	order.sort_by(|&a, &b| {
		let (pa, pb) = (vertices[a].position, vertices[b].position);
		(pa.x, pa.y).partial_cmp(&(pb.x, pb.y)).unwrap_or(std::cmp::Ordering::Equal)
	});

	if order.len() <= 2 {
		return order;
	}

	let cross = |o: usize, a: usize, b: usize| {
		let (po, pa, pb) = (vertices[o].position, vertices[a].position, vertices[b].position);
		(pa - po).cross(pb - po)
	};

	let mut hull: Vec<usize> = Vec::with_capacity(order.len() + 1);

	// Lower hull
	for &index in &order {
		while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], index) <= 0. {
			hull.pop();
		}
		hull.push(index);
	}

	// Upper hull
	let lower_len = hull.len() + 1;
	for &index in order.iter().rev() {
		while hull.len() >= lower_len && cross(hull[hull.len() - 2], hull[hull.len() - 1], index) <= 0. {
			hull.pop();
		}
		hull.push(index);
	}

	// The final vertex repeats the first
	hull.pop();
	hull
}

/// Classifies each cyclic edge of the hull polygon as either following an arc or bridging between
/// separate pieces of geometry.
fn label_hull_edges(vertices: &[HullVertex], hull: &[usize], arcs: &[ConvexArc]) -> Vec<EdgeLabel> {
	(0..hull.len())
		.map(|i| {
			let (u, v) = (&vertices[hull[i]], &vertices[hull[(i + 1) % hull.len()]]);

			// The edge follows an arc if both endpoints are nearby samples of that same arc
			let mut best: Option<EdgeLabel> = None;
			let mut best_gap = f64::INFINITY;
			for tag_u in &u.tags {
				let &SampleTag::Arc { arc, t: t_start } = tag_u else { continue };
				for tag_v in &v.tags {
					let &SampleTag::Arc { arc: arc_v, t: t_end } = tag_v else { continue };
					if arc_v != arc {
						continue;
					}
					let gap = (t_end - t_start).abs();
					let jump_limit = 2.5 / arcs[arc].sample_count as f64;
					if gap <= jump_limit && gap < best_gap {
						best_gap = gap;
						best = Some(EdgeLabel::OnArc { arc, t_start, t_end });
					}
				}
			}

			best.unwrap_or(EdgeLabel::Bridge)
		})
		.collect()
}

/// Whether edge `b` directly continues edge `a` along the same arc.
fn continues(a: EdgeLabel, b: EdgeLabel) -> bool {
	match (a, b) {
		(EdgeLabel::OnArc { arc: arc_a, t_end, .. }, EdgeLabel::OnArc { arc: arc_b, t_start, .. }) => arc_a == arc_b && t_end == t_start,
		_ => false,
	}
}

/// Groups the labeled hull edges into the cyclic sequence of boundary contacts: maximal arc ranges,
/// and the corner points standing alone between bridges.
fn extract_contacts(vertices: &[HullVertex], hull: &[usize], labels: &[EdgeLabel]) -> Vec<Contact> {
	let edge_count = labels.len();

	// Rotate to start at an edge that does not continue its predecessor, so no arc chain wraps around
	// the seam of the cyclic walk
	let start = (0..edge_count).find(|&i| !continues(labels[(i + edge_count - 1) % edge_count], labels[i])).unwrap_or(0);

	// A corner vertex prefers acting as a refinable touch of an arc (an interior-parameter tag) over a
	// fixed point; anchors and floating points have no interior tag and become fixed corner points
	let vertex_contact = |vertex: &HullVertex| {
		let interior_tag = vertex.tags.iter().find_map(|tag| match tag {
			&SampleTag::Arc { arc, t } if t > 0. && t < 1. => Some((arc, t)),
			_ => None,
		});
		match interior_tag {
			Some((arc, t)) => Contact::Arc { arc, t_in: t, t_out: t },
			None => Contact::Point { position: vertex.position },
		}
	};

	let mut contacts = Vec::new();
	let mut i = 0;
	while i < edge_count {
		let edge_index = (start + i) % edge_count;
		match labels[edge_index] {
			EdgeLabel::OnArc { arc, t_start, mut t_end } => {
				// Extend the chain over every directly continuing edge
				let mut length = 1;
				while i + length < edge_count {
					let next = labels[(start + i + length) % edge_count];
					if !continues(labels[(start + i + length - 1) % edge_count], next) {
						break;
					}
					let EdgeLabel::OnArc { t_end: chained_end, .. } = next else { break };
					t_end = chained_end;
					length += 1;
				}
				contacts.push(Contact::Arc { arc, t_in: t_start, t_out: t_end });
				i += length;
			}
			EdgeLabel::Bridge => {
				// A vertex flanked by bridges on both sides is its own standalone contact
				let previous = labels[(start + i + edge_count - 1) % edge_count];
				if !matches!(previous, EdgeLabel::OnArc { .. }) {
					contacts.push(vertex_contact(&vertices[hull[(start + i) % edge_count]]));
				}
				i += 1;
			}
		}
	}

	contacts
}

/// The tangent parameter on `cubic` whose tangent line passes through `from`, chosen as the candidate
/// nearest `guess` within `window`. Returns `None` when no such tangency exists.
fn nearest_tangent_param(cubic: &CubicBez, from: Point, guess: f64, window: f64) -> Option<f64> {
	cubic
		.tangents_to_point(from)
		.into_iter()
		.filter(|t| (t - guess).abs() <= window)
		.min_by(|a, b| (a - guess).abs().partial_cmp(&(b - guess).abs()).unwrap_or(std::cmp::Ordering::Equal))
}

/// One endpoint of a bridge line: either pinned to a fixed position (a corner anchor or standalone
/// point) or free to slide along an arc to its true tangent point.
enum BridgeEnd {
	Fixed(Point),
	Free { arc: usize, t: f64 },
}

/// Refines every transition between cyclically consecutive contacts so bridge lines touch their adjoining
/// curves at exact tangent points, writing the refined parameters back into the contacts.
fn refine_transitions(contacts: &mut [Contact], arcs: &[ConvexArc], distance_epsilon: f64) {
	let contact_count = contacts.len();
	if contact_count == 0 {
		return;
	}

	// Record each arc contact's sampled traversal direction before any refinement mutates it
	let sampled_ranges: Vec<Option<(f64, f64)>> = contacts
		.iter()
		.map(|contact| match contact {
			&Contact::Arc { t_in, t_out, .. } => Some((t_in, t_out)),
			Contact::Point { .. } => None,
		})
		.collect();

	for i in 0..contact_count {
		let j = (i + 1) % contact_count;

		// Departure state of contact `i` and arrival state of contact `j`. An arc parameter at an
		// exact endpoint is a corner the bridge is pinned to; an interior parameter is a tangency
		// estimate to be refined.
		let out_end = match contacts[i] {
			Contact::Arc { arc, t_out, .. } if t_out > 0. && t_out < 1. => BridgeEnd::Free { arc, t: t_out },
			Contact::Arc { arc, t_out, .. } => BridgeEnd::Fixed(arcs[arc].cubic.eval(t_out)),
			Contact::Point { position } => BridgeEnd::Fixed(position),
		};
		let in_end = match contacts[j] {
			Contact::Arc { arc, t_in, .. } if t_in > 0. && t_in < 1. => BridgeEnd::Free { arc, t: t_in },
			Contact::Arc { arc, t_in, .. } => BridgeEnd::Fixed(arcs[arc].cubic.eval(t_in)),
			Contact::Point { position } => BridgeEnd::Fixed(position),
		};

		let (refined_out, refined_in) = match (out_end, in_end) {
			(BridgeEnd::Fixed(_), BridgeEnd::Fixed(_)) => (None, None),
			(BridgeEnd::Fixed(from), BridgeEnd::Free { arc, t }) => {
				let window = 2.5 / arcs[arc].sample_count as f64;
				(None, nearest_tangent_param(&arcs[arc].cubic, from, t, window))
			}
			(BridgeEnd::Free { arc, t }, BridgeEnd::Fixed(from)) => {
				let window = 2.5 / arcs[arc].sample_count as f64;
				(nearest_tangent_param(&arcs[arc].cubic, from, t, window), None)
			}
			(BridgeEnd::Free { arc: arc_a, t: mut s }, BridgeEnd::Free { arc: arc_b, mut t }) => {
				// Alternate exact tangent-through-point solves until the bridge is tangent to both
				// curves. Each half-step is a closed-form quartic solve, so the iteration is stable;
				// nearest-to-guess root selection keeps it anchored to the sampled estimate.
				let window_a = 2.5 / arcs[arc_a].sample_count as f64;
				let window_b = 2.5 / arcs[arc_b].sample_count as f64;
				let (guess_s, guess_t) = (s, t);

				for _ in 0..MAX_BITANGENT_ITERATIONS {
					let from_b = arcs[arc_b].cubic.eval(t);
					if from_b.distance(arcs[arc_a].cubic.eval(s)) < distance_epsilon {
						break;
					}
					let new_s = nearest_tangent_param(&arcs[arc_a].cubic, from_b, guess_s, window_a).unwrap_or(s);
					let new_t = nearest_tangent_param(&arcs[arc_b].cubic, arcs[arc_a].cubic.eval(new_s), guess_t, window_b).unwrap_or(t);

					let converged = (new_s - s).abs() < TANGENCY_TOLERANCE && (new_t - t).abs() < TANGENCY_TOLERANCE;
					(s, t) = (new_s, new_t);
					if converged {
						break;
					}
				}

				(Some(s), Some(t))
			}
		};

		if let (Some(t), Contact::Arc { t_out, .. }) = (refined_out, &mut contacts[i]) {
			*t_out = t;
		}
		if let (Some(t), Contact::Arc { t_in, .. }) = (refined_in, &mut contacts[j]) {
			*t_in = t;
		}
	}

	// If refining both ends of a short contact made its parameters cross over, the contact has no real
	// extent on the boundary; collapse it to a single tangency point so emission stays consistent
	for (contact, sampled) in contacts.iter_mut().zip(sampled_ranges) {
		let (Contact::Arc { t_in, t_out, .. }, Some((sampled_in, sampled_out))) = (contact, sampled) else {
			continue;
		};
		if sampled_in != sampled_out && (sampled_out - sampled_in).signum() != (*t_out - *t_in).signum() {
			let midpoint = (*t_in + *t_out) / 2.;
			(*t_in, *t_out) = (midpoint, midpoint);
		}
	}
}

/// Builds the final closed path: each arc contact becomes a cut of its original source segment
/// (preserving the input's exact geometry and segment kind), and consecutive contacts are joined by
/// straight bridge lines wherever their endpoints do not already coincide.
fn emit_hull_path(contacts: &[Contact], arcs: &[ConvexArc], segments: &[PathSeg], distance_epsilon: f64) -> BezPath {
	let contact_in_position = |contact: &Contact| match contact {
		&Contact::Arc { arc, t_in, .. } => arcs[arc].cubic.eval(t_in),
		Contact::Point { position } => *position,
	};
	let contact_out_position = |contact: &Contact| match contact {
		&Contact::Arc { arc, t_out, .. } => arcs[arc].cubic.eval(t_out),
		Contact::Point { position } => *position,
	};

	let mut path = BezPath::new();
	let Some(first) = contacts.first() else { return path };
	path.move_to(contact_in_position(first));

	for (i, contact) in contacts.iter().enumerate() {
		// The portion of original geometry this contact contributes
		if let &Contact::Arc { arc, t_in, t_out } = contact
			&& (t_out - t_in).abs() > PARAM_EPSILON
		{
			let arc = &arcs[arc];
			let source = &segments[arc.source];
			let (source_in, source_out) = (arc.to_source_t(t_in), arc.to_source_t(t_out));

			// Cut the range out of the source segment, preserving its exact control points when the
			// whole segment lies on the hull
			let piece = if source_in == 0. && source_out == 1. {
				*source
			} else if source_in == 1. && source_out == 0. {
				source.reverse()
			} else {
				source.subsegment(source_in..source_out)
			};

			match piece {
				PathSeg::Line(line) => path.line_to(line.p1),
				PathSeg::Quad(quad) => path.quad_to(quad.p1, quad.p2),
				PathSeg::Cubic(cubic) => path.curve_to(cubic.p1, cubic.p2, cubic.p3),
			}
		}

		// The bridge line to the next contact, unless the two already meet at a shared anchor. The
		// final bridge back to the start is left to the implicit closing line.
		if i + 1 < contacts.len() {
			let next_in = contact_in_position(&contacts[i + 1]);
			if contact_out_position(contact).distance(next_in) > distance_epsilon {
				path.line_to(next_in);
			}
		}
	}

	path.close_path();
	path
}

#[cfg(test)]
mod tests {
	use super::*;
	use kurbo::{Line, ParamCurveNearest, QuadBez, Shape};

	/// Circle approximation constant for cubic Bezier quadrants.
	const KAPPA: f64 = 0.552284749831;

	/// A circle as four cubic quadrants, counterclockwise in mathematical (Y-up) orientation.
	fn circle_segments(center: Point, radius: f64) -> Vec<PathSeg> {
		let anchor = |dx: f64, dy: f64| Point::new(center.x + dx * radius, center.y + dy * radius);
		let quadrant = |a: Point, b: Point| {
			let handle_a = Point::new(a.x - (a.y - center.y) * KAPPA, a.y + (a.x - center.x) * KAPPA);
			let handle_b = Point::new(b.x + (b.y - center.y) * KAPPA, b.y - (b.x - center.x) * KAPPA);
			PathSeg::Cubic(CubicBez::new(a, handle_a, handle_b, b))
		};

		let (right, top, left, bottom) = (anchor(1., 0.), anchor(0., 1.), anchor(-1., 0.), anchor(0., -1.));
		vec![quadrant(right, top), quadrant(top, left), quadrant(left, bottom), quadrant(bottom, right)]
	}

	/// The hull polygon as densely sampled points, for geometric property checks.
	fn sample_hull_polygon(hull: &BezPath) -> Vec<Point> {
		let mut polygon = Vec::new();
		for segment in hull.segments() {
			for k in 0..64 {
				polygon.push(segment.eval(k as f64 / 64.));
			}
		}
		polygon
	}

	/// Asserts the four defining properties of a valid hull: the path is closed, its boundary is convex,
	/// it contains all the input geometry, and its curved portions lie exactly on the input curves.
	fn assert_hull_valid(hull: &BezPath, segments: &[PathSeg], loose_points: &[Point]) {
		assert!(matches!(hull.elements().last(), Some(PathEl::ClosePath)), "hull must be a closed path");

		let polygon = sample_hull_polygon(hull);
		assert!(polygon.len() >= 3, "hull must enclose an area");

		let scale = polygon.iter().map(|p| p.to_vec2().hypot()).fold(0., f64::max).max(1.);
		let turn_tolerance = scale * scale * 1e-9;

		// Convex and counterclockwise: every consecutive turn is a left turn (within tolerance)
		let n = polygon.len();
		for i in 0..n {
			let (a, b, c) = (polygon[i], polygon[(i + 1) % n], polygon[(i + 2) % n]);
			let turn = (b - a).cross(c - b);
			assert!(turn >= -turn_tolerance, "hull boundary must be convex, found right turn of {turn} at {b:?}");
		}

		// Containment: all input geometry lies inside the hull polygon. The polygon is a sampling of
		// the true hull, so allow a tolerance for the flatness error between polygon samples.
		let containment_tolerance = scale * 1e-4;
		let inside = |point: Point| (0..n).all(|i| (polygon[(i + 1) % n] - polygon[i]).cross(point - polygon[i]) >= -containment_tolerance * scale);
		for segment in segments {
			for k in 0..=100 {
				let point = segment.eval(k as f64 / 100.);
				assert!(inside(point), "input point {point:?} on {segment:?} must be inside the hull");
			}
		}
		for &point in loose_points {
			assert!(inside(point), "loose input point {point:?} must be inside the hull");
		}

		// Faithfulness: every curved piece of the hull lies on some input segment (bridge lines are the
		// only geometry the hull is allowed to invent)
		let on_input_tolerance = scale * 1e-6;
		for piece in hull.segments() {
			if matches!(piece, PathSeg::Line(_)) {
				continue;
			}
			for k in 0..=16 {
				let point = piece.eval(k as f64 / 16.);
				let distance = segments.iter().map(|segment| segment.nearest(point, 1e-9).distance_sq.sqrt()).fold(f64::INFINITY, f64::min);
				assert!(distance <= on_input_tolerance, "hull curve point {point:?} must lie on the input geometry (distance {distance})");
			}
		}
	}

	/// Asserts that every straight bridge line in the hull meets its adjacent curved pieces tangentially
	/// (the line direction is parallel to the curve tangent at the junction). Only valid for inputs whose
	/// bridges are all tangential (no corner anchors on the hull).
	fn assert_bridges_tangent(hull: &BezPath) {
		let pieces: Vec<PathSeg> = hull.segments().collect();
		let n = pieces.len();

		let tangent_at = |piece: &PathSeg, end: bool| match piece {
			PathSeg::Line(line) => line.p1 - line.p0,
			PathSeg::Quad(quad) => {
				if end {
					quad.p2 - quad.p1
				} else {
					quad.p1 - quad.p0
				}
			}
			PathSeg::Cubic(cubic) => {
				if end {
					cubic.p3 - cubic.p2
				} else {
					cubic.p1 - cubic.p0
				}
			}
		};

		let mut checked_junctions = 0;
		for i in 0..n {
			let (piece, next) = (&pieces[i], &pieces[(i + 1) % n]);
			let is_line = |p: &PathSeg| matches!(p, PathSeg::Line(_));
			if is_line(piece) == is_line(next) {
				continue;
			}

			let outgoing = tangent_at(piece, true).normalize();
			let incoming = tangent_at(next, false).normalize();
			let deviation = outgoing.cross(incoming).abs();
			assert!(deviation < 1e-6, "bridge line must be tangent to the adjacent curve, found angle deviation {deviation}");
			checked_junctions += 1;
		}
		assert!(checked_junctions > 0, "expected at least one line-to-curve junction to check");
	}

	/// Control points of every cubic in the path, for identity comparisons.
	fn cubics_of(path_segments: impl Iterator<Item = PathSeg>) -> Vec<[Point; 4]> {
		path_segments
			.filter_map(|segment| match segment {
				PathSeg::Cubic(cubic) => Some([cubic.p0, cubic.p1, cubic.p2, cubic.p3]),
				_ => None,
			})
			.collect()
	}

	fn assert_same_cubic_set(a: &[[Point; 4]], b: &[[Point; 4]], tolerance: f64) {
		assert_eq!(a.len(), b.len(), "cubic counts must match");
		for cubic in a {
			let found = b.iter().any(|other| cubic.iter().zip(other).all(|(p, q)| p.distance(*q) <= tolerance));
			assert!(found, "cubic {cubic:?} has no match");
		}
	}

	#[test]
	fn empty_input_gives_empty_hull() {
		let hull = convex_hull_of_geometry(&[], &[]);
		assert!(hull.elements().is_empty());
	}

	#[test]
	fn single_point_gives_single_anchor() {
		let hull = convex_hull_of_geometry(&[], &[Point::new(5., 7.)]);
		assert_eq!(hull.elements(), &[PathEl::MoveTo(Point::new(5., 7.))]);
	}

	#[test]
	fn collinear_points_give_degenerate_segment() {
		let points: Vec<Point> = (0..7).map(|i| Point::new(i as f64 * 10., i as f64 * 5.)).collect();
		let hull = convex_hull_of_geometry(&[], &points);
		assert_eq!(hull.elements().len(), 3, "expected move, line, close");
		assert!(hull.elements().contains(&PathEl::MoveTo(Point::new(0., 0.))));
		assert!(hull.elements().contains(&PathEl::LineTo(Point::new(60., 30.))));
	}

	#[test]
	fn points_only_form_polygon_hull() {
		let points = [
			Point::new(0., 0.),
			Point::new(100., 0.),
			Point::new(100., 100.),
			Point::new(0., 100.),
			Point::new(50., 50.),
			Point::new(25., 75.),
		];
		let hull = convex_hull_of_geometry(&[], &points);
		assert_hull_valid(&hull, &[], &points);
		assert!((hull.area().abs() - 10_000.).abs() < 1e-9, "hull must be the outer square, got area {}", hull.area());
		assert_eq!(hull.segments().count(), 4, "square hull must have exactly 4 edges");
	}

	#[test]
	fn convex_closed_shape_is_unchanged() {
		let segments = circle_segments(Point::new(20., -30.), 100.);
		let hull = convex_hull_of_geometry(&segments, &[]);

		assert_hull_valid(&hull, &segments, &[]);
		assert_same_cubic_set(&cubics_of(hull.segments()), &cubics_of(segments.iter().copied()), 1e-9);
	}

	#[test]
	fn clockwise_convex_shape_is_reversed_not_cut() {
		let segments: Vec<PathSeg> = circle_segments(Point::new(0., 0.), 50.).iter().rev().map(|segment| segment.reverse()).collect();
		let hull = convex_hull_of_geometry(&segments, &[]);

		assert_hull_valid(&hull, &segments, &[]);
		let expected: Vec<PathSeg> = segments.iter().map(|segment| segment.reverse()).collect();
		assert_same_cubic_set(&cubics_of(hull.segments()), &cubics_of(expected.into_iter()), 1e-9);
	}

	#[test]
	fn concave_side_is_bridged_with_a_straight_line() {
		// A half-moon: two convex quadrants on top, and a bottom edge that bulges inward (upward)
		let circle = circle_segments(Point::new(0., 0.), 100.);
		let segments = vec![
			circle[0],
			circle[1],
			PathSeg::Cubic(CubicBez::new(Point::new(-100., 0.), Point::new(-50., 60.), Point::new(50., 60.), Point::new(100., 0.))),
		];
		let hull = convex_hull_of_geometry(&segments, &[]);

		assert_hull_valid(&hull, &segments, &[]);

		// The two convex quadrants survive unchanged and the concave bottom is replaced by one straight line
		assert_same_cubic_set(&cubics_of(hull.segments()), &cubics_of(circle[0..2].iter().copied()), 1e-9);
		let line_count = hull.segments().filter(|segment| matches!(segment, PathSeg::Line(_))).count();
		assert_eq!(line_count, 1, "the concave side must collapse to exactly one bridge line");

		let semicircle_area = std::f64::consts::PI * 100. * 100. / 2.;
		assert!((hull.area().abs() - semicircle_area).abs() / semicircle_area < 1e-3);
	}

	#[test]
	fn disjoint_shapes_are_bridged_with_exact_tangents() {
		// Different radii so the outer bitangents are slanted and touch mid-arc, exercising the
		// free-free tangency refinement rather than corner-to-corner bridging
		let mut segments = circle_segments(Point::new(0., 0.), 50.);
		segments.extend(circle_segments(Point::new(300., 20.), 80.));
		let hull = convex_hull_of_geometry(&segments, &[]);

		assert_hull_valid(&hull, &segments, &[]);
		assert_bridges_tangent(&hull);

		let bridge_count = hull.segments().filter(|segment| matches!(segment, PathSeg::Line(_))).count();
		assert_eq!(bridge_count, 2, "two disjoint shapes must be connected by exactly two bridge lines");
	}

	#[test]
	fn floating_point_outside_shape_is_wrapped() {
		let segments = circle_segments(Point::new(0., 0.), 100.);
		let outlier = Point::new(400., 30.);
		let interior = Point::new(10., 20.);
		let hull = convex_hull_of_geometry(&segments, &[outlier, interior]);

		assert_hull_valid(&hull, &segments, &[outlier, interior]);
		assert_bridges_tangent(&hull);

		// The two tangent lines meet at the outlier point
		let lines: Vec<Line> = hull.segments().filter_map(|segment| if let PathSeg::Line(line) = segment { Some(line) } else { None }).collect();
		assert_eq!(lines.len(), 2);
		assert!(lines.iter().any(|line| line.p0.distance(outlier) < 1e-9 || line.p1.distance(outlier) < 1e-9));
	}

	#[test]
	fn open_convex_arc_is_closed_with_a_chord() {
		let segments = vec![circle_segments(Point::new(0., 0.), 100.)[0]];
		let hull = convex_hull_of_geometry(&segments, &[]);

		assert_hull_valid(&hull, &segments, &[]);
		assert_same_cubic_set(&cubics_of(hull.segments()), &cubics_of(segments.iter().copied()), 1e-9);
	}

	#[test]
	fn three_quarter_circle_keeps_arcs_and_adds_chord() {
		let segments = circle_segments(Point::new(0., 0.), 100.)[0..3].to_vec();
		let hull = convex_hull_of_geometry(&segments, &[]);

		assert_hull_valid(&hull, &segments, &[]);
		assert_same_cubic_set(&cubics_of(hull.segments()), &cubics_of(segments.iter().copied()), 1e-9);

		// Three quarters of the circle plus the triangle between the two open endpoints and the center
		let expected_area = std::f64::consts::PI * 100. * 100. * 0.75 + 100. * 100. / 2.;
		assert!((hull.area().abs() - expected_area).abs() / expected_area < 1e-3, "got area {}", hull.area());
	}

	#[test]
	fn s_curve_is_split_at_its_inflection() {
		let s_curve = PathSeg::Cubic(CubicBez::new(Point::new(0., 0.), Point::new(100., 0.), Point::new(0., 100.), Point::new(100., 100.)));
		let hull = convex_hull_of_geometry(&[s_curve], &[]);

		assert_hull_valid(&hull, &[s_curve], &[]);

		// Each side of the S contributes a convex piece, so the hull must contain both curves and lines
		assert!(hull.segments().any(|segment| matches!(segment, PathSeg::Cubic(_))));
		assert!(hull.segments().any(|segment| matches!(segment, PathSeg::Line(_))));
	}

	#[test]
	fn quadratic_segments_are_preserved_as_quadratics() {
		// A convex closed shape made of four outward-bulging quadratics
		let quad = |a: Point, control: Point, b: Point| PathSeg::Quad(QuadBez::new(a, control, b));
		let segments = vec![
			quad(Point::new(100., 0.), Point::new(100., 100.), Point::new(0., 100.)),
			quad(Point::new(0., 100.), Point::new(-100., 100.), Point::new(-100., 0.)),
			quad(Point::new(-100., 0.), Point::new(-100., -100.), Point::new(0., -100.)),
			quad(Point::new(0., -100.), Point::new(100., -100.), Point::new(100., 0.)),
		];
		let hull = convex_hull_of_geometry(&segments, &[]);

		assert_hull_valid(&hull, &segments, &[]);
		assert_eq!(
			hull.segments().filter(|segment| matches!(segment, PathSeg::Quad(_))).count(),
			4,
			"quadratic segments must stay quadratic"
		);
	}

	#[test]
	fn straight_line_cubic_overshoot_reaches_its_extreme() {
		// A cubic whose control points are collinear and overshoot past its end anchor: the curve
		// travels out to some x beyond 100 and doubles back, so the hull must reach that far point
		let overshoot = PathSeg::Cubic(CubicBez::new(Point::new(0., 0.), Point::new(300., 0.), Point::new(300., 0.), Point::new(100., 0.)));
		let above = Point::new(0., 50.);
		let hull = convex_hull_of_geometry(&[overshoot], &[above]);

		// The parametric maximum of x(t) along the line, estimated by dense sampling (the sampling
		// undershoots the true maximum slightly, so the hull may exceed it by the sampling error)
		let sampled_max_x = (0..=1000).map(|i| overshoot.eval(i as f64 / 1000.).x).fold(f64::NEG_INFINITY, f64::max);
		assert!(sampled_max_x > 150., "test setup must actually overshoot");

		let hull_max_x = sample_hull_polygon(&hull).iter().map(|point| point.x).fold(f64::NEG_INFINITY, f64::max);
		assert!(hull_max_x >= sampled_max_x - 1e-9, "hull must reach at least the sampled extreme: {hull_max_x} vs {sampled_max_x}");
		assert!(hull_max_x <= sampled_max_x + 0.01, "hull must not overshoot the true extreme: {hull_max_x} vs {sampled_max_x}");
		assert_hull_valid(&hull, &[overshoot], &[above]);
	}

	#[test]
	fn interior_geometry_is_ignored() {
		let mut segments = circle_segments(Point::new(0., 0.), 100.);
		let outer = segments.clone();
		segments.extend(circle_segments(Point::new(10., 5.), 30.));
		segments.push(PathSeg::Line(Line::new(Point::new(-20., -20.), Point::new(20., 20.))));
		let hull = convex_hull_of_geometry(&segments, &[Point::new(0., 40.)]);

		assert_hull_valid(&hull, &segments, &[]);
		assert_same_cubic_set(&cubics_of(hull.segments()), &cubics_of(outer.iter().copied()), 1e-9);
	}

	#[test]
	fn equal_shapes_bridge_exactly_between_anchors() {
		// Equal radii make the outer bitangents horizontal, touching each circle exactly at the anchor
		// points of its quadrant construction, exercising the corner-to-corner bridge path
		let mut segments = circle_segments(Point::new(0., 0.), 50.);
		segments.extend(circle_segments(Point::new(200., 0.), 50.));
		let hull = convex_hull_of_geometry(&segments, &[]);

		assert_hull_valid(&hull, &segments, &[]);

		let lines: Vec<Line> = hull.segments().filter_map(|segment| if let PathSeg::Line(line) = segment { Some(line) } else { None }).collect();
		assert_eq!(lines.len(), 2);
		for line in lines {
			assert!(
				(line.p0.y.abs() - 50.).abs() < 1e-9 && (line.p1.y - line.p0.y).abs() < 1e-9,
				"bridges must be the horizontal bitangents, got {line:?}"
			);
		}
	}

	#[test]
	fn overlapping_shapes_are_wrapped_together() {
		let mut segments = circle_segments(Point::new(0., 0.), 60.);
		segments.extend(circle_segments(Point::new(70., 25.), 60.));
		let hull = convex_hull_of_geometry(&segments, &[]);

		assert_hull_valid(&hull, &segments, &[]);
		assert_bridges_tangent(&hull);
	}

	#[test]
	fn randomized_inputs_always_produce_valid_hulls() {
		// A deterministic PRNG so failures are reproducible
		let mut state: u64 = 0x853c49e6748fea9b;
		let mut random = move || {
			state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
			(state >> 11) as f64 / (1u64 << 53) as f64
		};

		for iteration in 0..200 {
			let mut point = || Point::new(random() * 1000., random() * 1000.);

			let mut segments = Vec::new();
			let mut loose_points = Vec::new();
			for _ in 0..(1 + iteration % 5) {
				match iteration % 3 {
					0 => segments.push(PathSeg::Cubic(CubicBez::new(point(), point(), point(), point()))),
					1 => segments.push(PathSeg::Quad(QuadBez::new(point(), point(), point()))),
					_ => segments.push(PathSeg::Line(Line::new(point(), point()))),
				}
				if iteration % 4 == 0 {
					loose_points.push(point());
				}
			}

			let hull = convex_hull_of_geometry(&segments, &loose_points);
			assert_hull_valid(&hull, &segments, &loose_points);
		}
	}

	#[test]
	fn mixed_open_subpaths_and_points_are_all_wrapped() {
		let mut segments = vec![
			PathSeg::Line(Line::new(Point::new(-200., -50.), Point::new(-180., 40.))),
			PathSeg::Line(Line::new(Point::new(-180., 40.), Point::new(-120., 60.))),
			PathSeg::Cubic(CubicBez::new(Point::new(100., -80.), Point::new(160., -20.), Point::new(160., 40.), Point::new(100., 90.))),
		];
		segments.extend(circle_segments(Point::new(0., 200.), 40.));
		let loose = [Point::new(0., -150.), Point::new(10., 0.)];
		let hull = convex_hull_of_geometry(&segments, &loose);

		assert_hull_valid(&hull, &segments, &loose);
	}
}
