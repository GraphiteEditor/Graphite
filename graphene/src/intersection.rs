use crate::boolean_ops::{split_path_seg, subdivide_path_seg};
use crate::consts::{F64LOOSE, F64PRECISE};

use glam::{DAffine2, DMat2, DVec2};
use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveDeriv, ParamCurveExtrema, PathSeg, Point, QuadBez, Rect, Shape, Vec2};
use std::collections::VecDeque;
use std::ops::Mul;

#[derive(Debug, Clone, Default, Copy)]
/// A quad defined by four vertices.
pub struct Quad([DVec2; 4]);

impl Quad {
	/// Convert a box defined by two corner points to a quad.
	pub fn from_box(bbox: [DVec2; 2]) -> Self {
		let size = bbox[1] - bbox[0];
		Self([bbox[0], bbox[0] + size * DVec2::X, bbox[1], bbox[0] + size * DVec2::Y])
	}

	/// Get all the edges in the quad.
	pub fn lines(&self) -> [Line; 4] {
		[
			Line::new(to_point(self.0[0]), to_point(self.0[1])),
			Line::new(to_point(self.0[1]), to_point(self.0[2])),
			Line::new(to_point(self.0[2]), to_point(self.0[3])),
			Line::new(to_point(self.0[3]), to_point(self.0[0])),
		]
	}

	/// Generate a [BezPath] of the quad
	pub fn path(&self) -> BezPath {
		let mut path = kurbo::BezPath::new();
		path.move_to(to_point(self.0[0]));
		path.line_to(to_point(self.0[1]));
		path.line_to(to_point(self.0[2]));
		path.line_to(to_point(self.0[3]));
		path.close_path();
		path
	}
}

impl Mul<Quad> for DAffine2 {
	type Output = Quad;

	fn mul(self, rhs: Quad) -> Self::Output {
		let mut output = Quad::default();
		for (i, point) in rhs.0.iter().enumerate() {
			output.0[i] = self.transform_point2(*point);
		}
		output
	}
}

fn to_point(vec: DVec2) -> Point {
	Point::new(vec.x, vec.y)
}

/// Return `true` if `quad` intersects `shape`.
/// This is the case if any of the following conditions are true:
/// - the edges of `quad` and `shape` intersect
/// - `shape` is entirely contained within `quad`
/// - `filled` is `true` and `quad` is entirely contained within `shape`.
pub fn intersect_quad_bez_path(quad: Quad, shape: &BezPath, filled: bool) -> bool {
	let mut shape = shape.clone();

	// For filled shapes act like shape was closed even if it isn't
	if filled && shape.elements().last() != Some(&kurbo::PathEl::ClosePath) {
		shape.close_path();
	}

	// Check if outlines intersect
	if shape.segments().any(|path_segment| quad.lines().iter().any(|line| !path_segment.intersect_line(*line).is_empty())) {
		return true;
	}
	// Check if selection is entirely within the shape
	if filled && shape.contains(to_point(quad.0[0])) {
		return true;
	}

	// Check if shape is entirely within selection
	get_arbitrary_point_on_path(&shape).map(|shape_point| quad.path().contains(shape_point)).unwrap_or_default()
}

/// Returns a point on `path`.
/// This function will usually return the first point from the path's first segment, but callers should not rely on this behavior.
pub fn get_arbitrary_point_on_path(path: &BezPath) -> Option<Point> {
	path.segments().next().map(|seg| match seg {
		PathSeg::Line(line) => line.p0,
		PathSeg::Quad(quad) => quad.p0,
		PathSeg::Cubic(cubic) => cubic.p0,
	})
}

//
// Bezier Curve Intersection algorithm
//

/// Each intersection has two curves. This enum helps distinguished between the two.
// TODO: refactor so actual curve data and `Origin` aren't separate
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Origin {
	Alpha,
	Beta,
}

impl std::ops::Not for Origin {
	type Output = Self;
	fn not(self) -> Self {
		match self {
			Origin::Alpha => Origin::Beta,
			Origin::Beta => Origin::Alpha,
		}
	}
}

#[derive(Debug, PartialEq)]
pub struct Intersect {
	pub point: Point,
	pub t_a: f64,
	pub t_b: f64,
	pub a_seg_index: i32,
	pub b_seg_index: i32,
	pub quality: f64,
}

impl Intersect {
	pub fn new(point: Point, t_a: f64, t_b: f64, a_seg_index: i32, b_seg_index: i32) -> Self {
		Intersect {
			point,
			t_a,
			t_b,
			a_seg_index,
			b_seg_index,
			quality: -1.0,
		}
	}

	pub fn add_index(&mut self, a_index: i32, b_index: i32) {
		self.a_seg_index = a_index;
		self.b_seg_index = b_index;
	}

	pub fn segment_index(&self, o: Origin) -> i32 {
		match o {
			Origin::Alpha => self.a_seg_index,
			Origin::Beta => self.b_seg_index,
		}
	}

	pub fn t_value(&self, o: Origin) -> f64 {
		match o {
			Origin::Alpha => self.t_a,
			Origin::Beta => self.t_b,
		}
	}
}

impl From<(Point, f64, f64)> for Intersect {
	fn from(place_time: (Point, f64, f64)) -> Self {
		Intersect {
			point: place_time.0,
			t_a: place_time.1,
			t_b: place_time.2,
			a_seg_index: 0,
			b_seg_index: 0,
			quality: 0.0,
		}
	}
}

#[derive(Clone, Copy)]
struct SubCurve<'a> {
	pub curve: &'a PathSeg,
	pub start_t: f64,
	pub end_t: f64,
	/// Local endpoints
	local: [Point; 2],
	pub extrema: &'a Vec<(Point, f64)>,
}

impl<'a> SubCurve<'a> {
	/// Extrema given by [SubCurve::subcurve_extrema], they are stored externally so they don't have to recalculated
	pub fn new(parent: &'a PathSeg, extrema: &'a Vec<(Point, f64)>) -> Self {
		SubCurve {
			curve: parent,
			start_t: 0.0,
			end_t: 1.0,
			local: [parent.eval(0.0), parent.eval(1.0)],
			extrema,
		}
	}

	pub fn subcurve_extrema(parent: &PathSeg) -> Vec<(Point, f64)> {
		// Extrema at endpoints should not be included here as they must be calculated for each subcurve
		// Note: below filtering may filter out extrema near the endpoints
		parent
			.extrema()
			.iter()
			.filter_map(|t| if *t > F64PRECISE && *t < 1.0 - F64PRECISE { Some((parent.eval(*t), *t)) } else { None })
			.collect()
	}

	fn bounding_box(&self) -> Rect {
		let mut bound = Rect {
			x0: self.start().x,
			y0: self.start().y,
			x1: self.end().x,
			y1: self.end().y,
		};
		self.local
			.iter()
			.chain(
				self.extrema
					.iter()
					// Filter out "internal extrema which are not contained within this subcurve"
					.filter_map(|place_time| if place_time.1 > self.start_t && place_time.1 < self.end_t { Some(&place_time.0) } else { None }),
			)
			.for_each(|p| {
				if p.x < bound.x0 {
					bound.x0 = p.x;
				}
				if p.x > bound.x1 {
					bound.x1 = p.x;
				}
				if p.y < bound.y0 {
					bound.y0 = p.y;
				}
				if p.y > bound.y1 {
					bound.y1 = p.y;
				}
			});
		bound
	}

	fn available_precision(&self) -> f64 {
		(self.start_t - self.end_t).abs()
	}

	/// Split subcurve at `t`, as though the subcurve is a bezier curve, where `t` is a value between `0.0` and `1.0`.
	fn split<'sub_life>(self: &'sub_life SubCurve<'a>, t: f64) -> (SubCurve<'a>, SubCurve<'a>) {
		let split_t = self.start_t + t * (self.end_t - self.start_t);
		(
			SubCurve {
				curve: self.curve,
				start_t: self.start_t,
				end_t: split_t,
				local: [self.start(), self.curve.eval(split_t)],
				extrema: self.extrema,
			},
			SubCurve {
				curve: self.curve,
				start_t: split_t,
				end_t: self.end_t,
				local: [self.curve.eval(split_t), self.end()],
				extrema: self.extrema,
			},
		)
	}

	fn start(&self) -> Point {
		self.local[0]
	}

	fn end(&self) -> Point {
		self.local[1]
	}
}

// TODO: use the cool algorithm described in: https://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.99.9678&rep=rep1&type=pdf
/// Bezier Curve Intersection Algorithm
fn path_intersections(a: &SubCurve, b: &SubCurve, intersections: &mut Vec<Intersect>) {
	// At recursion depth 10:
	// - maximum recursive execution paths = 4^10 = 1048576
	// - minimum recursive execution paths = 1
	// - up to 9 cubic Bezier intersections
	// - `SubCurve` is 1/2^10 of the original curve
	// - conservatively: there should never be more than 9 * 100 = 1000 total recursive execution paths
	// TODO: Can probably be much less, should find a better capacity
	const MAX_CALL_NUM: usize = 1000;
	let mut call_buffer: VecDeque<(SubCurve, SubCurve)> = VecDeque::with_capacity(MAX_CALL_NUM);
	let mut recursion = 1.0;

	fn helper<'a, 'b: 'a>(a: &'a SubCurve<'b>, b: &'a SubCurve<'b>, recursion: f64, intersections: &mut Vec<Intersect>, call_buffer: &'a mut VecDeque<(SubCurve<'b>, SubCurve<'b>)>) {
		if let (PathSeg::Line(_), _) | (_, PathSeg::Line(_)) = (a.curve, b.curve) {
			line_curve_intersections((&mut a.curve.clone(), &mut b.curve.clone()), |a, b| valid_t(a) && valid_t(b), intersections);
			return;
		}
		// We are close enough to try linear approximation
		if recursion < (1 << 10) as f64 {
			// If the number of sub-curves being checked could exceed the threshold, check for overlap
			if call_buffer.len() >= MAX_CALL_NUM - 4 {
				overlapping_curve_intersections(a.curve, b.curve)
					.into_iter()
					.flatten()
					.for_each(|intersect| intersections.push(intersect));
				// Regardless of whether intersections were found, need to return to prevent crashing the editor
				// If no intersections are found above the curves are very close to overlapping but not quite
				return;
			}
			if let Some(mut cross) = line_intersection(&Line { p0: a.start(), p1: a.end() }, &Line { p0: b.start(), p1: b.end() }) {
				// Intersection `t_value` equals the recursive `t_value` + interpolated intersection value
				cross.t_b = b.start_t + cross.t_b * recursion;
				cross.t_a = a.start_t + cross.t_a * recursion;
				cross.quality = guess_quality(a.curve, b.curve, &cross);

				// log::debug!("checking: {:?}", cross.quality);
				if cross.quality <= F64LOOSE {
					// Invalid intersections should still be rejected
					// Rejects "valid" intersections on the non-inclusive end of a `PathSeg`
					if valid_t(cross.t_a) && valid_t(cross.t_b) {
						intersections.push(cross);
					}
					return;
				}

				// Eventually the points in the curve become too close together to split the curve meaningfully
				// Return the best estimate of intersection regardless of quality
				// Also provides a base case and prevents infinite recursion
				if a.available_precision() <= F64PRECISE || b.available_precision() <= F64PRECISE {
					log::debug!("precision reached");
					intersections.push(cross);
					return;
				}
			}
			// Alternate base case
			// Note: may occur for the less forgiving side of a `PathSeg` endpoint intersect
			if a.available_precision() <= F64PRECISE || b.available_precision() <= F64PRECISE {
				log::debug!("precision reached without finding intersect");
				return;
			}
		}
		let (a1, a2) = a.split(0.5);
		let (b1, b2) = b.split(0.5);

		if overlap(&a1.bounding_box(), &b1.bounding_box()) {
			call_buffer.push_back((a1, b1));
		}
		if overlap(&a1.bounding_box(), &b2.bounding_box()) {
			call_buffer.push_back((a1, b2));
		}
		if overlap(&a2.bounding_box(), &b1.bounding_box()) {
			call_buffer.push_back((a2, b1));
		}
		if overlap(&a2.bounding_box(), &b2.bounding_box()) {
			call_buffer.push_back((a2, b2));
		}
	}

	call_buffer.push_back((*a, *b));
	while !call_buffer.is_empty() {
		let mut current_level = call_buffer.len();
		while current_level > 0 {
			let (a, b) = call_buffer.pop_front().unwrap();
			helper(&a, &b, recursion, intersections, &mut call_buffer);
			recursion /= 2.0;
			current_level -= 1;
		}
	}
}

/// Does nothing when neither PathSeg in `line_curve` is a line.
/// Closure `t_validate` takes the two t_values of an Intersect as arguments.
/// The order of the t_values corresponds with the order of the PathSegs in `line_curve`,
/// `t_validate` should return true for allowable intersection t_values, valid intersections will be added to `intersections`.
pub fn line_curve_intersections<F>(line_curve: (&mut PathSeg, &mut PathSeg), t_validate: F, intersections: &mut Vec<Intersect>)
where
	F: Fn(f64, f64) -> bool,
{
	extend_curve(line_curve.0, F64PRECISE);
	extend_curve(line_curve.1, F64PRECISE);

	if let (PathSeg::Line(line), PathSeg::Line(line2)) = line_curve {
		if let Some(cross) = line_intersection(line, line2) {
			if t_validate(cross.t_a, cross.t_b) {
				intersections.push(cross);
			}
		} else {
			//the lines may be overlapping
			overlapping_curve_intersections(line_curve.0, line_curve.1)
				.into_iter()
				.flatten()
				.for_each(|intersect| intersections.push(intersect))
		}
	} else {
		let is_line_a;
		let (line, curve) = match line_curve {
			(PathSeg::Line(line), curve) => {
				is_line_a = true;
				(line, curve)
			}
			(curve, PathSeg::Line(line)) => {
				is_line_a = false;
				(line, curve)
			}
			_ => return,
		};
		let roots = match curve {
			PathSeg::Quad(quad) => Vec::from(quad_line_intersect(line, quad)),
			PathSeg::Cubic(cubic) => Vec::from(cubic_line_intersect(line, cubic)),
			_ => vec![], // Should never occur
		};
		intersections.extend(
			roots
				.iter()
				.filter_map(|time_option| {
					if let Some(time) = time_option {
						let point = match curve {
							PathSeg::Cubic(cubic) => cubic.eval(*time),
							PathSeg::Quad(quad) => quad.eval(*time),
							_ => Point::new(0.0, 0.0), // Should never occur
						};
						// The intersection point should be on the line, unless floating point math error produces bad results
						let line_time = projection_on_line(line, &point);
						if !t_validate(line_time, *time) {
							return None;
						}
						if is_line_a {
							Some(Intersect::from((point, line_time, *time)))
						} else {
							Some(Intersect::from((point, *time, line_time)))
						}
					} else {
						None
					}
				})
				.collect::<Vec<Intersect>>(),
		);
	}
}

/// Extend the starting point of `curve` backwards along its derivative.
/// Used to make finding intersections near endpoints reliable.
pub fn extend_curve(curve: &mut PathSeg, distance: f64) {
	fn extended_start<C: ParamCurve + ParamCurveDeriv>(c: &mut C, d: f64) -> Point {
		let mut c_prime = c.deriv().eval(0.0);
		c_prime.x *= d / c_prime.distance(Point::ORIGIN);
		c_prime.y *= d / c_prime.distance(Point::ORIGIN);
		let es_vec = c.eval(0.0) - c_prime;
		Point { x: es_vec.x, y: es_vec.y }
	}
	match curve {
		PathSeg::Line(line) => line.p0 = extended_start(line, distance),
		PathSeg::Quad(quad) => quad.p0 = extended_start(quad, distance),
		PathSeg::Cubic(cubic) => cubic.p0 = extended_start(cubic, distance),
	};
}

/// For quality Q in the worst case, the point on curve `a` corresponding to `guess` is distance Q from the point on curve `b`.
// TODO: Optimization: inline? maybe..
fn guess_quality(a: &PathSeg, b: &PathSeg, guess: &Intersect) -> f64 {
	let at_a = b.eval(guess.t_b);
	let at_b = a.eval(guess.t_a);
	at_a.distance(guess.point) + at_b.distance(guess.point)
}

/// If curves overlap, returns intersections corresponding to the endpoints of the overlapping section
///
/// # Panics
/// May panic if either curve is very short, or has endpoints which are close together
// TODO: test the case where a and b are identical
// TODO: test this, especially the overlapping curve cases which are more complex
pub fn overlapping_curve_intersections(a: &PathSeg, b: &PathSeg) -> [Option<Intersect>; 2] {
	// To check if two curves overlap we find if the endpoints of either curve are on the other curve.
	// Then, the curves are split at these points, if the resulting control polygons match the curves are the same
	let b_on_a: Vec<Option<f64>> = [point_t_value(a, &b.start()), point_t_value(a, &b.end())].into_iter().collect();
	let a_on_b: Vec<Option<f64>> = [point_t_value(b, &a.start()), point_t_value(b, &a.end())].into_iter().collect();
	// I think, but have not mathematically shown, that if a and b are parts of the same curve then b_on_a and a_on_b should together have no more than three non-None elements. Which occurs when a or b is a cubic bezier which crosses itself
	let b_on_a_not_none = b_on_a.iter().filter_map(|o| *o).count();
	let a_on_b_not_none = a_on_b.iter().filter_map(|o| *o).count();
	match b_on_a_not_none + a_on_b_not_none {
		2 | 3 => {
			let (t1a, t1b, t2a, t2b): (f64, f64, f64, f64);
			let to_compare = if b_on_a_not_none == 2 {
				t1a = b_on_a[0].unwrap();
				t1b = 0.0;
				t2a = b_on_a[1].unwrap();
				t2b = 1.0;
				let mut split_at = if t1a > t2a { [t2a, t1a] } else { [t1a, t2a] };
				(*b, subdivide_path_seg(a, &mut split_at)[1].unwrap())
			} else if a_on_b_not_none == 2 {
				t1a = 0.0;
				t1b = a_on_b[0].unwrap();
				t2a = 1.0;
				t2b = a_on_b[1].unwrap();
				let mut split_at = if t1b > t2b { [t2b, t1b] } else { [t1b, t2b] };
				(*a, subdivide_path_seg(b, &mut split_at)[1].unwrap())
			} else {
				match (b_on_a[0], b_on_a[1], a_on_b[0], a_on_b[1]) {
					(None, Some(a_val), None, Some(b_val)) => {
						t1b = b_val;
						t2b = 1.0;
						t1a = 1.0;
						t2a = a_val;
						(split_path_seg(b, b_val).1.unwrap(), split_path_seg(a, a_val).1.unwrap())
					}
					(None, Some(a_val), Some(b_val), None) => {
						t1b = b_val;
						t2b = 1.0;
						t1a = 0.0;
						t2a = a_val;
						(split_path_seg(b, b_val).1.unwrap(), split_path_seg(a, a_val).0.unwrap())
					}
					(Some(a_val), None, None, Some(b_val)) => {
						t1b = 0.0;
						t2b = b_val;
						t1a = a_val;
						t2a = 1.0;
						(split_path_seg(b, b_val).0.unwrap(), split_path_seg(a, a_val).1.unwrap())
					}
					(Some(a_val), None, Some(b_val), None) => {
						t1b = 0.0;
						t2b = b_val;
						t1a = a_val;
						t2a = 0.0;
						(split_path_seg(b, b_val).0.unwrap(), split_path_seg(a, a_val).0.unwrap())
					}
					_ => unreachable!("Overlapping curve intersections: too many intersections for match arm"),
				}
			};
			let mut to_return = [None, None];
			if match_control_polygon(&to_compare.0, &to_compare.1) {
				if valid_t(t1a) && valid_t(t1b) {
					to_return[0] = Some(Intersect::from((to_compare.0.start(), t1a, t1b)));
				}
				if valid_t(t2a) && valid_t(t2b) {
					to_return[1] = Some(Intersect::from((to_compare.0.end(), t2a, t2b)));
				}
			}
			to_return
		}
		_ => [None, None],
	}
}

/// Returns true if the Bezier curves described by `a` and `b` have the same control polygon.
/// The order of the polygon does not effect the result,
pub fn match_control_polygon(a: &PathSeg, b: &PathSeg) -> bool {
	let mut a_polygon = get_control_polygon(a);
	let mut b_polygon = get_control_polygon(b);
	// Allow matching of polygons whose points are reverse ordered
	if a_polygon.first().unwrap() == b_polygon.last().unwrap() && a_polygon.last().unwrap() == b_polygon.first().unwrap() {
		b_polygon.reverse()
	}
	if a_polygon.len() == b_polygon.len() {
		a_polygon.iter().eq(b_polygon.iter())
	} else {
		// A sneaky higher degree Bezier curve may pose as a lower degree one
		let (a_ref, b_ref) = if a_polygon.len() < b_polygon.len() {
			(&mut b_polygon, &mut a_polygon)
		} else {
			(&mut a_polygon, &mut b_polygon)
		};

		let mut a_iter = a_ref.iter();
		for b_point in b_ref.iter() {
			let a_point = a_iter.next().unwrap();
			if *a_point != *b_point {
				loop {
					if let Some(a_line) = a_iter.next() {
						if !colinear(&[a_point, b_point, a_line]) {
							return false;
						}
						if *a_line == *b_point {
							break;
						}
					} else {
						return false;
					}
				}
			}
		}
		true
	}
}

pub fn colinear(points: &[&Point]) -> bool {
	let ray = Line { p0: *points[0], p1: *points[1] };
	for p in points.iter().skip(2) {
		if point_t_value(&PathSeg::Line(ray), p).is_none() {
			return false;
		}
	}
	true
}

pub fn get_control_polygon(a: &PathSeg) -> Vec<Point> {
	match a {
		PathSeg::Line(Line { p0, p1 }) => vec![*p0, *p1],
		PathSeg::Quad(QuadBez { p0, p1, p2 }) => vec![*p0, *p1, *p2],
		PathSeg::Cubic(CubicBez { p0, p1, p2, p3 }) => vec![*p0, *p1, *p2, *p3],
	}
}

/// If `p` in on `PathSeg` `a`, returns `Some(t_value)` for `p` in the edge case where the path crosses itself,
/// and `p` is at the cross, the first t_value found (but not necessarily the smallest `t_value`) is returned.
// TODO: create a trait or something for roots to remove duplicate code
pub fn point_t_value(a: &PathSeg, p: &Point) -> Option<f64> {
	match a {
		PathSeg::Line(line) => {
			let [mut p0, p1] = linear_bezier_coefficients(line);
			p0 -= p.to_vec2();
			let x_root = linear_root(p0.x, p1.x);
			let y_root = linear_root(p0.y, p1.y);
			if let (Some(x_root_val), Some(y_root_val)) = (x_root, y_root) {
				if (y_root_val - x_root_val).abs() < F64LOOSE {
					return Some(x_root_val);
				}
			}
			return None;
		}
		PathSeg::Quad(quad) => {
			let [mut p0, p1, p2] = quadratic_bezier_coefficients(quad);
			p0 -= p.to_vec2();
			let x_roots = quadratic_real_roots(p0.x, p1.x, p2.x);
			quadratic_real_roots(p0.y, p1.y, p2.y)
				.into_iter()
				.find(|yt_option| {
					x_roots
						.iter()
						.any(|xt_option| yt_option.is_some() && xt_option.is_some() && ((yt_option.unwrap() - xt_option.unwrap()).abs() < F64LOOSE))
				})
				.flatten()
		}
		PathSeg::Cubic(cubic) => {
			let [mut p0, p1, p2, p3] = cubic_bezier_coefficients(cubic);
			p0 -= p.to_vec2();
			let x_roots = cubic_real_roots(p0.x, p1.x, p2.x, p3.x);
			cubic_real_roots(p0.y, p1.y, p2.y, p3.y)
				.into_iter()
				.find(|yt_option| {
					x_roots
						.iter()
						.any(|xt_option| yt_option.is_some() && xt_option.is_some() && ((yt_option.unwrap() - xt_option.unwrap()).abs() < F64LOOSE))
				})
				.flatten()
		}
	}
	.and_then(|t| if valid_t(t) { Some(t) } else { None })
}

pub fn intersections(a: &BezPath, b: &BezPath) -> Vec<Intersect> {
	// log::info!("{:?}", a.to_svg());
	// log::info!("{:?}", b.to_svg());

	let mut intersections: Vec<Intersect> = Vec::new();
	// There is some duplicate computation of b_extrema here, but I doubt it's significant
	a.segments().enumerate().for_each(|(a_index, a_seg)| {
		let a_extrema = SubCurve::subcurve_extrema(&a_seg);
		b.segments().enumerate().for_each(|(b_index, b_seg)| {
			let b_extrema = SubCurve::subcurve_extrema(&b_seg);
			let mut intersects = Vec::new();
			path_intersections(&SubCurve::new(&a_seg, &a_extrema), &SubCurve::new(&b_seg, &b_extrema), &mut intersects);
			for mut path_intersection in intersects {
				path_intersection.add_index(a_index.try_into().unwrap(), b_index.try_into().unwrap());
				intersections.push(path_intersection);
			}
		})
	});

	// log::info!("{:?}", intersections);

	intersections
}

/// Returns the intersection point as if lines extended forever.
pub fn line_intersect_point(a: &Line, b: &Line) -> Option<Point> {
	line_intersection_unchecked(a, b).map(|intersect| intersect.point)
}

/// Returns intersection point and `t` values, treating lines as Bezier curves.
pub fn line_intersection(a: &Line, b: &Line) -> Option<Intersect> {
	match line_intersection_unchecked(a, b) {
		Some(intersect) => {
			if valid_t(intersect.t_a) && valid_t(intersect.t_b) {
				Some(intersect)
			} else {
				None
			}
		}
		None => None,
	}
}

/// Returns intersection point and `t` values, treating lines as rays.
pub fn line_intersection_unchecked(a: &Line, b: &Line) -> Option<Intersect> {
	let slopes = DMat2::from_cols_array(&[(b.p1 - b.p0).x, (b.p1 - b.p0).y, (a.p0 - a.p1).x, (a.p0 - a.p1).y]);
	if slopes.determinant() == 0.0 {
		return None;
	}
	let t_values = slopes.inverse() * DVec2::new(a.p0.x - b.p0.x, a.p0.y - b.p0.y);
	Some(Intersect::from((b.eval(t_values[0]), t_values[1], t_values[0])))
}

/// Returns the `t_value` of the point nearest to `p` on `a`.
pub fn projection_on_line(a: &Line, p: &Point) -> f64 {
	let ray = a.p1.to_vec2() - a.p0.to_vec2();
	ray.dot(p.to_vec2() - a.p0.to_vec2()) / ((ray.to_point().distance(Point::ORIGIN)) * (ray.to_point().distance(Point::ORIGIN)))
}

pub fn cubic_line_intersect(a: &Line, b: &CubicBez) -> [Option<f64>; 3] {
	let l_y = a.p1.x - a.p0.x;
	let l_x = a.p1.y - a.p0.y;
	let bp0 = b.p0.to_vec2();
	let bp1 = b.p1.to_vec2();
	let bp2 = b.p2.to_vec2();
	let bp3 = b.p3.to_vec2();
	let c0 = bp0;
	let c1 = -3.0 * bp0 + 3.0 * bp1;
	let c2 = 3.0 * bp0 - 6.0 * bp1 + 3.0 * bp2;
	let c3 = -1.0 * bp0 + 3.0 * bp1 - 3.0 * bp2 + bp3;

	cubic_real_roots(
		-a.p0.y * l_y + a.p0.x * l_x - l_x * c0.x + l_y * c0.y,
		l_y * c1.y - l_x * c1.x,
		l_y * c2.y - l_x * c2.x,
		l_y * c3.y - l_x * c3.x,
	)
}

pub fn quad_line_intersect(a: &Line, b: &QuadBez) -> [Option<f64>; 2] {
	let l_y = a.p1.x - a.p0.x;
	let l_x = a.p1.y - a.p0.y;
	let bp0 = b.p0.to_vec2();
	let bp1 = b.p1.to_vec2();
	let bp2 = b.p2.to_vec2();
	let c0 = bp0;
	let c1 = -2.0 * bp0 + 2.0 * bp1;
	let c2 = bp0 - 2.0 * bp1 + bp2;

	quadratic_real_roots(-a.p0.y * l_y + a.p0.x * l_x - l_x * c0.x + l_y * c0.y, l_y * c1.y - l_x * c1.x, l_y * c2.y - l_x * c2.x)
}

/// Returns real roots to cubic equation: `f(t) = a0 + t*a1 + t^2*a2 + t^3*a3`.
/// This function uses the Cardano-Viete and Numerical Recipes algorithm, found here: https://quarticequations.com/Cubic.pdf
pub fn cubic_real_roots(mut a0: f64, mut a1: f64, mut a2: f64, a3: f64) -> [Option<f64>; 3] {
	use std::f64::consts::FRAC_PI_3 as PI_3;

	a0 /= a3;
	a1 /= a3;
	a2 /= a3;

	let q: f64 = a1 / 3.0 - a2 * a2 / 9.0;
	let r: f64 = (a1 * a2 - 3.0 * a0) / 6.0 - a2 * a2 * a2 / 27.0;

	let r2_q3 = r * r + q * q * q;
	if r2_q3 > 0.0 {
		#[allow(non_snake_case)] // Allow name `A` for consistency with algorithm
		let A = (r.abs() + r2_q3.sqrt()).cbrt();

		let t1 = match r {
			r if r >= 0.0 => A - q / A,
			r if r < 0.0 => q / A - A,
			_ => 0.0, // Should never occur
		};

		[Some(t1 - a2 / 3.0), None, None]
	} else {
		let phi = match q > -F64PRECISE && q < F64PRECISE {
			true => 0.0,
			false => (r / (-q).powf(3.0 / 2.0)).acos() / 3.0,
		};

		[
			Some(2.0 * (-q).sqrt() * (phi).cos() - a2 / 3.0),
			Some(2.0 * (-q).sqrt() * (phi + 2.0 * PI_3).cos() - a2 / 3.0),
			Some(2.0 * (-q).sqrt() * (phi - 2.0 * PI_3).cos() - a2 / 3.0),
		]
	}
}

/// A quadratic bezier can be written `x = p0 + t*p1 + t^2*p2 + t^3*p3`, where `x`, `p0`, `p1`, `p2`, and `p3` are vectors.
/// This function returns `[p0, p1, p2, p3]`.
pub fn cubic_bezier_coefficients(cubic: &CubicBez) -> [Vec2; 4] {
	let p0 = cubic.p0.to_vec2();
	let p1 = cubic.p1.to_vec2();
	let p2 = cubic.p2.to_vec2();
	let p3 = cubic.p3.to_vec2();
	let c0 = p0;
	let c1 = -3.0 * p0 + 3.0 * p1;
	let c2 = 3.0 * p0 - 6.0 * p1 + 3.0 * p2;
	let c3 = -1.0 * p0 + 3.0 * p1 - 3.0 * p2 + p3;
	[c0, c1, c2, c3]
}

/// Returns real roots to the quadratic equation: `f(t) = a0 + t*a1 + t^2*a2`.
// TODO: make numerically stable
pub fn quadratic_real_roots(a0: f64, a1: f64, a2: f64) -> [Option<f64>; 2] {
	let radicand = a1 * a1 - 4.0 * a2 * a0;
	if radicand < 0.0 {
		return [None, None];
	}
	[Some((-a1 + radicand.sqrt()) / (2.0 * a2)), Some((-a1 - radicand.sqrt()) / (2.0 * a2))]
}

/// A quadratic bezier can be written `x = p0 + t*p1 + t^2*p2`, where `x`, `p0`, `p1`, and `p2` are vectors.
/// This function returns `[p0, p1, p2]`.
pub fn quadratic_bezier_coefficients(quad: &QuadBez) -> [Vec2; 3] {
	let p0 = quad.p0.to_vec2();
	let p1 = quad.p1.to_vec2();
	let p2 = quad.p2.to_vec2();
	let c0 = p0;
	let c1 = -2.0 * p0 + 2.0 * p1;
	let c2 = p0 - 2.0 * p1 + p2;
	[c0, c1, c2]
}

/// Returns the root to the linear equation: `f(t) = a0 + t*a1`.
pub fn linear_root(a0: f64, a1: f64) -> Option<f64> {
	if a1 == 0.0 {
		return None;
	}
	if a1.is_infinite() {
		return Some(a0);
	}
	Some(-a0 / a1)
}

/// A line can be written `x = p0 + t*p1`, where `x`, `p0` and `p1` are vectors.
/// Returns `[p0, p1]`.
pub fn linear_bezier_coefficients(line: &Line) -> [Vec2; 2] {
	let p0 = line.p0.to_vec2();
	let p1 = line.p1.to_vec2();
	[p0, p1 - p0]
}

/// Returns `true` if rectangles overlap, even if either rectangle has 0 area.
/// Uses `kurbo::Rect{x0, y0, x1, y1}` where `x0 <= x1` and `y0 <= y1`.
pub fn overlap(a: &Rect, b: &Rect) -> bool {
	a.x0 <= b.x1 && a.y0 <= b.y1 && b.x0 <= a.x1 && b.y0 <= a.y1
}

/// Tests if a `t` value belongs to `[0.0, 1.0)`.
/// Uses [crate::consts::F64PRECISE] to allow a slightly larger range of values.
pub fn valid_t(t: f64) -> bool {
	t > -F64PRECISE && t < (1.0 - F64PRECISE)
}

/// Each of these tests have been visually, but not mathematically, verified.
/// These tests are all ignored because each test looks for exact floating point comparisons, so isn't tolerant to small adjustments in the algorithm.
mod tests {
	// These imports are used in the tests which are #[ignore]
	#[allow(unused_imports)]
	use super::*;
	#[allow(unused_imports)]
	use crate::boolean_ops::point_on_curve;
	#[allow(unused_imports)]
	use std::{fs::File, io::Write};

	/// Two intersect points, on different `PathSegs`.
	#[ignore]
	#[test]
	fn curve_intersection_basic() {
		let a =
			BezPath::from_svg("M-739.7999877929688 -50.89999999999998L-676.7999877929688 -50.89999999999998L-676.7999877929688 27.100000000000023L-739.7999877929688 27.100000000000023Z").expect("");
		let b = BezPath::from_svg("M-649.2999877929688 72.10000000000002L-694.7999877929688 72.10000000000002L-694.7999877929688 0.8222196224152754L-649.2999877929688 0.8222196224152754Z").expect("");
		let expected = [
			Intersect {
				point: Point::new(-676.7999877929688, 0.8222196224152754),
				t_a: 0.6631053797745545,
				t_b: 0.3956043956043956,
				a_seg_index: 1,
				b_seg_index: 2,
				quality: 0.0,
			},
			Intersect {
				point: Point::new(-694.7999877929688, 27.10000000000003),
				t_a: 0.2857142857142857,
				t_b: 0.6313327906904278,
				a_seg_index: 2,
				b_seg_index: 1,
				quality: 0.0,
			},
		];
		let result = intersections(&a, &b);
		assert_eq!(expected.len(), result.len());
		assert!(expected.iter().zip(result.iter()).fold(true, |equal, (a, b)| equal && a == b));

		let a =
			BezPath::from_svg("M-663.1000244140627 -549.4740810512067C-663.1000244140627 -516.8197385387762 -690.6345122994688 -490.3481636282921 -724.6000244140627 -490.3481636282921C-758.5655365286565 -490.3481636282921 -786.1000244140627 -516.8197385387762 -786.1000244140627 -549.4740810512067C-786.1000244140627 -582.128423563637 -758.5655365286565 -608.5999984741211 -724.6000244140627 -608.5999984741211C-690.6345122994688 -608.5999984741211 -663.1000244140627 -582.128423563637 -663.1000244140627 -549.4740810512067").expect("");
		let b = BezPath::from_svg("M-834.7843084184785 -566.2292363273158C-834.7843084184785 -597.2326143708982 -805.749982642916 -622.3658181414634 -769.9343267290242 -622.3658181414634C-734.1186708151323 -622.3658181414634 -705.0843450395697 -597.2326143708982 -705.0843450395697 -566.2292363273158C-705.0843450395697 -535.2258582837335 -734.1186708151323 -510.0926545131682 -769.9343267290242 -510.0926545131682C-805.749982642916 -510.0926545131682 -834.7843084184785 -535.2258582837334 -834.7843084184785 -566.2292363273158").expect("");
		let expected = [
			Intersect {
				point: Point::new(-770.4753350264828, -510.09456728384305),
				t_a: 0.5368149286026136,
				t_b: 0.005039955230097687,
				a_seg_index: 1,
				b_seg_index: 3,
				quality: 0.0,
			},
			Intersect {
				point: Point::new(-727.3175070060661, -608.5433117814998),
				t_a: 0.9731908875121124,
				t_b: 0.45548363569548905,
				a_seg_index: 2,
				b_seg_index: 1,
				quality: 0.0,
			},
		];
		let result = intersections(&a, &b);
		assert_eq!(expected.len(), result.len());
		assert!(expected.iter().zip(result.iter()).fold(true, |equal, (a, b)| equal && a == b));

		let a =
			BezPath::from_svg("M-421.6225245705596 -963.1740648809906L-446.65763791855386 -1011.5169335848782L-496.72786461454245 -1011.5169335848782L-521.7629779625368 -963.1740648809906L-496.7278646145425 -914.831196177103L-446.6576379185539 -914.831196177103Z").expect("");
		let b = BezPath::from_svg("M-561.0072096972251 -1026.4026766566521L-502.81748678026577 -1026.4026766566521L-502.81748678026577 -945.8843391320225L-561.0072096972251 -945.8843391320225Z")
			.expect("");
		let expected = [
			Intersect {
				point: Point::new(-502.81748678026577, -999.757857413672),
				t_a: 0.24324324324304233,
				t_b: 0.33091616223235865,
				a_seg_index: 2,
				b_seg_index: 1,
				quality: 0.0,
			},
			Intersect {
				point: Point::new(-512.8092221038916, -945.8843391320225),
				t_a: 0.35764790573087535,
				t_b: 0.1717096219530834,
				a_seg_index: 3,
				b_seg_index: 2,
				quality: 0.0,
			},
		];
		let result = intersections(&a, &b);
		assert_eq!(expected.len(), result.len());
		assert!(expected.iter().zip(result.iter()).fold(true, |equal, (a, b)| equal && a == b));
	}

	/// Intersect points at ends of `PathSeg`s.
	#[ignore]
	#[test]
	fn curve_intersection_seg_edges() {
		let a =
			BezPath::from_svg("M-355.41190151646936 -204.93220299904385C-355.41190151646936 -164.32790664074417 -389.9224217662629 -131.4116207799262 -432.4933059063151 -131.4116207799262C-475.06419004636723 -131.4116207799262 -509.5747102961608 -164.32790664074417 -509.5747102961608 -204.93220299904382C-509.5747102961608 -245.53649935734347 -475.06419004636723 -278.45278521816147 -432.4933059063151 -278.45278521816147C-389.9224217662629 -278.45278521816147 -355.41190151646936 -245.5364993573435 -355.41190151646936 -204.93220299904385").expect("");
		let b = BezPath::from_svg("M-450.7808181070286 -146.42509665727817C-450.7808181070286 -185.38383768558714 -421.2406499166092 -216.96613737992166 -384.8010057614469 -216.96613737992166C-348.3613616062847 -216.96613737992166 -318.82119341586525 -185.38383768558714 -318.82119341586525 -146.4250966572782C-318.82119341586525 -107.46635562896924 -348.3613616062846 -75.88405593463473 -384.8010057614469 -75.88405593463472C-421.2406499166092 -75.8840559346347 -450.78081810702855 -107.46635562896921 -450.7808181070286 -146.42509665727817").expect("");
		let expected = [
			Intersect {
				point: Point::new(-449.629331039312, -133.2349088577284),
				t_a: 0.1383488820074267,
				t_b: 0.8842879656175459,
				a_seg_index: 1,
				b_seg_index: 3,
				quality: 0.00000000000002842170943040401,
			},
			Intersect {
				point: Point::new(-355.5702650533912, -209.683276560014),
				t_a: 0.9606918211578568,
				t_b: 0.28804943846673475,
				a_seg_index: 3,
				b_seg_index: 1,
				quality: 0.0,
			},
		];
		let result = intersections(&a, &b);
		assert_eq!(expected.len(), result.len());
		assert!(expected.iter().zip(result.iter()).fold(true, |equal, (a, b)| equal && a == b));
	}

	#[test]
	#[ignore]
	fn cubic_roots_intersection() {
		let roots = cubic_real_roots(1.5, 1.1, 3.6, 1.0);
		assert_eq!(roots.iter().filter_map(|r| *r).last().unwrap(), -3.4063481215142195);

		let roots = cubic_real_roots(-7.1, 1.1, 3.6, 1.0);
		assert_eq!(roots.iter().filter_map(|r| *r).last().unwrap(), 1.115909058984805);

		let roots = cubic_real_roots(-7.1, -9.5, -4.6, 1.0);
		assert_eq!(roots.iter().filter_map(|r| *r).last().unwrap(), 6.289837710873103);

		let roots = cubic_real_roots(-1.5, -3.3, 1.6, 1.0);
		assert_eq!(roots, [Some(1.4330896870185468), Some(-2.636017358627879), Some(-0.3970723283906693)]);

		// TODO: 3 real root case
		// for root in roots {
		// 	if let Some(num) = root {
		// 		print!("{:.32}", num);
		// 	}
		// }
	}

	#[test]
	#[ignore]
	fn test_colinear() {
		let p1 = Point { x: 0.0001, y: 3.0002 };
		let p2 = Point { x: 0.029, y: 3.058 };
		let p3 = Point { x: 100.237, y: 203.474 };
		let p4 = Point { x: 720.297, y: 1443.594 };
		assert!(colinear(&[&p1, &p2, &p3, &p4]));
	}

	#[test]
	#[ignore]
	fn test_point_t_value() {
		let vertical_line = Line::new(Point::new(0.0, -10.0), Point::new(0.0, 10.0));
		let t_value = point_t_value(&PathSeg::Line(vertical_line), &Point::new(0.0, 1.0));
		assert_eq!(t_value.unwrap(), 0.55);
	}

	#[test]
	#[ignore]
	fn test_kurbo_eval_stability() {
		let mut test_results = File::create("..\\target\\debug\\test_kurbo_eval_results.txt").expect("");
		let test_curve = BezPath::from_svg("M-355.41190151646936 -204.93220299904385C-355.41190151646936 -164.32790664074417 -389.9224217662629 -131.4116207799262 -432.4933059063151 -131.4116207799262C-475.06419004636723 -131.4116207799262 -509.5747102961608 -164.32790664074417 -509.5747102961608 -204.93220299904382C-509.5747102961608 -245.53649935734347 -475.06419004636723 -278.45278521816147 -432.4933059063151 -278.45278521816147C-389.9224217662629 -278.45278521816147 -355.41190151646936 -245.5364993573435 -355.41190151646936 -204.93220299904385").expect("").segments().next().unwrap();
		let mut val = 0.0;
		while val < 0.0 + 1000000.0 * f64::EPSILON {
			writeln!(&mut test_results, "{:?}, {:?}", val, test_curve.eval(val).x).expect("");
			val += f64::EPSILON;
		}
	}

	#[test]
	#[ignore]
	fn test_quality_stability() {
		let mut test_results = File::create("..\\target\\debug\\test_quality_results.txt").expect("");
		let mut val = 0.0;

		while val < 0.0 + 1000000.0 * f64::EPSILON {
			let a = Line::new(Point::new(0.0, 0.0), Point::new(1.0 + val + val, 1.0 + val + val));
			let b = Line::new(Point::new(0.0, 1.0 + val + val), Point::new(1.0 + val + val, 0.0));
			let guess = Intersect::from((Point::new(0.5 + val, 0.5 + val), 0.5, 0.5));

			writeln!(&mut test_results, "{:?}, {:?}", val, guess_quality(&PathSeg::Line(a), &PathSeg::Line(b), &guess)).expect("");
			val += f64::EPSILON;
		}
	}

	#[test]
	#[ignore]
	fn test_quality_cubic_stability() {
		let mut test_results = File::create("..\\target\\debug\\test_quality_cubic_results.txt").expect("");
		let mut val = 0.0;

		while val < 0.0 + 1000000.0 * f64::EPSILON {
			let a = PathSeg::Cubic(CubicBez::new(
				Point::new(0.0 + val, 0.0),
				Point::new(0.25 + val, 0.661437827766),
				Point::new(0.75 + val, 0.968245836552),
				Point::new(1.0 + val, 1.0),
			));
			let b = PathSeg::Cubic(CubicBez::new(
				Point::new(0.0, 1.0),
				Point::new(0.25, 0.968245836552),
				Point::new(0.75, 0.661437827766),
				Point::new(1.0, 0.0),
			));
			let guess = Intersect::from((Point::new(0.5 + val, 0.5 + val), 0.5 + val, 0.5 + val));

			writeln!(&mut test_results, "{:?}, {:?}", val, guess_quality(&a, &b, &guess)).expect("");
			val += f64::EPSILON;
		}
	}

	#[test]
	#[ignore]
	fn test_line_intersection_stability() {
		let mut test_results = File::create("..\\target\\debug\\test_line_intersect_results.txt").expect("");
		let mut val = 0.0;

		while val < 0.0 + 1000000.0 * f64::EPSILON {
			let a = Line::new(Point::new(0.0 + val, 0.0), Point::new(1.0 + val, 1.0));
			let b = Line::new(Point::new(0.0, 1.0), Point::new(1.0, 0.0));

			let line_intersection = line_intersection(&a, &b).unwrap();
			writeln!(
				&mut test_results,
				"{:?}, {:?}, {:?}, {:?}",
				val,
				line_intersection.t_a,
				line_intersection.point.x,
				guess_quality(&PathSeg::Line(a), &PathSeg::Line(b), &line_intersection)
			)
			.expect("");
			val += f64::EPSILON;
		}
	}

	#[test]
	#[ignore]
	fn test_line_intersection_cancellation() {
		let mut test_results = File::create("..\\target\\debug\\test_line_intersection_cancellation_results.txt").expect("");
		let val = 1.0;
		let mut theta = F64PRECISE;

		while theta < std::f64::consts::FRAC_PI_2 - 0.1 {
			let a = Line::new(Point::new(1.0, 1.0), Point::new(1.0 + val, 1.0 + val));
			let b = Line::new(Point::new(1.0, 1.0 + val * f64::cos(theta)), Point::new(1.0 + val, 1.0 + val * f64::sin(theta)));

			let line_intersection = line_intersection(&a, &b).unwrap();
			writeln!(
				&mut test_results,
				"{:?}, {:?}, {:?}, {:?}",
				theta, line_intersection.t_a, line_intersection.t_b, line_intersection.point.x,
			)
			.expect("");
			theta += f64::powf(2.0, 20.0) * F64LOOSE;
		}
	}

	#[test]
	#[ignore]
	fn test_intersections_stability() {
		let mut test_results_intersection = File::create("..\\target\\debug\\test_curve_intersections_multi_results.txt").expect("");
		let mut val = 0.0;

		while val < 0.0 + 1000000.0 * f64::EPSILON {
			let a = PathSeg::Cubic(CubicBez::new(
				Point::new(0.0 + val, 0.0),
				Point::new(0.25 + val, 0.661437827766),
				Point::new(0.75 + val, 0.968245836552),
				Point::new(1.0 + val, 1.0),
			));
			let b = PathSeg::Cubic(CubicBez::new(
				Point::new(0.0, 1.0),
				Point::new(0.25, 0.968245836552),
				Point::new(0.75, 0.661437827766),
				Point::new(1.0, 0.0),
			));
			let aex = SubCurve::subcurve_extrema(&a);
			let bex = SubCurve::subcurve_extrema(&b);
			let a_sub = SubCurve::new(&a, &aex);
			let b_sub = SubCurve::new(&b, &bex);
			let mut intersections = Vec::new();
			path_intersections(&a_sub, &b_sub, &mut intersections);

			writeln!(
				&mut test_results_intersection,
				"{:?}, {:?}, {:?}, {:?}",
				val,
				intersections.first().unwrap().point.x,
				intersections.first().unwrap().quality,
				intersections.first().unwrap().t_a
			)
			.expect("");

			val += f64::EPSILON;
		}
	}

	#[ignore]
	#[test]
	fn test_test_dir() {
		use std::env::current_dir;
		println!("{:?}", current_dir());
	}
}
