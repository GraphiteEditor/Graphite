//! Concave hull of Bezier path geometry: the region a ball rolled around the outside cannot reach.
//!
//! The ball rolls around the geometry from outside, unable to cross any curve or point or to squeeze through a gap
//! narrower than its diameter. Everything it cannot reach is the hull. The boundary follows the input curves wherever
//! the ball touches them, and spans each gap where the ball touches two places at once with a straight line. The ball is
//! sized by its curvature, one over its radius. At zero curvature the ball is flat and can enter no dent at all, leaving
//! the convex hull, which [`convex_hull_of_geometry`] constructs exactly.
//!
//! The algorithm proceeds in five stages:
//! 1. Normalize: split every curve at its inflections and cusps so each piece turns one way. Unlike for the convex hull,
//!    straight pieces stay, since they are walls the ball cannot pass.
//! 2. Sample: place samples along every piece much closer together than the ball's diameter, so it cannot slip between them.
//! 3. Flood: triangulate the samples and spread in from the outside across every edge with a dual Voronoi edge leaving the
//!    ball room to pass. The edges where the flood stops are the boundary, read off as cycles of samples.
//! 4. Refine: solve each place the ball touches two pieces of geometry at once to its exact contact points. Where any
//!    decision falls within the sampling's error, densify the samples there and repeat.
//! 5. Emit: cut the touched ranges out of the original segments and join them with the spans, tracing strands once.

use super::convex_hull::{PARAM_EPSILON, collinear_direction, convex_hull_of_geometry, position_key, source_cut, straight_curve_reversals};
use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveDeriv, ParamCurveNearest, PathSeg, Point, QuadBez, Vec2};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::f64::consts::PI;
use tinyvec::TinyVec;

/// Sample spacing along each piece of geometry, as a fraction of the ball's radius. Below 2 the ball cannot slip
/// between neighboring samples, and the margin under that keeps most flood decisions clear of the sampling error.
const SPACING_PER_RADIUS: f64 = 0.25;
/// The most a curve may turn between neighboring samples, so every sample interval is nearly straight.
const MAX_TURN_PER_INTERVAL: f64 = PI / 12.;
/// Sample budget for the triangulation, which sets a floor on the ball's radius relative to the total length of the geometry.
const MAX_SAMPLES: usize = 200_000;
/// Rounds of densifying samples near decisions too close to call before accepting the result.
const MAX_REFINEMENT_ROUNDS: usize = 6;
/// A ball this many times larger than the geometry is indistinguishable from flat, so the exact convex hull is used.
const FLAT_BALL_RADIUS_PER_SCALE: f64 = 1e6;
/// Iteration cap for solving where a span touches the geometry.
const MAX_SPAN_ITERATIONS: usize = 50;
/// Residual (the sine of an angle) at which a span's contact solve counts as converged.
const SPAN_TOLERANCE: f64 = 1e-12;
/// How many sample intervals past the sampled boundary a span's contact may lie.
const CONTACT_REACH: usize = 2;
/// Stands in for the vertex at infinity that the ghost triangles wrapping the outside of the triangulation share.
const GHOST: u32 = u32::MAX;
/// How many cells wide the grid that [`hilbert_index`] numbers is.
const HILBERT_GRID_SIDE: u32 = 1 << 16;

/// One piece of an input segment that turns only one way (or not at all), which the ball may touch along a range.
struct HullArc {
	/// Cubic representation of this piece, used for all internal math (exact degree elevation for lines and quadratics).
	cubic: CubicBez,
	/// Index into the input segment list identifying the segment this arc is a piece of.
	source: usize,
	/// Parameter range of the source segment covered by this piece.
	source_t0: f64,
	source_t1: f64,
	/// Whether this piece is a straight line, which has no curvature to resolve.
	straight: bool,
	/// Sample parameters in ascending order from 0 to 1.
	samples: Vec<f64>,
}

impl HullArc {
	/// Map a local parameter on this arc to a parameter on its source segment.
	fn to_source_t(&self, t: f64) -> f64 {
		self.source_t0 + t * (self.source_t1 - self.source_t0)
	}
}

/// What a sample vertex lies on.
#[derive(Clone, Copy, Default)]
enum Membership {
	/// The sample at `index` along an arc.
	Arc { arc: u32, index: u32 },
	/// A free-floating point, or a segment so small it is only a point.
	#[default]
	Point,
}

/// A unique sample position, carrying every sample that landed on it (such as the anchor two arcs share).
struct SampleVertex {
	position: Point,
	memberships: TinyVec<[Membership; 2]>,
}

/// The sample vertices of one round, merged by position.
struct SampleSet {
	vertices: Vec<SampleVertex>,
	/// The vertex at each sample of each arc.
	arc_vertices: Vec<Vec<u32>>,
}

/// Everything about the request that stays fixed across refinement rounds.
struct HullContext<'a> {
	segments: &'a [PathSeg],
	points: &'a [Point],
	radius: f64,
	distance_epsilon: f64,
}

/// Computes the concave hull of path segments plus free-floating points for a ball of the given curvature (one over its
/// radius), zero giving the convex hull. Returns a closed path around each region, an open path along each strand without
/// area, and a lone point for each point the ball circles all the way around, meeting only at exactly shared anchors.
pub fn concave_hull_of_geometry(segments: &[PathSeg], loose_points: &[Point], curvature: f64) -> BezPath {
	// A flat ball touches only the convex hull, which has its own exact construction
	if curvature.is_nan() || curvature <= 0. {
		return convex_hull_of_geometry(segments, loose_points);
	}

	// Stage 1: normalize the input into one-way arcs and standalone points
	let (mut arcs, points) = normalize_geometry(segments, loose_points);
	let scale = geometry_scale(&arcs, &points);
	if 1. / curvature >= scale * FLAT_BALL_RADIUS_PER_SCALE {
		return convex_hull_of_geometry(segments, loose_points);
	}

	// Stage 2: sample every arc closer together than the ball's diameter, within the sample budget
	let total_length: f64 = arcs.iter().map(|arc| estimated_length(&arc.cubic)).sum();
	let radius = (1. / curvature).max(total_length / (MAX_SAMPLES as f64 * SPACING_PER_RADIUS)).max(scale * 1e-9);
	for arc in &mut arcs {
		arc.samples = sample_arc(&arc.cubic, arc.straight, radius * SPACING_PER_RADIUS);
	}

	let context = HullContext {
		segments,
		points: &points,
		radius,
		distance_epsilon: (scale * 1e-9).max(f64::MIN_POSITIVE),
	};

	// Stages 3 to 5, repeated with denser samples wherever a round finds a decision too close to call
	let mut round = 0;
	loop {
		let (hull, flagged) = trace_hull(&arcs, &context);

		let sample_count: usize = arcs.iter().map(|arc| arc.samples.len()).sum();
		let finished = flagged.is_empty() || round == MAX_REFINEMENT_ROUNDS || sample_count > MAX_SAMPLES;
		if finished || !densify(&mut arcs, &flagged, context.distance_epsilon * 1e3) {
			return hull;
		}
		round += 1;
	}
}

/// Splits the input into one-way arcs and standalone points. Curved segments are split at their inflections and cusps,
/// straight ones (including curves with collinear control points) where they reverse along their line.
fn normalize_geometry(segments: &[PathSeg], loose_points: &[Point]) -> (Vec<HullArc>, Vec<Point>) {
	let mut arcs = Vec::new();
	let mut points: Vec<Point> = loose_points.iter().copied().filter(|point| point.is_finite()).collect();

	for (source, segment) in segments.iter().enumerate() {
		// Lines are elevated so their parameters stay those of the source line
		let cubic = match *segment {
			PathSeg::Line(Line { p0, p1 }) => CubicBez::new(p0, p0.lerp(p1, 1. / 3.), p0.lerp(p1, 2. / 3.), p1),
			_ => segment.to_cubic(),
		};
		if ![cubic.p0, cubic.p1, cubic.p2, cubic.p3].iter().all(|point| point.is_finite()) {
			continue;
		}

		// A point-like segment contributes only its position
		let spread = cubic
			.p0
			.distance(cubic.p1)
			.max(cubic.p0.distance(cubic.p2))
			.max(cubic.p0.distance(cubic.p3))
			.max(cubic.p1.distance(cubic.p3));
		if spread < f64::MIN_POSITIVE.max(1e-12) {
			points.push(cubic.p0);
			continue;
		}

		let direction = collinear_direction(&cubic, spread);
		let mut split_params: Vec<f64> = match direction {
			Some(direction) => straight_curve_reversals(&cubic, direction).collect(),
			None => cubic.inflections().into_iter().filter(|t| (PARAM_EPSILON..1. - PARAM_EPSILON).contains(t)).collect(),
		};
		split_params.sort_by(f64::total_cmp);
		split_params.dedup();

		let bounds: Vec<f64> = std::iter::once(0.).chain(split_params).chain(std::iter::once(1.)).collect();
		let first_new_arc = arcs.len();
		for window in bounds.windows(2) {
			let (t0, t1) = (window[0], window[1]);
			if t1 - t0 < PARAM_EPSILON {
				continue;
			}
			arcs.push(HullArc {
				cubic: cubic.subsegment(t0..t1),
				source,
				source_t0: t0,
				source_t1: t1,
				straight: direction.is_some(),
				samples: Vec::new(),
			});
		}

		// Stitch the exact endpoint positions across the split pieces so their shared samples merge exactly
		for arc_index in first_new_arc + 1..arcs.len() {
			arcs[arc_index].cubic.p0 = arcs[arc_index - 1].cubic.p3;
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

/// The corners of a box holding all the input, which its control points bound. Inverted when there is no input.
fn geometry_bounds(arcs: &[HullArc], points: &[Point]) -> (Point, Point) {
	let positions = arcs.iter().flat_map(|arc| [arc.cubic.p0, arc.cubic.p1, arc.cubic.p2, arc.cubic.p3]).chain(points.iter().copied());
	positions.fold((Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY)), |(min, max), point| {
		(Point::new(min.x.min(point.x), min.y.min(point.y)), Point::new(max.x.max(point.x), max.y.max(point.y)))
	})
}

/// The overall size of the input, used to make tolerances scale-relative.
fn geometry_scale(arcs: &[HullArc], points: &[Point]) -> f64 {
	let (min, max) = geometry_bounds(arcs, points);
	if min.x > max.x { 0. } else { (max - min).hypot() }
}

/// A cheap length estimate, between the chord and the control polygon, which bound the true length.
fn estimated_length(cubic: &CubicBez) -> f64 {
	let polygon = (cubic.p1 - cubic.p0).hypot() + (cubic.p2 - cubic.p1).hypot() + (cubic.p3 - cubic.p2).hypot();
	(polygon + (cubic.p3 - cubic.p0).hypot()) / 2.
}

/// The unit tangent of a cubic at `t`, falling back to the direction just inside the curve where a handle retracted
/// onto its anchor leaves no derivative.
fn unit_tangent(cubic: &CubicBez, t: f64) -> Vec2 {
	let derivative = cubic.deriv();
	let tangent = derivative.eval(t).to_vec2();
	if tangent.hypot() > estimated_length(cubic) * 1e-9 {
		return tangent.normalize();
	}

	let nearby = derivative.eval(if t < 0.5 { t + 1e-6 } else { t - 1e-6 }).to_vec2();
	if nearby.hypot() > 0. { nearby.normalize() } else { (cubic.p3 - cubic.p0).normalize() }
}

/// The signed curvature of a cubic at `t`, positive where it turns left as the parameter increases.
fn signed_curvature(cubic: &CubicBez, t: f64) -> f64 {
	let first = cubic.deriv();
	let (velocity, acceleration) = (first.eval(t).to_vec2(), first.deriv().eval(t).to_vec2());
	let speed = velocity.hypot();

	if speed > 0. { velocity.cross(acceleration) / (speed * speed * speed) } else { 0. }
}

/// The unsigned angle between two directions.
fn angle_between(a: Vec2, b: Vec2) -> f64 {
	a.cross(b).atan2(a.dot(b)).abs()
}

/// Parameters from 0 to 1 splitting an arc into intervals with chords that fit within `spacing` and tangents that turn little.
fn sample_arc(cubic: &CubicBez, straight: bool, spacing: f64) -> Vec<f64> {
	// Halve each interval until it is short and nearly straight, depth first so the samples come out in order. Curves
	// start from quarters so a long arc's total turn is never mistaken for a small one.
	let minimum_intervals = if straight { 1 } else { 4 };
	let mut intervals: Vec<(f64, f64)> = (0..minimum_intervals)
		.rev()
		.map(|i| (i as f64 / minimum_intervals as f64, (i + 1) as f64 / minimum_intervals as f64))
		.collect();

	let mut samples = vec![0.];
	while let Some((t0, t1)) = intervals.pop() {
		let middle = (t0 + t1) / 2.;
		let too_long = cubic.eval(t0).distance(cubic.eval(t1)) > spacing;
		let too_curved = !straight && {
			let (start, center, end) = (unit_tangent(cubic, t0), unit_tangent(cubic, middle), unit_tangent(cubic, t1));
			angle_between(start, center) + angle_between(center, end) > MAX_TURN_PER_INTERVAL
		};

		if (too_long || too_curved) && t1 - t0 > PARAM_EPSILON {
			intervals.push((middle, t1));
			intervals.push((t0, middle));
		} else {
			samples.push(t1);
		}
	}

	samples
}

/// Inserts a sample midway through every flagged interval longer than `minimum_length`. Returns whether any were added.
fn densify(arcs: &mut [HullArc], flagged: &FxHashSet<(usize, usize)>, minimum_length: f64) -> bool {
	let mut by_arc: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
	for &(arc, interval) in flagged {
		by_arc.entry(arc).or_default().push(interval);
	}

	let mut changed = false;
	for (arc_index, mut intervals) in by_arc {
		let arc = &mut arcs[arc_index];
		intervals.sort_unstable();
		intervals.dedup();

		// Rebuild the samples in one pass, following the start of each flagged interval with its midpoint
		let mut intervals = intervals.into_iter().peekable();
		let mut densified = Vec::with_capacity(arc.samples.len() + intervals.len());
		for (index, &t0) in arc.samples.iter().enumerate() {
			densified.push(t0);
			if intervals.next_if_eq(&index).is_some()
				&& let Some(&t1) = arc.samples.get(index + 1)
				&& arc.cubic.eval(t0).distance(arc.cubic.eval(t1)) > minimum_length
			{
				densified.push((t0 + t1) / 2.);
				changed = true;
			}
		}
		arc.samples = densified;
	}

	changed
}

impl SampleSet {
	/// Samples every arc and adds the standalone points, merging positions within `tolerance` into shared vertices.
	fn new(arcs: &[HullArc], points: &[Point], tolerance: f64) -> Self {
		let sample_count = arcs.iter().map(|arc| arc.samples.len()).sum::<usize>() + points.len();
		let mut vertices: Vec<SampleVertex> = Vec::with_capacity(sample_count);

		// Cells count from the corner of the geometry's bounds, which every sample lies within, so they stay small integers
		let (corner, _) = geometry_bounds(arcs, points);
		let cell_of = |position: Point| (((position.x - corner.x) / tolerance) as i64, ((position.y - corner.y) / tolerance) as i64);

		// Each cell names the latest vertex in it, and each vertex the one before it in its cell
		let mut latest_in_cell: FxHashMap<(i64, i64), u32> = FxHashMap::default();
		latest_in_cell.reserve(sample_count);
		let mut previous_in_cell: Vec<Option<u32>> = Vec::with_capacity(sample_count);

		let mut add = |position: Point, membership: Membership| -> u32 {
			let (x, y) = cell_of(position);
			let neighboring_cells = (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| (x + dx, y + dy)));
			let existing = neighboring_cells.into_iter().find_map(|cell| {
				let mut vertex = latest_in_cell.get(&cell).copied();
				while let Some(index) = vertex {
					if vertices[index as usize].position.distance(position) <= tolerance {
						return Some(index);
					}
					vertex = previous_in_cell[index as usize];
				}
				None
			});

			let index = existing.unwrap_or_else(|| {
				let index = vertices.len() as u32;
				vertices.push(SampleVertex {
					position,
					memberships: TinyVec::new(),
				});
				previous_in_cell.push(latest_in_cell.insert((x, y), index));
				index
			});
			vertices[index as usize].memberships.push(membership);
			index
		};

		let arc_vertices = arcs
			.iter()
			.enumerate()
			.map(|(arc_index, arc)| {
				let last = arc.samples.len() - 1;
				(arc.samples.iter().enumerate())
					.map(|(index, &t)| {
						// End samples use the exact control points so the anchor two arcs share merges exactly
						let position = match index {
							0 => arc.cubic.p0,
							index if index == last => arc.cubic.p3,
							_ => arc.cubic.eval(t),
						};
						add(
							position,
							Membership::Arc {
								arc: arc_index as u32,
								index: index as u32,
							},
						)
					})
					.collect()
			})
			.collect();

		for &point in points {
			add(point, Membership::Point);
		}

		Self { vertices, arc_vertices }
	}

	fn position(&self, vertex: u32) -> Point {
		self.vertices[vertex as usize].position
	}

	/// The position of a vertex for the exact geometric predicates.
	fn coordinate(&self, vertex: u32) -> robust::Coord<f64> {
		let Point { x, y } = self.position(vertex);
		robust::Coord { x, y }
	}

	/// Whether the ball may pivot around this vertex as a corner: a free point, or the end of an arc.
	fn is_corner(&self, vertex: u32) -> bool {
		self.vertices[vertex as usize].memberships.iter().any(|membership| match *membership {
			Membership::Arc { arc, index } => index == 0 || index as usize == self.arc_vertices[arc as usize].len() - 1,
			Membership::Point => true,
		})
	}

	/// The sample intervals on either side of a vertex along every arc it lies on.
	fn adjacent_intervals(&self, vertex: u32) -> impl Iterator<Item = (usize, usize)> + '_ {
		self.vertices[vertex as usize].memberships.iter().flat_map(move |membership| {
			let (arc, index) = match *membership {
				Membership::Arc { arc, index } => (arc as usize, index as usize),
				Membership::Point => (0, usize::MAX),
			};
			let interval_count = self.arc_vertices.get(arc).map_or(0, |vertices| vertices.len().saturating_sub(1));
			[index.wrapping_sub(1), index]
				.into_iter()
				.filter(move |&interval| interval < interval_count)
				.map(move |interval| (arc, interval))
		})
	}

	/// How far the true geometry near a vertex can stray from the samples, bounding the error of any flood decision it determines.
	fn sampling_error(&self, vertex: u32) -> f64 {
		let chord = |(arc, interval): (usize, usize)| self.position(self.arc_vertices[arc][interval]).distance(self.position(self.arc_vertices[arc][interval + 1]));
		0.6 * self.adjacent_intervals(vertex).map(chord).fold(0., f64::max)
	}
}

/// A Delaunay triangulation as half-edges, three per counterclockwise triangle with the triangle on each half-edge's left,
/// wrapped by ghost triangles that share the vertex at infinity so every half-edge has a twin.
struct Mesh {
	/// The vertex each half-edge starts from, or [`GHOST`] for the vertex at infinity.
	origin: Vec<u32>,
	/// The oppositely directed half-edge sharing each half-edge's edge.
	twin: Vec<u32>,
}

impl Mesh {
	/// Triangulates the sample vertices by inserting each into the triangle holding it and flipping edges until the
	/// triangulation is Delaunay again, all with exact predicates. Expected O(n log n) in the order [`insertion_order`] gives.
	/// Returns `None` when the vertices all lie on one line.
	fn new(samples: &SampleSet) -> Option<Self> {
		let order = insertion_order(samples);
		let orientation = |a: u32, b: u32, c: u32| robust::orient2d(samples.coordinate(a), samples.coordinate(b), samples.coordinate(c));

		// Seed with the first three vertices not on one line, wound counterclockwise
		let (&first, &second) = (order.first()?, order.get(1)?);
		let third = *order.iter().find(|&&vertex| orientation(first, second, vertex) != 0.)?;
		let seed = if orientation(first, second, third) > 0. { [first, second, third] } else { [first, third, second] };

		// A ghost triangle lies beyond each of the seed's edges, and each ghost shares an edge to infinity with the next
		let [a, b, c] = seed;
		let mut mesh = Self {
			origin: vec![a, b, c, b, a, GHOST, c, b, GHOST, a, c, GHOST],
			twin: vec![3, 6, 9, 0, 11, 7, 1, 5, 10, 2, 8, 4],
		};

		// Consecutive vertices lie near each other, so each walk sets out from where the last insertion went
		let mut walk_start = 0;
		for &vertex in &order {
			if seed.contains(&vertex) {
				continue;
			}
			let Some((triangle, on_edge)) = mesh.locate(samples, vertex, walk_start) else {
				log::warn!("Concave hull triangulation could not place a sample vertex");
				continue;
			};

			let new_edges = match on_edge {
				None => mesh.split_triangle(triangle, vertex),
				Some(half_edge) => mesh.split_edge(half_edge, vertex),
			};
			mesh.flip_to_delaunay(samples, new_edges);
			walk_start = triangle;
		}

		Some(mesh)
	}

	/// Flips edges, starting from the given half-edges, until no triangle's circumcircle holds another vertex.
	fn flip_to_delaunay(&mut self, samples: &SampleSet, mut unchecked: Vec<usize>) {
		while let Some(half_edge) = unchecked.pop() {
			let twin = self.twin[half_edge];
			if twin == GHOST {
				continue;
			}
			let twin = twin as usize;
			let (half_edge_next, half_edge_previous) = (Self::next(half_edge), Self::next(Self::next(half_edge)));
			let (twin_next, twin_previous) = (Self::next(twin), Self::next(Self::next(twin)));

			// The triangle (a, b, c) and its neighbor (b, a, d) across the edge from a to b
			let [a, b, c, d] = [half_edge, half_edge_next, half_edge_previous, twin_previous].map(|half_edge| self.origin[half_edge]);
			if !Self::breaks_delaunay(samples, [a, b, c, d]) {
				continue;
			}

			// Replace them with the triangles (c, a, d) and (d, b, c) sharing the edge between c and d, after which their outer
			// edges need checking again
			let outer_twins = [self.twin[half_edge_previous], self.twin[twin_next], self.twin[twin_previous], self.twin[half_edge_next]];
			[self.origin[half_edge], self.origin[half_edge_next], self.origin[half_edge_previous]] = [c, a, d];
			[self.origin[twin], self.origin[twin_next], self.origin[twin_previous]] = [d, b, c];
			let outer_edges = [half_edge, half_edge_next, twin, twin_next];
			for (outer_edge, outer_twin) in outer_edges.into_iter().zip(outer_twins) {
				self.link(outer_edge, outer_twin);
			}
			self.link(half_edge_previous, twin_previous as u32);
			unchecked.extend(outer_edges);
		}
	}

	/// Whether the edge from `a` to `b` between the triangles (a, b, c) and (b, a, d) breaks the Delaunay condition. A ghost
	/// triangle's circumcircle is the open half-plane beyond its hull edge, so an edge to infinity breaks it where the hull
	/// turns inward there, and a hull edge never does.
	fn breaks_delaunay(samples: &SampleSet, [a, b, c, d]: [u32; 4]) -> bool {
		let orientation = |a: u32, b: u32, c: u32| robust::orient2d(samples.coordinate(a), samples.coordinate(b), samples.coordinate(c));

		if a == GHOST {
			orientation(d, b, c) > 0.
		} else if b == GHOST {
			orientation(c, a, d) > 0.
		} else if c == GHOST || d == GHOST {
			false
		} else {
			robust::incircle(samples.coordinate(a), samples.coordinate(b), samples.coordinate(c), samples.coordinate(d)) > 0.
		}
	}

	/// Walks from a triangle toward a vertex's position, returning the triangle holding it and the half-edge it lies on, if
	/// any. A vertex outside the hull is held by the ghost triangle beyond the hull edge the walk leaves through. The walk is
	/// certain to arrive only in a Delaunay triangulation.
	fn locate(&self, samples: &SampleSet, vertex: u32, start: usize) -> Option<(usize, Option<usize>)> {
		let target = samples.coordinate(vertex);

		// The walk sets out from a real triangle, which is across a ghost's one finite edge
		let mut triangle = start;
		if self.is_ghost(triangle) {
			let finite = (3 * triangle..3 * triangle + 3).find(|&half_edge| self.origin[half_edge] != GHOST && self.destination(half_edge) != GHOST)?;
			triangle = Self::triangle(self.twin[finite] as usize);
		}

		for _ in 0..self.origin.len() {
			let mut on_edge = None;
			let mut beyond = None;
			for half_edge in 3 * triangle..3 * triangle + 3 {
				let side = robust::orient2d(samples.coordinate(self.origin[half_edge]), samples.coordinate(self.destination(half_edge)), target);
				if side < 0. {
					beyond = Some(half_edge);
					break;
				}
				if side == 0. {
					// Lying on two edges would mean coinciding with a vertex, which merging the samples rules out
					if on_edge.is_some() {
						return None;
					}
					on_edge = Some(half_edge);
				}
			}

			let Some(half_edge) = beyond else { return Some((triangle, on_edge)) };
			triangle = Self::triangle(self.twin[half_edge] as usize);
			if self.is_ghost(triangle) {
				return Some((triangle, None));
			}
		}

		None
	}

	/// Splits a triangle into three around a vertex inside it, returning their outer half-edges.
	fn split_triangle(&mut self, triangle: usize, vertex: u32) -> Vec<usize> {
		let [ab, bc, ca] = [3 * triangle, 3 * triangle + 1, 3 * triangle + 2];
		let [a, b, c] = [ab, bc, ca].map(|half_edge| self.origin[half_edge]);
		let [beyond_bc, beyond_ca] = [self.twin[bc], self.twin[ca]];

		// The triangle (a, b, c) becomes (a, b, p), joined by the new (b, c, p) and (c, a, p)
		let (first, second) = (self.origin.len(), self.origin.len() + 3);
		self.origin[ca] = vertex;
		self.origin.extend([b, c, vertex, c, a, vertex]);
		self.twin.extend([GHOST; 6]);
		self.link(first, beyond_bc);
		self.link(second, beyond_ca);
		self.link(bc, (first + 2) as u32);
		self.link(first + 1, (second + 2) as u32);
		self.link(second + 1, ca as u32);

		vec![ab, first, second]
	}

	/// Splits the edge a vertex lies on, along with the triangles on either side, returning their outer half-edges.
	fn split_edge(&mut self, ab: usize, vertex: u32) -> Vec<usize> {
		let (bc, ca) = (Self::next(ab), Self::next(Self::next(ab)));
		let [a, b, c] = [ab, bc, ca].map(|half_edge| self.origin[half_edge]);
		let (ba, beyond_bc) = (self.twin[ab], self.twin[bc]);

		// The triangle (a, b, c) becomes (a, p, c), joined by the new (p, b, c)
		let first = self.origin.len();
		self.origin[bc] = vertex;
		self.origin.extend([vertex, b, c]);
		self.twin.extend([GHOST; 3]);
		self.link(first + 1, beyond_bc);
		self.link(bc, (first + 2) as u32);
		let mut outer_edges = vec![ca, first + 1];
		if ba == GHOST {
			return outer_edges;
		}

		// Likewise across the edge, (b, a, d) becomes (b, p, d), joined by the new (p, a, d)
		let ba = ba as usize;
		let (ad, db) = (Self::next(ba), Self::next(Self::next(ba)));
		let (d, beyond_ad) = (self.origin[db], self.twin[ad]);
		let second = self.origin.len();
		self.origin[ad] = vertex;
		self.origin.extend([vertex, a, d]);
		self.twin.extend([GHOST; 3]);
		self.link(second + 1, beyond_ad);
		self.link(ad, (second + 2) as u32);
		self.link(ab, second as u32);
		self.link(first, ba as u32);

		outer_edges.extend([db, second + 1]);
		outer_edges
	}

	/// Makes two half-edges each other's twin, or marks one as bordering the outside.
	fn link(&mut self, half_edge: usize, twin: u32) {
		self.twin[half_edge] = twin;
		if twin != GHOST {
			self.twin[twin as usize] = half_edge as u32;
		}
	}

	fn next(half_edge: usize) -> usize {
		half_edge - half_edge % 3 + (half_edge + 1) % 3
	}

	fn triangle(half_edge: usize) -> usize {
		half_edge / 3
	}

	fn destination(&self, half_edge: usize) -> u32 {
		self.origin[Self::next(half_edge)]
	}

	fn is_ghost(&self, triangle: usize) -> bool {
		self.origin[3 * triangle..3 * triangle + 3].contains(&GHOST)
	}
}

/// The order to insert the sample vertices in: rounds doubling in size drawn at random, which keeps the expected flips per
/// insertion constant for any input, each sorted along a Hilbert curve so every walk from the last insertion is short.
fn insertion_order(samples: &SampleSet) -> Vec<u32> {
	let count = samples.vertices.len();
	let mut order: Vec<u32> = (0..count as u32).collect();

	// Shuffled with a fixed seed, so the same geometry always gives the same hull
	let mut state = 0x853C_49E6_748F_EA9B_u64;
	for i in (1..count).rev() {
		state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
		order.swap(i, ((state >> 33) % (i as u64 + 1)) as usize);
	}

	// Each vertex's place along a Hilbert curve through a grid over the bounding box
	let positions = samples.vertices.iter().map(|vertex| vertex.position);
	let (min, max) = positions
		.clone()
		.fold((Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY)), |(min, max), position| {
			(Point::new(min.x.min(position.x), min.y.min(position.y)), Point::new(max.x.max(position.x), max.y.max(position.y)))
		});
	let cells_per_unit = HILBERT_GRID_SIDE as f64 / (max.x - min.x).max(max.y - min.y).max(f64::MIN_POSITIVE);
	let cell = |value: f64, min: f64| (((value - min) * cells_per_unit) as u32).min(HILBERT_GRID_SIDE - 1);
	let keys: Vec<u64> = positions.map(|position| hilbert_index(cell(position.x, min.x), cell(position.y, min.y))).collect();

	let mut round_end = count;
	while round_end > 1 {
		let round_start = round_end / 2;
		order[round_start..round_end].sort_unstable_by_key(|&vertex| keys[vertex as usize]);
		round_end = round_start;
	}

	order
}

/// The position of a grid cell along the Hilbert curve that visits every cell, each one next to the last.
fn hilbert_index(mut x: u32, mut y: u32) -> u64 {
	let mut index = 0;
	let mut quadrant_size = HILBERT_GRID_SIDE / 2;

	while quadrant_size > 0 {
		let (right, top) = (u32::from((x & quadrant_size) > 0), u32::from((y & quadrant_size) > 0));
		index += u64::from(quadrant_size) * u64::from(quadrant_size) * u64::from((3 * right) ^ top);

		// Turn the quadrant so the curve through it starts where the curve through the previous one ended
		if top == 0 {
			if right == 1 {
				(x, y) = (HILBERT_GRID_SIDE - 1 - x, HILBERT_GRID_SIDE - 1 - y);
			}
			(x, y) = (y, x);
		}
		quadrant_size /= 2;
	}

	index
}

/// The room a ball has to cross a half-edge between its two triangles, and the vertices with sampling error that bounds
/// how wrong that room can be.
#[derive(Clone, Copy)]
struct Clearance {
	room: f64,
	determined_by: [u32; 3],
}

/// The radius of the circle through a triangle's corners, infinite for collinear corners.
fn circumradius(a: Point, b: Point, c: Point) -> f64 {
	a.distance(b) * b.distance(c) * c.distance(a) / (2. * (b - a).cross(c - a).abs())
}

/// The clearance along the Voronoi edge dual to every half-edge. Free space retracts onto the Voronoi diagram, so a ball
/// can cross between two triangles exactly when it fits everywhere along that Voronoi edge. The tightest point is either
/// the edge's midpoint (where the Voronoi edge crosses it) or the nearer of its circumcenters.
fn clearances(mesh: &Mesh, samples: &SampleSet) -> Vec<Clearance> {
	let corners = |triangle: usize| [0, 1, 2].map(|j| mesh.origin[3 * triangle + j]);
	let circumradii: Vec<f64> = (0..mesh.origin.len() / 3)
		.map(|triangle| {
			if mesh.is_ghost(triangle) {
				return f64::INFINITY;
			}
			let [a, b, c] = corners(triangle).map(|vertex| samples.position(vertex));
			circumradius(a, b, c)
		})
		.collect();

	// Whether the circumcenter of the triangle left of a half-edge lies beyond it, which is when the angle opposite the
	// half-edge is obtuse. Unlike the circumcenter itself, this stays reliable for the slivers along straight geometry.
	let center_beyond = |half_edge: usize| {
		let apex = samples.position(mesh.origin[Mesh::next(Mesh::next(half_edge))]);
		(samples.position(mesh.origin[half_edge]) - apex).dot(samples.position(mesh.destination(half_edge)) - apex) < 0.
	};

	(0..mesh.origin.len())
		.map(|half_edge| {
			let twin = mesh.twin[half_edge] as usize;
			let (left, right) = (Mesh::triangle(half_edge), Mesh::triangle(twin));
			if mesh.is_ghost(left) && mesh.is_ghost(right) {
				return Clearance {
					room: f64::INFINITY,
					determined_by: [GHOST; 3],
				};
			}

			let (start, end) = (mesh.origin[half_edge], mesh.destination(half_edge));
			let through_edge = Clearance {
				room: samples.position(start).distance(samples.position(end)) / 2.,
				determined_by: [start, end, GHOST],
			};
			let at_center = |triangle: usize| Clearance {
				room: circumradii[triangle],
				determined_by: corners(triangle),
			};

			match (mesh.is_ghost(left), mesh.is_ghost(right)) {
				(false, false) if center_beyond(half_edge) != center_beyond(twin) => {
					if circumradii[left] < circumradii[right] {
						at_center(left)
					} else {
						at_center(right)
					}
				}
				// From outside, the ball arrives along the ray from the circumcenter away from the edge
				(false, true) if center_beyond(half_edge) => at_center(left),
				(true, false) if center_beyond(twin) => at_center(right),
				_ => through_edge,
			}
		})
		.collect()
}

/// Which side of a span or stretch a candidate contact chooses.
#[derive(Clone, Copy)]
enum SpanEnd {
	/// A fixed corner the ball touches.
	Point(Point),
	/// A point on an arc, walked in `direction` (positive is forward), solved from the parameter `seed`.
	Arc { arc: usize, direction: f64, seed: f64 },
}

/// The exact contact parameters of a span, where it arrives, and the center of the ball touching both ends.
#[derive(Clone, Copy)]
struct SpanSolution {
	departure_t: f64,
	arrival_t: f64,
	arrival: Point,
	center: Point,
}

/// A maximal piece of the boundary between spans.
#[derive(Clone, Copy)]
struct Piece {
	kind: PieceKind,
	/// Sample vertices where the boundary arrives at and leaves this piece.
	arrival_vertex: u32,
	departure_vertex: u32,
}

#[derive(Clone, Copy)]
enum PieceKind {
	/// The boundary follows an arc from `t_in` to `t_out`, walking it forward when `direction` is positive. Equal parameters
	/// mark a single touch, with its extent found by refining the spans on either side.
	Stretch { arc: usize, t_in: f64, t_out: f64, direction: f64 },
	/// A point the ball pivots around: a sharp anchor, an open path's end, or a free point.
	Corner { position: Point },
}

/// Classification of one edge of a boundary cycle.
#[derive(Clone, Copy)]
enum CycleEdge {
	/// The edge joins neighboring samples of an arc, so the boundary follows that arc from sample `from` to sample `to`.
	Along { arc: usize, from: usize, to: usize },
	/// The boundary jumps between separate places on the geometry, where the ball touches both.
	Span,
}

/// How a span ends on one side, chosen among the candidate contacts.
#[derive(Clone, Copy)]
enum EndChoice {
	/// On the piece's own arc.
	Own,
	/// On a neighboring arc past the piece's end vertex (at parameter `vertex_t` on it), which the boundary then follows
	/// between that vertex and the contact.
	Neighbor { arc: usize, vertex_t: f64, direction: f64 },
	/// At the corner the piece's end vertex forms.
	Corner,
}

/// Arc intervals and points bucketed in a grid, for checking that a ball holds no geometry.
struct ObstacleGrid {
	cell_size: f64,
	/// The corner of the geometry's bounds, which cells count from.
	corner: Point,
	obstacles: Vec<Obstacle>,
	/// The obstacles overlapping each cell, by index.
	cells: FxHashMap<(i64, i64), Vec<u32>>,
}

/// A piece of geometry, with a box holding it and the lowest cell that box overlaps.
struct Obstacle {
	kind: ObstacleKind,
	bounds: kurbo::Rect,
	first_cell: (i64, i64),
}

#[derive(Clone, Copy)]
enum ObstacleKind {
	Interval { arc: usize, interval: usize },
	Point(Point),
}

impl ObstacleGrid {
	fn new(arcs: &[HullArc], points: &[Point], cell_size: f64) -> Self {
		let (corner, _) = geometry_bounds(arcs, points);
		let mut grid = Self {
			cell_size,
			corner,
			obstacles: Vec::new(),
			cells: FxHashMap::default(),
		};

		let intervals = arcs.iter().enumerate().flat_map(|(arc_index, arc)| {
			arc.samples.windows(2).enumerate().map(move |(interval, window)| {
				let bounds = kurbo::ParamCurveExtrema::bounding_box(&arc.cubic.subsegment(window[0]..window[1]));
				(ObstacleKind::Interval { arc: arc_index, interval }, bounds)
			})
		});
		let points = points.iter().map(|&point| (ObstacleKind::Point(point), kurbo::Rect::from_points(point, point)));

		for (kind, bounds) in intervals.chain(points) {
			let (min, max) = (grid.cell_of(Point::new(bounds.x0, bounds.y0)), grid.cell_of(Point::new(bounds.x1, bounds.y1)));
			let index = grid.obstacles.len() as u32;
			grid.obstacles.push(Obstacle { kind, bounds, first_cell: min });

			for x in min.0..=max.0 {
				for y in min.1..=max.1 {
					grid.cells.entry((x, y)).or_default().push(index);
				}
			}
		}

		grid
	}

	fn cell_of(&self, position: Point) -> (i64, i64) {
		(
			((position.x - self.corner.x) / self.cell_size).floor() as i64,
			((position.y - self.corner.y) / self.cell_size).floor() as i64,
		)
	}

	/// The distance from `center` to the nearest geometry within `reach`, with that geometry's arc interval if it lies on one.
	/// Each obstacle is measured once, in the first of the searched cells it overlaps, and only if its box lies nearer than
	/// the nearest geometry found so far.
	fn nearest(&self, center: Point, reach: f64, arcs: &[HullArc]) -> (f64, Option<(usize, usize)>) {
		let (min, max) = (self.cell_of(center - Vec2::splat(reach)), self.cell_of(center + Vec2::splat(reach)));
		let mut nearest = (reach, None);

		for x in min.0..=max.0 {
			for y in min.1..=max.1 {
				for &index in self.cells.get(&(x, y)).into_iter().flatten() {
					let obstacle = &self.obstacles[index as usize];
					let first_searched_cell = (obstacle.first_cell.0.max(min.0), obstacle.first_cell.1.max(min.1));
					let box_offset = Vec2::new(
						(obstacle.bounds.x0 - center.x).max(center.x - obstacle.bounds.x1),
						(obstacle.bounds.y0 - center.y).max(center.y - obstacle.bounds.y1),
					);
					if first_searched_cell != (x, y) || Vec2::new(box_offset.x.max(0.), box_offset.y.max(0.)).hypot() >= nearest.0 {
						continue;
					}

					let (distance, interval) = match obstacle.kind {
						ObstacleKind::Point(point) => (point.distance(center), None),
						ObstacleKind::Interval { arc, interval } => {
							let samples = &arcs[arc].samples;
							let piece = arcs[arc].cubic.subsegment(samples[interval]..samples[interval + 1]);
							(piece.nearest(center, 1e-12).distance_sq.sqrt(), Some((arc, interval)))
						}
					};
					if distance < nearest.0 {
						nearest = (distance, interval);
					}
				}
			}
		}

		nearest
	}
}

/// One round of stages 3 to 5 over the current samples, returning the hull and the sample intervals to densify before
/// the next round (empty when every decision was clear).
fn trace_hull(arcs: &[HullArc], context: &HullContext) -> (BezPath, FxHashSet<(usize, usize)>) {
	let radius = context.radius;
	let samples = SampleSet::new(arcs, context.points, context.distance_epsilon);
	let mut flagged = FxHashSet::default();

	let Some(mesh) = Mesh::new(&samples) else {
		return (collinear_hull(&samples, radius), flagged);
	};

	// Stage 3: flood in from the outside across every edge the ball fits through
	let clearances = clearances(&mesh, &samples);
	let triangle_count = mesh.origin.len() / 3;

	let mut reachable = vec![false; triangle_count];
	let mut queue: VecDeque<usize> = (0..triangle_count).filter(|&triangle| mesh.is_ghost(triangle)).collect();
	for &ghost in &queue {
		reachable[ghost] = true;
	}
	while let Some(triangle) = queue.pop_front() {
		let half_edges = 3 * triangle..3 * triangle + 3;
		for (&twin, clearance) in mesh.twin[half_edges.clone()].iter().zip(&clearances[half_edges]) {
			let neighbor = Mesh::triangle(twin as usize);
			if !reachable[neighbor] && clearance.room >= radius {
				reachable[neighbor] = true;
				queue.push_back(neighbor);
			}
		}
	}

	// Sampling only overstates room, so a reached passage the ball barely fits through may truly be closed
	for (half_edge, clearance) in clearances.iter().enumerate() {
		if clearance.room < radius || clearance.room.is_infinite() || mesh.twin[half_edge] < half_edge as u32 || !reachable[Mesh::triangle(half_edge)] {
			continue;
		}
		let determining = clearance.determined_by.into_iter().filter(|&vertex| vertex != GHOST);
		if clearance.room - determining.clone().map(|vertex| samples.sampling_error(vertex)).fold(0., f64::max) < radius {
			flagged.extend(determining.flat_map(|vertex| samples.adjacent_intervals(vertex)));
		}
	}

	// The boundary is every edge the flood reaches but cannot cross, traced with the reachable side on the left
	let is_boundary = |half_edge: usize| reachable[Mesh::triangle(half_edge)] && clearances[half_edge].room < radius;
	let mut visited = vec![false; mesh.origin.len()];
	let mut cycles = Vec::new();
	for start in 0..mesh.origin.len() {
		if visited[start] || !is_boundary(start) {
			continue;
		}

		let mut cycle = Vec::new();
		let mut half_edge = start;
		while !visited[half_edge] {
			visited[half_edge] = true;
			cycle.push(mesh.origin[half_edge]);

			// Rotate clockwise around the edge's end, through triangles the ball reaches, to the next edge it cannot cross
			let mut next = Mesh::next(half_edge);
			for _ in 0..mesh.origin.len() {
				if is_boundary(next) {
					break;
				}
				next = Mesh::next(mesh.twin[next] as usize);
			}
			half_edge = next;
		}

		// Reversed, the hull lies on the left
		cycle.reverse();
		cycles.push(cycle);
	}

	// A vertex the ball reaches all the way around is a lone point of the hull
	let mut touches_reachable = vec![false; samples.vertices.len()];
	let mut on_boundary = vec![false; samples.vertices.len()];
	for half_edge in 0..mesh.origin.len() {
		let origin = mesh.origin[half_edge];
		if origin != GHOST && reachable[Mesh::triangle(half_edge)] {
			touches_reachable[origin as usize] = true;
			on_boundary[origin as usize] |= clearances[half_edge].room < radius;
		}
	}

	// Stages 4 and 5: refine each cycle's spans to their exact contacts and emit it
	let obstacles = ObstacleGrid::new(arcs, context.points, radius);

	// An edge follows the first arc it joins neighboring samples of, and otherwise spans a gap
	let classify = |from: u32, to: u32| {
		let along = samples.vertices[from as usize].memberships.iter().find_map(|membership| {
			let Membership::Arc { arc, index } = *membership else { return None };
			let (arc, index) = (arc as usize, index as usize);
			let vertices = &samples.arc_vertices[arc];

			if vertices.get(index + 1) == Some(&to) {
				Some(CycleEdge::Along { arc, from: index, to: index + 1 })
			} else if index > 0 && vertices[index - 1] == to {
				Some(CycleEdge::Along { arc, from: index, to: index - 1 })
			} else {
				None
			}
		});
		along.unwrap_or(CycleEdge::Span)
	};

	let mut hull = BezPath::new();
	for cycle in &cycles {
		let edges: Vec<CycleEdge> = (0..cycle.len()).map(|i| classify(cycle[i], cycle[(i + 1) % cycle.len()])).collect();

		let (pieces, spanned) = extract_pieces(cycle, &edges, &samples, arcs);
		let (pieces, links) = refine_cycle(pieces, spanned, &samples, arcs, &obstacles, context, &mut flagged);
		emit_cycle(&mut hull, &pieces, &links, arcs, context);
	}

	for (vertex, sample) in samples.vertices.iter().enumerate() {
		if touches_reachable[vertex] && !on_boundary[vertex] {
			hull.move_to(sample.position);
		}
	}

	(hull, flagged)
}

/// The hull of geometry lying all along one line, where the triangulation has no triangles: the stretches of line the
/// ball cannot pass between, and the points standing alone.
fn collinear_hull(samples: &SampleSet, radius: f64) -> BezPath {
	let mut hull = BezPath::new();
	let Some(first) = samples.vertices.first() else { return hull };

	let direction = samples
		.vertices
		.iter()
		.map(|vertex| vertex.position - first.position)
		.max_by(|a, b| a.hypot2().total_cmp(&b.hypot2()))
		.unwrap_or_default();
	let mut order: Vec<Point> = samples.vertices.iter().map(|vertex| vertex.position).collect();
	order.sort_by(|a, b| (*a - first.position).dot(direction).total_cmp(&(*b - first.position).dot(direction)));

	let mut run_start = 0;
	for i in 1..=order.len() {
		if i < order.len() && order[i].distance(order[i - 1]) < 2. * radius {
			continue;
		}
		hull.move_to(order[run_start]);
		if i - 1 > run_start {
			hull.line_to(order[i - 1]);
		}
		run_start = i;
	}

	hull
}

/// Groups a boundary cycle's edges into pieces: maximal runs along one arc, and the corners or single touches standing
/// alone between spans. Returns the pieces with whether each is joined to the next by a span rather than a shared vertex.
fn extract_pieces(cycle: &[u32], edges: &[CycleEdge], samples: &SampleSet, arcs: &[HullArc]) -> (Vec<Piece>, Vec<bool>) {
	let n = cycle.len();
	let continues = |previous: CycleEdge, next: CycleEdge| match (previous, next) {
		(
			CycleEdge::Along { arc, from, to },
			CycleEdge::Along {
				arc: next_arc,
				from: next_from,
				to: next_to,
			},
		) => arc == next_arc && to == next_from && (to as isize - from as isize) == (next_to as isize - next_from as isize),
		_ => false,
	};

	// Start right after a span, or else at an edge that does not continue its predecessor, so no piece straddles the seam
	let start = (0..n)
		.find(|&i| matches!(edges[(i + n - 1) % n], CycleEdge::Span))
		.or_else(|| (0..n).find(|&i| !continues(edges[(i + n - 1) % n], edges[i])))
		.unwrap_or(0);

	// A vertex standing alone between spans is a single touch of the arc it lies inside, or else a corner
	let standalone = |i: usize| {
		let vertex = cycle[i];
		let interior = samples.vertices[vertex as usize].memberships.iter().find_map(|membership| match *membership {
			Membership::Arc { arc, index } if index > 0 && (index as usize) < samples.arc_vertices[arc as usize].len() - 1 => Some((arc as usize, index as usize)),
			_ => None,
		});
		let kind = match interior {
			Some((arc, index)) => {
				let t = arcs[arc].samples[index];
				let (previous, next) = (samples.position(cycle[(i + n - 1) % n]), samples.position(cycle[(i + 1) % n]));
				let heading = next - previous;
				let direction = if unit_tangent(&arcs[arc].cubic, t).dot(heading) >= 0. { 1. } else { -1. };
				PieceKind::Stretch { arc, t_in: t, t_out: t, direction }
			}
			None => PieceKind::Corner { position: samples.position(vertex) },
		};
		Piece {
			kind,
			arrival_vertex: vertex,
			departure_vertex: vertex,
		}
	};

	let mut pieces = Vec::new();
	let mut spanned_into = Vec::new();
	let mut arriving_by_span = matches!(edges[(start + n - 1) % n], CycleEdge::Span);
	let mut run: Option<Piece> = None;

	for step in 0..n {
		let i = (start + step) % n;
		match edges[i] {
			CycleEdge::Along { arc, from, to } => {
				let t_to = arcs[arc].samples[to];
				let extended = match &mut run {
					Some(Piece {
						kind: PieceKind::Stretch { t_out, .. },
						departure_vertex,
						..
					}) if continues(edges[(i + n - 1) % n], edges[i]) => {
						*t_out = t_to;
						*departure_vertex = cycle[(i + 1) % n];
						true
					}
					_ => false,
				};

				if !extended {
					if let Some(finished) = run.take() {
						pieces.push(finished);
						spanned_into.push(arriving_by_span);
						arriving_by_span = false;
					}
					run = Some(Piece {
						kind: PieceKind::Stretch {
							arc,
							t_in: arcs[arc].samples[from],
							t_out: t_to,
							direction: if to > from { 1. } else { -1. },
						},
						arrival_vertex: cycle[i],
						departure_vertex: cycle[(i + 1) % n],
					});
				}
			}
			CycleEdge::Span => {
				pieces.push(run.take().unwrap_or_else(|| standalone(i)));
				spanned_into.push(arriving_by_span);
				arriving_by_span = true;
			}
		}
	}
	if let Some(finished) = run.take() {
		pieces.push(finished);
		spanned_into.push(arriving_by_span);
	}

	// Each piece's link to the next is a span if the next piece is reached by one
	let count = pieces.len();
	let spanned = (0..count).map(|k| spanned_into[(k + 1) % count]).collect();
	(pieces, spanned)
}

/// The point where the boundary arrives at a piece, or leaves it when `departure` is set.
fn piece_point(piece: &Piece, arcs: &[HullArc], departure: bool) -> Point {
	match piece.kind {
		PieceKind::Stretch { arc, t_in, t_out, .. } => arcs[arc].cubic.eval(if departure { t_out } else { t_in }),
		PieceKind::Corner { position } => position,
	}
}

/// The unit tangent where the boundary arrives at or leaves a stretch, in its walking direction.
fn walking_tangent(arc: &HullArc, t: f64, direction: f64) -> Vec2 {
	unit_tangent(&arc.cubic, t) * direction
}

/// The candidate contacts for one end of a span, best first: the piece's own arc, arcs continuing past an end vertex the
/// piece shares with them, and the corner that vertex forms.
fn end_candidates(piece: &Piece, departure: bool, seed_offset: f64, samples: &SampleSet, arcs: &[HullArc]) -> Vec<(SpanEnd, EndChoice)> {
	let mut candidates = Vec::new();
	let vertex = if departure { piece.departure_vertex } else { piece.arrival_vertex };

	let own_arc = match piece.kind {
		PieceKind::Stretch { arc, t_in, t_out, direction } => {
			// A span across a concave corner seeds back from the corner, where both ends would otherwise coincide
			let seed = if departure { t_out - direction * seed_offset } else { t_in + direction * seed_offset };
			candidates.push((
				SpanEnd::Arc {
					arc,
					direction,
					seed: seed.clamp(0., 1.),
				},
				EndChoice::Own,
			));
			Some(arc)
		}
		PieceKind::Corner { .. } => None,
	};

	if samples.is_corner(vertex) {
		let corner = (SpanEnd::Point(samples.position(vertex)), EndChoice::Corner);
		if own_arc.is_none() {
			candidates.push(corner);
		}

		for membership in &samples.vertices[vertex as usize].memberships {
			let Membership::Arc { arc, index } = *membership else { continue };
			let (arc, last) = (arc as usize, arcs[arc as usize].samples.len() - 1);
			if Some(arc) == own_arc || (index != 0 && index as usize != last) {
				continue;
			}

			// The boundary follows the neighbor away from the vertex after leaving the piece, or toward it before arriving
			let starts_here = index == 0;
			let direction = if starts_here == departure { 1. } else { -1. };
			let vertex_t = if starts_here { 0. } else { 1. };
			let half_interval = if starts_here { arcs[arc].samples[1] / 2. } else { (1. - arcs[arc].samples[last - 1]) / 2. };
			let seed = if starts_here { half_interval } else { 1. - half_interval };
			candidates.push((SpanEnd::Arc { arc, direction, seed }, EndChoice::Neighbor { arc, vertex_t, direction }));
		}

		if own_arc.is_some() {
			candidates.push(corner);
		}
	}

	candidates
}

/// Solves for the contacts of a ball of the given radius touching both ends of a span, with the ball on the boundary's
/// right. Each arc end satisfies the tangent-chord condition of a circle through both contacts tangent to that arc, which
/// for a flat ball reduces to the span being tangent there.
fn solve_span(departure: SpanEnd, arrival: SpanEnd, arcs: &[HullArc], radius: f64) -> Option<SpanSolution> {
	let half_curvature = 0.5 / radius;
	let state = |end: SpanEnd, t: f64| match end {
		SpanEnd::Point(position) => (position, None),
		SpanEnd::Arc { arc, direction, .. } => (arcs[arc].cubic.eval(t), Some(walking_tangent(&arcs[arc], t, direction))),
	};
	let residual = |t: [f64; 2]| -> Option<[f64; 2]> {
		let ((start, start_tangent), (end, end_tangent)) = (state(departure, t[0]), state(arrival, t[1]));
		let chord = end - start;
		let length = chord.hypot();
		if length.is_nan() || length <= 0. {
			return None;
		}
		let chord_direction = chord / length;

		Some([
			start_tangent.map_or(0., |tangent| tangent.cross(chord_direction) + length * half_curvature),
			end_tangent.map_or(0., |tangent| chord_direction.cross(tangent) + length * half_curvature),
		])
	};
	let size = |residual: [f64; 2]| residual[0].abs().max(residual[1].abs());

	// Damped Newton iteration over whichever ends are free to slide, with a numerical Jacobian
	let free = [matches!(departure, SpanEnd::Arc { .. }), matches!(arrival, SpanEnd::Arc { .. })];
	let seed = |end: SpanEnd| if let SpanEnd::Arc { seed, .. } = end { seed } else { 0. };
	let mut t = [seed(departure), seed(arrival)];
	let mut current = residual(t)?;

	for _ in 0..MAX_SPAN_ITERATIONS {
		if size(current) < SPAN_TOLERANCE * 1e-3 {
			break;
		}

		let step_size = 1e-7;
		let mut jacobian = [[0.; 2]; 2];
		for column in 0..2 {
			if !free[column] {
				jacobian[column][column] = 1.;
				continue;
			}
			let (mut forward, mut backward) = (t, t);
			forward[column] += step_size;
			backward[column] -= step_size;
			let (forward, backward) = (residual(forward)?, residual(backward)?);
			for row in 0..2 {
				jacobian[row][column] = (forward[row] - backward[row]) / (2. * step_size);
			}
		}

		let determinant = jacobian[0][0] * jacobian[1][1] - jacobian[0][1] * jacobian[1][0];
		if determinant.is_nan() || determinant.abs() <= f64::MIN_POSITIVE {
			break;
		}
		let step = [
			(jacobian[1][1] * current[0] - jacobian[0][1] * current[1]) / determinant,
			(jacobian[0][0] * current[1] - jacobian[1][0] * current[0]) / determinant,
		];

		// Limit the step, then halve it until the residual shrinks
		let mut damping = (0.2 / step[0].abs().max(step[1].abs())).min(1.);
		let mut improved = None;
		for _ in 0..16 {
			let candidate = [t[0] - step[0] * damping, t[1] - step[1] * damping];
			if let Some(candidate_residual) = residual(candidate)
				&& size(candidate_residual) < size(current)
			{
				improved = Some((candidate, candidate_residual));
				break;
			}
			damping /= 2.;
		}
		let Some((next, next_residual)) = improved else { break };
		(t, current) = (next, next_residual);
	}

	let range_tolerance = 1e-9;
	if size(current) > SPAN_TOLERANCE || (0..2).any(|i| free[i] && !(-range_tolerance..=1. + range_tolerance).contains(&t[i])) {
		return None;
	}
	let t = t.map(|t| t.clamp(0., 1.));

	// The ball's center sits a radius to the right of each arc contact. The tangent-chord conditions also admit the ball
	// mirrored across the span, and then the two contacts disagree about where its center is.
	let right = |tangent: Vec2| Vec2::new(tangent.y, -tangent.x);
	let ((start, start_tangent), (end, end_tangent)) = (state(departure, t[0]), state(arrival, t[1]));
	let center = match (start_tangent, end_tangent) {
		(Some(tangent), _) => start + right(tangent) * radius,
		(None, Some(tangent)) => end + right(tangent) * radius,
		(None, None) => {
			let chord = end - start;
			let half_length = chord.hypot() / 2.;
			if !(half_length > 0. && half_length < radius) {
				return None;
			}
			start.midpoint(end) + right(chord.normalize()) * (radius * radius - half_length * half_length).sqrt()
		}
	};
	if let (Some(_), Some(tangent)) = (start_tangent, end_tangent)
		&& center.distance(end + right(tangent) * radius) > radius * 1e-6
	{
		return None;
	}

	// The ball must sit outside the span rather than reaching back around more than half its circumference
	if (center - start).dot(right(end - start)) <= 0. {
		return None;
	}

	Some(SpanSolution {
		departure_t: t[0],
		arrival_t: t[1],
		arrival: end,
		center,
	})
}

/// Refines every span in a cycle to its exact contacts, choosing among each end's candidate contacts the best one with a
/// ball that holds no geometry, and rebuilds the pieces around the chosen contacts. Spans with no valid choice, contacts that
/// cross over, and stretches curving tighter than the ball flag their sample intervals for densifying.
fn refine_cycle(
	pieces: Vec<Piece>,
	mut spanned: Vec<bool>,
	samples: &SampleSet,
	arcs: &[HullArc],
	obstacles: &ObstacleGrid,
	context: &HullContext,
	flagged: &mut FxHashSet<(usize, usize)>,
) -> (Vec<Piece>, Vec<Option<SpanSolution>>) {
	let radius = context.radius;
	let count = pieces.len();
	let clear_tolerance = radius * 1e-7 + context.distance_epsilon * 10.;

	// A joint where the boundary turns right is a concave corner the ball cannot reach into, so it is spanned too, seeded
	// at the expected contacts a radius-sized distance back from the corner on each side
	let mut joint_seed_offsets = vec![[0.; 2]; count];
	for k in 0..count {
		let (piece, next) = (&pieces[k], &pieces[(k + 1) % count]);
		let (
			PieceKind::Stretch { arc, t_out, direction, .. },
			PieceKind::Stretch {
				arc: next_arc,
				t_in: next_t_in,
				direction: next_direction,
				..
			},
		) = (piece.kind, next.kind)
		else {
			continue;
		};
		if spanned[k] {
			continue;
		}

		let (leaving, entering) = (walking_tangent(&arcs[arc], t_out, direction), walking_tangent(&arcs[next_arc], next_t_in, next_direction));
		let turn = leaving.cross(entering).atan2(leaving.dot(entering));
		let contact_distance = radius * (turn.abs() / 2.).tan();
		if turn < 0. && turn > -(PI - 1e-6) && contact_distance > context.distance_epsilon * 1e3 {
			spanned[k] = true;
			let offset = |arc: &HullArc, t: f64| (contact_distance / arc.cubic.deriv().eval(t).to_vec2().hypot().max(f64::MIN_POSITIVE)).min(0.5);
			joint_seed_offsets[k] = [offset(&arcs[arc], t_out), offset(&arcs[next_arc], next_t_in)];
		}
	}

	// Solve each span over its candidate contacts, preferring the earliest candidates with an empty ball
	let mut resolved: Vec<Option<(EndChoice, EndChoice, SpanSolution)>> = vec![None; count];
	for k in 0..count {
		if !spanned[k] {
			continue;
		}
		let (piece, next) = (&pieces[k], &pieces[(k + 1) % count]);
		let departures = end_candidates(piece, true, joint_seed_offsets[k][0], samples, arcs);
		let arrivals = end_candidates(next, false, joint_seed_offsets[k][1], samples, arcs);

		let mut pairs: Vec<(usize, usize)> = (0..departures.len()).flat_map(|i| (0..arrivals.len()).map(move |j| (i, j))).collect();
		pairs.sort_by_key(|&(i, j)| (i + j, i));

		let mut best: Option<(f64, EndChoice, EndChoice, SpanSolution)> = None;
		for (i, j) in pairs {
			let Some(solution) = solve_span(departures[i].0, arrivals[j].0, arcs, radius) else { continue };
			if !within_reach(piece, departures[i].1, solution.departure_t, arcs) || !within_reach(next, arrivals[j].1, solution.arrival_t, arcs) {
				continue;
			}
			let (clearance, _) = obstacles.nearest(solution.center, radius, arcs);
			if best.as_ref().is_none_or(|best| clearance > best.0) {
				best = Some((clearance, departures[i].1, arrivals[j].1, solution));
			}
			if clearance >= radius - clear_tolerance {
				break;
			}
		}

		match best {
			Some((clearance, departure, arrival, solution)) => {
				if clearance < radius - clear_tolerance {
					let (_, invader) = obstacles.nearest(solution.center, radius, arcs);
					flagged.extend(invader);
					flagged.extend(samples.adjacent_intervals(piece.departure_vertex).chain(samples.adjacent_intervals(next.arrival_vertex)));
				}
				resolved[k] = Some((departure, arrival, solution));
			}
			// Without any solution, the span stays a chord between the sampled positions
			None => {
				flagged.extend(samples.adjacent_intervals(piece.departure_vertex).chain(samples.adjacent_intervals(next.arrival_vertex)));
				let (departure, arrival) = (piece_point(piece, arcs, true), piece_point(next, arcs, false));
				let solution = SpanSolution {
					departure_t: f64::NAN,
					arrival_t: f64::NAN,
					arrival,
					center: departure.midpoint(arrival),
				};
				resolved[k] = Some((EndChoice::Corner, EndChoice::Corner, solution));
			}
		}
	}

	// Rebuild the pieces around the chosen contacts, inserting the stretches of neighboring arcs that some contacts moved onto
	let mut refined = Vec::with_capacity(count);
	let mut links = Vec::with_capacity(count);
	for k in 0..count {
		let mut piece = pieces[k];
		let (arriving, leaving) = (resolved[(k + count - 1) % count], resolved[k]);

		let mut before = None;
		if let Some((_, choice, solution)) = arriving {
			match (choice, &mut piece.kind) {
				(EndChoice::Own, PieceKind::Stretch { t_in, .. }) => *t_in = snap(solution.arrival_t),
				(EndChoice::Neighbor { arc, vertex_t, direction }, _) => {
					before = Some(Piece {
						kind: PieceKind::Stretch {
							arc,
							t_in: snap(solution.arrival_t),
							t_out: vertex_t,
							direction,
						},
						arrival_vertex: piece.arrival_vertex,
						departure_vertex: piece.arrival_vertex,
					});
					stretch_to_vertex(&mut piece, samples, arcs, false);
				}
				(EndChoice::Corner, _) => stretch_to_vertex(&mut piece, samples, arcs, false),
				_ => {}
			}
		}

		let mut after = None;
		if let Some((choice, _, solution)) = leaving {
			match (choice, &mut piece.kind) {
				(EndChoice::Own, PieceKind::Stretch { t_out, .. }) => *t_out = snap(solution.departure_t),
				(EndChoice::Neighbor { arc, vertex_t, direction }, _) => {
					after = Some(Piece {
						kind: PieceKind::Stretch {
							arc,
							t_in: vertex_t,
							t_out: snap(solution.departure_t),
							direction,
						},
						arrival_vertex: piece.departure_vertex,
						departure_vertex: piece.departure_vertex,
					});
					stretch_to_vertex(&mut piece, samples, arcs, true);
				}
				(EndChoice::Corner, _) => stretch_to_vertex(&mut piece, samples, arcs, true),
				_ => {}
			}
		}

		for piece in [before, Some(piece), after].into_iter().flatten() {
			links.push(None);
			refined.push(piece);
		}
		if let Some(link) = links.last_mut() {
			*link = leaving.map(|(_, _, solution)| solution);
		}
	}

	// A stretch with refined ends that crossed over has no real extent, which only a decision too close to call produces
	for piece in &mut refined {
		if let PieceKind::Stretch { t_in, t_out, direction, .. } = &mut piece.kind
			&& (*t_out - *t_in) * *direction < -PARAM_EPSILON
		{
			flagged.extend(samples.adjacent_intervals(piece.arrival_vertex).chain(samples.adjacent_intervals(piece.departure_vertex)));
			let midpoint = (*t_in + *t_out) / 2.;
			(*t_in, *t_out) = (midpoint, midpoint);
		}
	}

	// The ball cannot follow a stretch curving the other way more tightly than itself, so such intervals need resolving
	let curvature = 1. / radius;
	for piece in &refined {
		let PieceKind::Stretch { arc, t_in, t_out, direction } = piece.kind else { continue };
		if arcs[arc].straight {
			continue;
		}
		let (low, high) = (t_in.min(t_out), t_in.max(t_out));
		let samples = &arcs[arc].samples;
		let first_interval = samples.partition_point(|&sample| sample <= low).saturating_sub(1);
		for (interval, window) in samples.windows(2).enumerate().skip(first_interval) {
			if window[0] >= high {
				break;
			}
			let (start, end) = (window[0].max(low), window[1].min(high));
			if end <= start {
				continue;
			}
			let too_tight = (1..=4).any(|s| -signed_curvature(&arcs[arc].cubic, start + (end - start) * s as f64 / 5.) * direction > curvature * (1. + 1e-6));
			if too_tight {
				flagged.insert((arc, interval));
			}
		}
	}

	(refined, links)
}

/// Whether a span's contact lies near the part of the geometry the sampled boundary found it touching, rather than on a
/// distant part of the same arcs where the equations happen to have another solution.
fn within_reach(piece: &Piece, choice: EndChoice, t: f64, arcs: &[HullArc]) -> bool {
	let (arc, low, high) = match (choice, piece.kind) {
		(EndChoice::Own, PieceKind::Stretch { arc, t_in, t_out, .. }) => (arc, t_in.min(t_out), t_in.max(t_out)),
		(EndChoice::Neighbor { arc, vertex_t, .. }, _) => (arc, vertex_t, vertex_t),
		_ => return true,
	};

	let samples = &arcs[arc].samples;
	let below = samples.partition_point(|&sample| sample < low).saturating_sub(CONTACT_REACH);
	let above = (samples.partition_point(|&sample| sample <= high) + CONTACT_REACH - 1).min(samples.len() - 1);
	(samples[below] - PARAM_EPSILON..=samples[above] + PARAM_EPSILON).contains(&t)
}

/// Pins a stretch's end to the vertex it shares with a neighboring arc or forms a corner at, leaving corners as they are.
fn stretch_to_vertex(piece: &mut Piece, samples: &SampleSet, arcs: &[HullArc], departure: bool) {
	let PieceKind::Stretch { arc, t_in, t_out, .. } = &mut piece.kind else { return };
	let vertex = if departure { piece.departure_vertex } else { piece.arrival_vertex };

	let at_vertex = samples.vertices[vertex as usize].memberships.iter().find_map(|membership| match *membership {
		Membership::Arc { arc: membership_arc, index } if membership_arc as usize == *arc => Some(arcs[*arc].samples[index as usize]),
		_ => None,
	});
	if let Some(t) = at_vertex {
		*(if departure { t_out } else { t_in }) = t;
	}
}

/// Snaps a parameter within epsilon of an arc's end onto it, so contacts at anchors land on them exactly.
fn snap(t: f64) -> f64 {
	if t < PARAM_EPSILON {
		0.
	} else if t > 1. - PARAM_EPSILON {
		1.
	} else {
		t
	}
}

/// One directed step along a boundary cycle.
#[derive(Clone, Copy)]
enum Step {
	/// Along a source segment from one of its parameters to another.
	Along { source: usize, from: f64, to: f64 },
	/// Straight across a span, from the contact made near sample vertex `from` to the one near `to`.
	Across { from: u32, to: u32 },
}

/// The part a step of a boundary cycle plays in the hull.
#[derive(Clone, Copy, PartialEq)]
enum StepRole {
	/// Bounds a region, which lies on its left.
	Outline,
	/// Walks out along a strand: geometry the ball reaches on both sides, which bounds no region.
	StrandOut,
	/// Walks back along a strand to where it set out.
	StrandBack,
}

/// Appends one cycle's boundary to the hull: cuts of the original segments joined by the spans. The cycle walks each
/// strand out and back, which becomes an open path traced once, and each region it bounds becomes a closed path. These
/// meet only at anchors in exactly the same place.
fn emit_cycle(hull: &mut BezPath, pieces: &[Piece], links: &[Option<SpanSolution>], arcs: &[HullArc], context: &HullContext) {
	let (steps, rough_ends) = cycle_steps(pieces, links, arcs, context);
	let n = steps.len();
	if n == 0 {
		return;
	}
	let partners = strand_partners(&steps, &rough_ends, context.distance_epsilon * 1e3);

	let continues = |previous: usize, next: usize| match (steps[previous], steps[next]) {
		(
			Step::Along { source, from, to },
			Step::Along {
				source: next_source,
				from: next_from,
				to: next_to,
			},
		) => source == next_source && (to - next_from).abs() < PARAM_EPSILON && (to - from).signum() == (next_to - next_from).signum(),
		_ => false,
	};

	// Start right after an outline step, so the walk ends on one that can close exactly, where no cut continues across the
	// seam. Without any, start right after the walk turns back at the end of a strand, so the first strand runs end to end.
	let previous = |i: usize| (i + n - 1) % n;
	let start = (0..n)
		.find(|&i| partners[previous(i)].is_none() && !continues(previous(i), i))
		.or_else(|| (0..n).find(|&i| partners[previous(i)].is_none()))
		.or_else(|| (0..n).find(|&i| partners[previous(i)] == Some(i)))
		.unwrap_or(0);
	let order: Vec<usize> = (0..n).map(|step| (start + step) % n).collect();

	let (roles, regions) = nest_strands(&order, &partners).unwrap_or_else(|| (vec![StepRole::Outline; n], vec![order.clone()]));

	// Settle where each step ends so every place paths meet is exactly one point. A strand walked back ends where it set
	// out, and the last outline step of each region closes it where it began.
	let closing: FxHashSet<usize> = regions.iter().filter_map(|region| region.last().copied()).collect();
	let (mut starts, mut ends) = (vec![Point::ZERO; n], vec![Point::ZERO; n]);
	let mut pen = rough_ends[order[n - 1]];
	let mut region_starts = vec![pen];
	for &i in &order {
		starts[i] = pen;
		ends[i] = match roles[i] {
			StepRole::StrandBack => {
				region_starts.pop();
				partners[i].map_or(rough_ends[i], |partner| starts[partner])
			}
			StepRole::Outline if closing.contains(&i) => region_starts.last().copied().unwrap_or(rough_ends[i]),
			_ => rough_ends[i],
		};
		if roles[i] == StepRole::StrandOut {
			region_starts.push(ends[i]);
		}
		pen = ends[i];
	}

	// Each run of strands walked out one after another becomes one open path
	let mut strands: Vec<Vec<usize>> = Vec::new();
	for (position, &i) in order.iter().enumerate() {
		if roles[i] != StepRole::StrandOut {
			continue;
		}
		match strands.last_mut() {
			Some(strand) if position > 0 && roles[order[position - 1]] == StepRole::StrandOut => strand.push(i),
			_ => strands.push(vec![i]),
		}
	}

	// Another path meets each place a path starts, so no cut merges across one
	let junctions: FxHashSet<(u64, u64)> = regions.iter().chain(&strands).filter_map(|run| run.first()).map(|&i| position_key(starts[i])).collect();

	let segments_of = |run: &[usize]| {
		let mut segments = Vec::new();
		let mut pending: Option<(usize, f64, f64, Point, Point)> = None;
		let flush = |segments: &mut Vec<PathSeg>, pending: &mut Option<(usize, f64, f64, Point, Point)>| {
			if let Some((source, from, to, start, end)) = pending.take() {
				segments.push(pinned(source_cut(&context.segments[source], from, to), start, end));
			}
		};

		let mut previous_step = None;
		for &i in run {
			match steps[i] {
				Step::Along { source, from, to } => {
					let merges = previous_step.is_some_and(|previous_step| continues(previous_step, i)) && !junctions.contains(&position_key(starts[i]));
					match &mut pending {
						Some((_, _, pending_to, _, pending_end)) if merges => (*pending_to, *pending_end) = (to, ends[i]),
						_ => {
							flush(&mut segments, &mut pending);
							pending = Some((source, from, to, starts[i], ends[i]));
						}
					}
				}
				Step::Across { .. } => {
					flush(&mut segments, &mut pending);
					if starts[i] != ends[i] {
						segments.push(PathSeg::Line(Line::new(starts[i], ends[i])));
					}
				}
			}
			previous_step = Some(i);
		}
		flush(&mut segments, &mut pending);

		segments
	};

	let trace = |hull: &mut BezPath, segments: &[PathSeg]| {
		let Some(first) = segments.first() else { return false };
		hull.move_to(first.start());
		for segment in segments {
			hull.push(segment.as_path_el());
		}
		true
	};

	for region in &regions {
		if trace(hull, &segments_of(region)) {
			hull.close_path();
		}
	}

	for strand in &strands {
		let mut segments = segments_of(strand);

		// A strand along source segments runs the way most of them were drawn
		let backward: f64 = strand.iter().map(|&i| if let Step::Along { from, to, .. } = steps[i] { from - to } else { 0. }).sum();
		if backward > 0. {
			segments.reverse();
			for segment in &mut segments {
				*segment = segment.reverse();
			}
		}

		trace(hull, &segments);
	}
}

/// The steps a refined cycle takes, with roughly where each ends. A stretch is split wherever another on the same source
/// segment ends, so a strand walked out along part of a segment and back along part of it pairs up step for step.
fn cycle_steps(pieces: &[Piece], links: &[Option<SpanSolution>], arcs: &[HullArc], context: &HullContext) -> (Vec<Step>, Vec<Point>) {
	let source_range = |piece: &Piece| match piece.kind {
		PieceKind::Stretch { arc, t_in, t_out, .. } if (t_out - t_in).abs() > PARAM_EPSILON => {
			let arc = &arcs[arc];
			Some((arc.source, snap(arc.to_source_t(t_in)), snap(arc.to_source_t(t_out))))
		}
		_ => None,
	};

	let mut breakpoints: FxHashMap<usize, Vec<f64>> = FxHashMap::default();
	for (source, from, to) in pieces.iter().filter_map(source_range) {
		breakpoints.entry(source).or_default().extend([from, to]);
	}
	for parameters in breakpoints.values_mut() {
		parameters.sort_by(f64::total_cmp);
		parameters.dedup();
	}

	let count = pieces.len();
	let (mut steps, mut ends) = (Vec::new(), Vec::new());
	for k in 0..count {
		if let Some((source, from, to)) = source_range(&pieces[k]) {
			let (low, high) = (from.min(to), from.max(to));
			let parameters = breakpoints.get(&source).map_or(&[][..], Vec::as_slice);
			let inside = parameters.partition_point(|&t| t <= low + PARAM_EPSILON)..parameters.partition_point(|&t| t < high - PARAM_EPSILON);
			let mut splits = parameters.get(inside).unwrap_or_default().to_vec();
			if to < from {
				splits.reverse();
			}

			let parameters: Vec<f64> = [from].into_iter().chain(splits).chain([to]).collect();
			for window in parameters.windows(2) {
				steps.push(Step::Along {
					source,
					from: window[0],
					to: window[1],
				});
				ends.push(source_point(&context.segments[source], window[1]));
			}
		}

		if let Some(span) = links[k] {
			steps.push(Step::Across {
				from: pieces[k].departure_vertex,
				to: pieces[(k + 1) % count].arrival_vertex,
			});
			ends.push(span.arrival);
		}
	}

	(steps, ends)
}

/// The step walking each step's geometry the other way, for the steps of a strand. Steps along a source segment pair up
/// by their parameter ranges, and spans by the sample vertices they join and the places they reach. O(steps log steps).
fn strand_partners(steps: &[Step], ends: &[Point], tolerance: f64) -> Vec<Option<usize>> {
	let n = steps.len();
	let start = |i: usize| ends[(i + n - 1) % n];
	let range_start = |i: usize| if let Step::Along { from, to, .. } = steps[i] { from.min(to) } else { 0. };

	let mut along: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
	let mut across: FxHashMap<(u32, u32), Vec<usize>> = FxHashMap::default();
	for (i, step) in steps.iter().enumerate() {
		match *step {
			Step::Along { source, .. } => along.entry(source).or_default().push(i),
			Step::Across { from, to } => across.entry((from, to)).or_default().push(i),
		}
	}

	// Steps reversing each other start their ranges together, so sorting by that narrows the search for a partner
	for indices in along.values_mut() {
		indices.sort_by(|&a, &b| range_start(a).total_cmp(&range_start(b)));
	}

	let mut partners = vec![None; n];
	for i in 0..n {
		if partners[i].is_some() {
			continue;
		}

		let candidates = match steps[i] {
			Step::Along { source, .. } => {
				let sorted = along.get(&source).map_or(&[][..], Vec::as_slice);
				let nearby = sorted.partition_point(|&j| range_start(j) < range_start(i) - PARAM_EPSILON)..sorted.partition_point(|&j| range_start(j) <= range_start(i) + PARAM_EPSILON);
				sorted.get(nearby).unwrap_or_default()
			}
			Step::Across { from, to } => across.get(&(to, from)).map_or(&[][..], Vec::as_slice),
		};
		let reverses = |j: usize| match (steps[i], steps[j]) {
			(Step::Along { from, to, .. }, Step::Along { from: back_from, to: back_to, .. }) => (from - back_to).abs() <= PARAM_EPSILON && (to - back_from).abs() <= PARAM_EPSILON,
			(Step::Across { .. }, Step::Across { .. }) => start(i).distance(ends[j]) <= tolerance && ends[i].distance(start(j)) <= tolerance,
			_ => false,
		};

		if let Some(j) = candidates.iter().copied().filter(|&j| j != i && partners[j].is_none() && reverses(j)).min() {
			partners[i] = Some(j);
			partners[j] = Some(i);
		}
	}

	partners
}

/// The role of each step in walk order, and the outline steps of each region, the outermost first. Strands nest like
/// parentheses, each walked back before any walked out after it, so the outline steps between walking a strand out and
/// back bound the region at its far end. Returns `None` if the pairs cross, which only a malformed cycle produces.
fn nest_strands(order: &[usize], partners: &[Option<usize>]) -> Option<(Vec<StepRole>, Vec<Vec<usize>>)> {
	let mut roles = vec![StepRole::Outline; partners.len()];
	let mut outermost = Vec::new();
	let mut nested = Vec::new();
	let mut open: Vec<(usize, Vec<usize>)> = Vec::new();

	for &i in order {
		match partners[i] {
			None => match open.last_mut() {
				Some((_, region)) => region.push(i),
				None => outermost.push(i),
			},
			Some(partner) if roles[partner] == StepRole::StrandOut => {
				let (outbound, region) = open.pop()?;
				if outbound != partner {
					return None;
				}
				roles[i] = StepRole::StrandBack;
				nested.push(region);
			}
			Some(_) => {
				roles[i] = StepRole::StrandOut;
				open.push((i, Vec::new()));
			}
		}
	}
	if !open.is_empty() {
		return None;
	}

	Some((roles, [outermost].into_iter().chain(nested).collect()))
}

/// The point at a parameter of a source segment, exactly its anchor at either end.
fn source_point(source: &PathSeg, t: f64) -> Point {
	if t == 0. {
		source.start()
	} else if t == 1. {
		source.end()
	} else {
		source.eval(t)
	}
}

/// A segment with its ends moved onto the given points, which lie within rounding error of where they were.
fn pinned(segment: PathSeg, start: Point, end: Point) -> PathSeg {
	match segment {
		PathSeg::Line(_) => PathSeg::Line(Line::new(start, end)),
		PathSeg::Quad(quad) => PathSeg::Quad(QuadBez::new(start, quad.p1, end)),
		PathSeg::Cubic(cubic) => PathSeg::Cubic(CubicBez::new(start, cubic.p1, cubic.p2, end)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use kurbo::{PathEl, Shape};

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

	/// The segments of a closed polygon through the given corners.
	fn polygon_segments(corners: &[(f64, f64)]) -> Vec<PathSeg> {
		(0..corners.len()).map(|i| PathSeg::Line(Line::new(corners[i], corners[(i + 1) % corners.len()]))).collect()
	}

	/// A square of side 100 with a rectangular notch 20 wide and 40 deep cut into its top edge.
	fn notched_square() -> Vec<PathSeg> {
		polygon_segments(&[(0., 0.), (100., 0.), (100., 100.), (60., 100.), (60., 60.), (40., 60.), (40., 100.), (0., 100.)])
	}

	/// The subpaths of a hull, each with its segments.
	fn subpaths(hull: &BezPath) -> Vec<BezPath> {
		let mut subpaths = Vec::new();
		for element in hull.elements() {
			if let PathEl::MoveTo(_) = element {
				subpaths.push(BezPath::new());
			}
			if let Some(subpath) = subpaths.last_mut() {
				subpath.push(*element);
			}
		}
		subpaths
	}

	/// The closed subpaths of a hull, which bound its regions.
	fn closed_subpaths(hull: &BezPath) -> Vec<BezPath> {
		subpaths(hull).into_iter().filter(|subpath| matches!(subpath.elements().last(), Some(PathEl::ClosePath))).collect()
	}

	/// The open subpaths of a hull with at least one segment, which follow its strands.
	fn strands(hull: &BezPath) -> Vec<BezPath> {
		let open = |subpath: &BezPath| !matches!(subpath.elements().last(), Some(PathEl::ClosePath)) && subpath.segments().next().is_some();
		subpaths(hull).into_iter().filter(open).collect()
	}

	/// Whether a point is exactly one of the anchors a path passes through.
	fn has_anchor(path: &BezPath, point: Point) -> bool {
		path.elements().iter().any(|element| element.end_point() == Some(point))
	}

	fn total_area(hull: &BezPath) -> f64 {
		closed_subpaths(hull).iter().map(|subpath| subpath.area()).sum()
	}

	/// Asserts the defining properties of a concave hull: every closed region winds counterclockwise, contains all the
	/// input geometry it surrounds, and every place it leaves the input geometry is spanned by a ball holding no geometry.
	fn assert_hull_valid(hull: &BezPath, segments: &[PathSeg], loose_points: &[Point], curvature: f64) {
		let radius = 1. / curvature;
		let scale = segments
			.iter()
			.flat_map(|segment| [segment.start(), segment.end()])
			.chain(loose_points.iter().copied())
			.map(|point| point.to_vec2().hypot())
			.fold(1., f64::max);
		let tolerance = scale * 1e-6;

		let regions = closed_subpaths(hull);
		for region in &regions {
			assert!(region.area() >= -tolerance, "every region must wind counterclockwise, found area {}", region.area());
		}

		// No part of the hull is traced twice, as a strand walked out and back would be
		let hull_segments: Vec<PathSeg> = hull.segments().collect();
		let same = |a: PathSeg, b: PathSeg| [0., 0.5, 1.].iter().all(|&t| a.eval(t).distance(b.eval(t)) <= tolerance);
		for (i, &a) in hull_segments.iter().enumerate() {
			for &b in &hull_segments[i + 1..] {
				assert!(!same(a, b) && !same(a, b.reverse()), "segment {a:?} is traced twice");
			}
		}

		// Containment: input geometry lies inside a region or on the hull's boundary (where open chains and points also count)
		let on_hull = |point: Point| {
			regions.iter().any(|region| region.winding(point) != 0)
				|| hull.segments().any(|segment| segment.nearest(point, 1e-9).distance_sq.sqrt() <= tolerance * 10.)
				|| hull.elements().iter().any(|element| matches!(element, PathEl::MoveTo(p) if p.distance(point) <= tolerance))
		};
		for segment in segments {
			for k in 0..=50 {
				let point = segment.eval(k as f64 / 50.);
				assert!(on_hull(point), "input point {point:?} on {segment:?} must lie in the hull");
			}
		}
		for &point in loose_points {
			assert!(on_hull(point), "loose input point {point:?} must lie in the hull");
		}

		// Every straight span's ball (a radius to its right, touching both ends) must hold no input geometry
		let distance_to_input = |point: Point| {
			let to_segments = segments.iter().map(|segment| segment.nearest(point, 1e-12).distance_sq.sqrt());
			to_segments.chain(loose_points.iter().map(|loose| loose.distance(point))).fold(f64::INFINITY, f64::min)
		};
		for segment in hull.segments() {
			let PathSeg::Line(line) = segment else { continue };
			let on_input = |point: Point| distance_to_input(point) <= tolerance;
			let chord = line.p1 - line.p0;
			if chord.hypot() <= tolerance
				|| !on_input(line.p0)
				|| !on_input(line.p1)
				|| segments.iter().any(
					|segment| matches!(segment, PathSeg::Line(input) if input.nearest(line.p0, 1e-12).distance_sq.sqrt() <= tolerance && input.nearest(line.p1, 1e-12).distance_sq.sqrt() <= tolerance),
				) {
				continue;
			}
			let half = chord.hypot() / 2.;
			let center = line.p0.midpoint(line.p1) + Vec2::new(chord.y, -chord.x).normalize() * (radius * radius - half * half).max(0.).sqrt();
			let clearance = distance_to_input(center);
			assert!(
				clearance >= radius * (1. - 1e-6) - tolerance,
				"span {line:?} has geometry inside its ball (clearance {clearance}, radius {radius})"
			);
		}
	}

	#[test]
	fn zero_curvature_is_the_convex_hull() {
		let segments = notched_square();
		assert_eq!(concave_hull_of_geometry(&segments, &[], 0.), convex_hull_of_geometry(&segments, &[]));
	}

	#[test]
	fn a_ball_smaller_than_a_notch_keeps_it_and_spans_its_inner_corners() {
		// A ball of radius 5 enters the 20 wide notch but reaches into neither of its inner corners, which it spans 5 from the corner
		let segments = notched_square();
		let hull = concave_hull_of_geometry(&segments, &[], 1. / 5.);
		assert_hull_valid(&hull, &segments, &[], 1. / 5.);
		assert!(
			(total_area(&hull) - 9225.).abs() < 1e-6,
			"expected the notch minus its two spanned corners, got area {}",
			total_area(&hull)
		);
	}

	#[test]
	fn a_ball_wider_than_a_notch_fills_it() {
		let segments = notched_square();
		let hull = concave_hull_of_geometry(&segments, &[], 1. / 15.);
		assert_hull_valid(&hull, &segments, &[], 1. / 15.);
		assert!((total_area(&hull) - 10_000.).abs() < 1e-6, "the notch narrower than the ball must fill, got area {}", total_area(&hull));
	}

	#[test]
	fn a_convex_shape_smaller_than_the_ball_keeps_its_exact_curves() {
		let segments = circle_segments(Point::new(20., -30.), 100.);
		let hull = concave_hull_of_geometry(&segments, &[], 1. / 10.);
		assert_hull_valid(&hull, &segments, &[], 1. / 10.);

		let cubics: Vec<CubicBez> = hull.segments().filter_map(|segment| if let PathSeg::Cubic(cubic) = segment { Some(cubic) } else { None }).collect();
		assert_eq!(cubics.len(), 4, "the circle's four quadrants must come through whole");
		for segment in &segments {
			let PathSeg::Cubic(input) = segment else { unreachable!() };
			assert!(cubics.contains(input), "quadrant {input:?} must be kept exactly");
		}
	}

	#[test]
	fn shapes_farther_apart_than_the_ball_stay_separate() {
		let mut segments = circle_segments(Point::new(0., 0.), 50.);
		segments.extend(circle_segments(Point::new(130., 0.), 50.));

		// The 30 wide gap lets a ball of radius 10 through, but not one of radius 20
		let apart = concave_hull_of_geometry(&segments, &[], 1. / 10.);
		assert_eq!(closed_subpaths(&apart).len(), 2);
		assert_hull_valid(&apart, &segments, &[], 1. / 10.);

		let together = concave_hull_of_geometry(&segments, &[], 1. / 20.);
		assert_eq!(closed_subpaths(&together).len(), 1);
		assert_hull_valid(&together, &segments, &[], 1. / 20.);
		assert!(total_area(&together) > 2. * PI * 2500. * 0.99);
	}

	#[test]
	fn point_clouds_give_alpha_shapes() {
		// A ring of points around a center point, and an outlier far away
		let mut points: Vec<Point> = (0..12).map(|i| Point::new(100. * (i as f64 * PI / 6.).cos(), 100. * (i as f64 * PI / 6.).sin())).collect();
		points.extend([Point::new(0., 0.), Point::new(400., 0.)]);

		// Ring neighbors are 52 apart, too close for a ball of radius 30, so the ring encloses its center as one polygon
		let ring = concave_hull_of_geometry(&[], &points, 1. / 30.);
		assert_hull_valid(&ring, &[], &points, 1. / 30.);
		let regions = closed_subpaths(&ring);
		assert_eq!(regions.len(), 1);
		assert!((regions[0].area() - 30_000.).abs() < 1e-6, "expected the regular 12-gon, got area {}", regions[0].area());
		assert!(ring.elements().contains(&PathEl::MoveTo(Point::new(400., 0.))), "the outlier must stand alone");

		// A ball of radius 20 slips between every pair, leaving only the points
		let scattered = concave_hull_of_geometry(&[], &points, 1. / 20.);
		assert_eq!(scattered.elements().len(), points.len());
		assert!(scattered.elements().iter().all(|element| matches!(element, PathEl::MoveTo(_))));
	}

	#[test]
	fn two_close_points_give_a_line() {
		let points = [Point::new(0., 0.), Point::new(10., 0.), Point::new(100., 50.)];
		let hull = concave_hull_of_geometry(&[], &points, 1. / 10.);

		let lines: Vec<PathSeg> = hull.segments().collect();
		assert_eq!(lines.len(), 1, "expected one open line, got {hull:?}");
		assert!((lines[0].start() == points[0] && lines[0].end() == points[1]) || (lines[0].start() == points[1] && lines[0].end() == points[0]));
		assert!(hull.elements().contains(&PathEl::MoveTo(points[2])), "the far point must stand alone");
	}

	#[test]
	fn an_open_path_smaller_than_the_ball_wraps_to_itself() {
		let wave = PathSeg::Cubic(CubicBez::new((0., 0.), (50., 80.), (100., -80.), (150., 0.)));
		let hull = concave_hull_of_geometry(&[wave], &[], 1.);
		assert!(closed_subpaths(&hull).is_empty(), "a thin hull has no closed region");
		assert_eq!(hull.segments().collect::<Vec<_>>(), [wave], "the hull must be the wave itself, drawn the same way");
	}

	#[test]
	fn a_branching_strand_is_traced_once_from_an_anchor_of_its_region() {
		// A ring of points with a branching tail, each point too close to its neighbors for a ball of radius 30 to pass
		let mut points: Vec<Point> = (0..12).map(|i| Point::new(100. * (i as f64 * PI / 6.).cos(), 100. * (i as f64 * PI / 6.).sin())).collect();
		points.extend([Point::new(150., 0.), Point::new(200., 0.), Point::new(250., 0.), Point::new(200., 50.)]);

		let hull = concave_hull_of_geometry(&[], &points, 1. / 30.);
		assert_hull_valid(&hull, &[], &points, 1. / 30.);

		let regions = closed_subpaths(&hull);
		assert_eq!(regions.len(), 1);
		assert!((regions[0].area() - 30_000.).abs() < 1e-6, "expected the regular 12-gon, got area {}", regions[0].area());

		let strands = strands(&hull);
		assert_eq!(strands.iter().flat_map(|strand| strand.segments()).count(), 4, "expected the tail's four edges, got {strands:?}");
		assert!(
			has_anchor(&regions[0], points[0]) && strands.iter().any(|strand| has_anchor(strand, points[0])),
			"the tail must start at the ring's anchor"
		);
	}

	#[test]
	fn a_strand_joining_two_regions_is_traced_once() {
		let ring = |center: f64| (0..12).map(move |i| Point::new(center + 100. * (i as f64 * PI / 6.).cos(), 100. * (i as f64 * PI / 6.).sin()));
		let mut points: Vec<Point> = ring(0.).chain(ring(400.)).collect();
		points.extend([Point::new(150., 0.), Point::new(200., 0.), Point::new(250., 0.)]);

		let hull = concave_hull_of_geometry(&[], &points, 1. / 30.);
		assert_hull_valid(&hull, &[], &points, 1. / 30.);

		let regions = closed_subpaths(&hull);
		assert_eq!(regions.len(), 2, "each ring must be a region of its own");
		for region in &regions {
			assert!((region.area() - 30_000.).abs() < 1e-6, "expected a regular 12-gon, got area {}", region.area());
		}
		assert_eq!(strands(&hull).iter().flat_map(|strand| strand.segments()).count(), 4, "expected the four edges joining the rings");
	}

	#[test]
	fn a_stick_on_a_circle_becomes_a_strand_starting_at_an_anchor_of_the_circle() {
		let mut segments = circle_segments(Point::ZERO, 50.);
		segments.push(PathSeg::Line(Line::new((50., 0.), (150., 0.))));

		let hull = concave_hull_of_geometry(&segments, &[], 1. / 10.);
		assert_hull_valid(&hull, &segments, &[], 1. / 10.);

		// The ball spans the corners on either side of the stick, so the strand starts where both spans meet it
		let (regions, strands) = (closed_subpaths(&hull), strands(&hull));
		assert_eq!((regions.len(), strands.len()), (1, 1), "expected the circle and the stick, got {hull:?}");
		let stick: Vec<PathSeg> = strands[0].segments().collect();
		assert_eq!(stick.len(), 1);
		assert_eq!(stick[0].end(), Point::new(150., 0.), "the stick must run the way it was drawn");
		assert!(has_anchor(&regions[0], stick[0].start()), "the stick must start exactly at an anchor of the circle");

		// The ball touching both the stick and the circle sits about 9 along the stick from the circle
		assert!(stick[0].start().y == 0. && (55. ..65.).contains(&stick[0].start().x), "the stick starts at {:?}", stick[0].start());
	}

	#[test]
	fn quadratic_and_line_segments_keep_their_kinds() {
		let segments = vec![
			PathSeg::Quad(QuadBez::new((100., 0.), (100., 100.), (0., 100.))),
			PathSeg::Line(Line::new((0., 100.), (0., 0.))),
			PathSeg::Line(Line::new((0., 0.), (100., 0.))),
		];
		let hull = concave_hull_of_geometry(&segments, &[], 1. / 10.);
		assert_hull_valid(&hull, &segments, &[], 1. / 10.);
		assert_eq!(hull.segments().filter(|segment| matches!(segment, PathSeg::Quad(_))).count(), 1);
	}

	#[test]
	fn area_grows_as_curvature_falls() {
		let mut segments = notched_square();
		segments.extend(circle_segments(Point::new(160., 50.), 30.));
		let loose = [Point::new(50., -60.)];

		let mut previous = 0.;
		for curvature in [0.5, 0.2, 0.1, 0.05, 0.02, 0.01, 0.005, 0.001, 0.] {
			let hull = concave_hull_of_geometry(&segments, &loose, curvature);
			if curvature > 0. {
				assert_hull_valid(&hull, &segments, &loose, curvature);
			}
			let area = total_area(&hull);
			assert!(area >= previous - 1e-6, "area must not shrink as curvature falls ({area} after {previous} at curvature {curvature})");
			previous = area;
		}
	}

	#[test]
	fn a_nearly_flat_ball_approaches_the_convex_hull() {
		let mut segments = circle_segments(Point::new(0., 0.), 50.);
		segments.extend(circle_segments(Point::new(300., 20.), 80.));
		let convex = convex_hull_of_geometry(&segments, &[]).area();
		let concave = total_area(&concave_hull_of_geometry(&segments, &[], 1e-6));
		assert!((convex - concave).abs() / convex < 1e-4, "convex {convex} vs nearly flat {concave}");
	}

	#[test]
	fn randomized_inputs_produce_valid_hulls() {
		// A deterministic PRNG so failures are reproducible
		let mut state: u64 = 0x853C_49E6_748F_EA9B;
		let mut random = move || {
			state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
			(state >> 11) as f64 / (1u64 << 53) as f64
		};

		for iteration in 0..60 {
			let mut point = || Point::new(random() * 400., random() * 400.);
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

			let curvature = [1. / 20., 1. / 60., 1. / 150.][iteration % 3];
			let hull = concave_hull_of_geometry(&segments, &loose_points, curvature);
			assert_hull_valid(&hull, &segments, &loose_points, curvature);
		}
	}
}
