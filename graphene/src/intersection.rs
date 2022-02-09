use core::{panic, slice::SlicePattern};
use std::{ops::Mul, path::Path};

use crate::{
	boolean_ops::split_path_seg,
	boolean_ops::subdivide_path_seg,
	consts::{CURVE_FIDELITY, F64PRECISION},
};
use glam::{DAffine2, DMat2, DVec2};
use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveExtrema, PathSeg, Point, QuadBez, Rect, Shape, Vec2};

#[derive(Debug, Clone, Default, Copy)]
pub struct Quad([DVec2; 4]);

impl Quad {
	pub fn from_box(bbox: [DVec2; 2]) -> Self {
		let size = bbox[1] - bbox[0];
		Self([bbox[0], bbox[0] + size * DVec2::X, bbox[1], bbox[0] + size * DVec2::Y])
	}

	pub fn lines(&self) -> [Line; 4] {
		[
			Line::new(to_point(self.0[0]), to_point(self.0[1])),
			Line::new(to_point(self.0[1]), to_point(self.0[2])),
			Line::new(to_point(self.0[2]), to_point(self.0[3])),
			Line::new(to_point(self.0[3]), to_point(self.0[0])),
		]
	}

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

pub fn intersect_quad_bez_path(quad: Quad, shape: &BezPath, filled: bool) -> bool {
	let mut shape = shape.clone();
	// for filled shapes act like shape was closed even if it isn't
	if filled && shape.elements().last() != Some(&kurbo::PathEl::ClosePath) {
		shape.close_path();
	}

	// check if outlines intersect
	if shape.segments().any(|path_segment| quad.lines().iter().any(|line| !path_segment.intersect_line(*line).is_empty())) {
		return true;
	}
	// check if selection is entirely within the shape
	if filled && shape.contains(to_point(quad.0[0])) {
		return true;
	}

	// check if shape is entirely within selection
	get_arbitrary_point_on_path(&shape).map(|shape_point| quad.path().contains(shape_point)).unwrap_or_default()
}

pub fn get_arbitrary_point_on_path(path: &BezPath) -> Option<Point> {
	path.segments().next().map(|seg| match seg {
		PathSeg::Line(line) => line.p0,
		PathSeg::Quad(quad) => quad.p0,
		PathSeg::Cubic(cubic) => cubic.p0,
	})
}

/// \/                               \/
/// Bezier Curve Intersection algorithm
/// \/                               \/

/// each intersection has two curves, which are distinguished between using this enum
/// TODO: refactor so actual curve data and Origin aren't separate
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

///TODO: remove quality from Intersect
#[derive(Debug, PartialEq)]
pub struct Intersect {
	pub point: Point,
	pub t_a: f64,
	pub t_b: f64,
	pub a_seg_idx: i32,
	pub b_seg_idx: i32,
	pub quality: f64,
}

impl Intersect {
	pub fn new(point: Point, t_a: f64, t_b: f64, a_seg_idx: i32, b_seg_idx: i32) -> Self {
		Intersect {
			point,
			t_a,
			t_b,
			a_seg_idx,
			b_seg_idx,
			quality: -1.0,
		}
	}

	pub fn add_idx(&mut self, a_idx: i32, b_idx: i32) {
		self.a_seg_idx = a_idx;
		self.b_seg_idx = b_idx;
	}

	pub fn seg_idx(&self, o: Origin) -> i32 {
		match o {
			Origin::Alpha => self.a_seg_idx,
			Origin::Beta => self.b_seg_idx,
		}
	}

	pub fn t_val(&self, o: Origin) -> f64 {
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
			a_seg_idx: 0,
			b_seg_idx: 0,
			quality: 0.0,
		}
	}
}

struct SubCurve<'a> {
	pub curve: &'a PathSeg,
	pub start_t: f64,
	pub end_t: f64,
	local: [Point; 2], // local endpoints
	pub extrema: &'a Vec<(Point, f64)>,
}

impl<'a> SubCurve<'a> {
	pub fn new(parent: &'a PathSeg, extrema: &'a Vec<(Point, f64)>) -> Self {
		SubCurve {
			curve: parent,
			start_t: 0.0,
			end_t: 1.0,
			local: [parent.eval(0.0), parent.eval(1.0)],
			extrema,
		}
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

	/// split subcurve at t, as though the subcurve is a bezier curve, where t is a value between 0.0 and 1.0
	fn split(&self, t: f64) -> (SubCurve, SubCurve) {
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

// TODO use the cool algorithm described in the paper below
// * https://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.99.9678&rep=rep1&type=pdf
// - Bezier Curve Intersection Algorithm
// - TODO: How does f64 precision effect the algorithm?
// - TODO: profile algorithm
// - Bug: intersections of "perfectly aligned" line or curve
// - If the algorithm is rewritten to be non-recursive it can be restructured to be more breadth first then depth first
// - - test for overlapping curves by splitting the curves
// - Behavior: deep recursion could result in stack overflow
// - Improvement: intersections on the end of segments
// - Improvement: more adaptive way to decide when "close enough"
// - Optimization: any extra copying happening?

fn path_intersections(a: &SubCurve, b: &SubCurve, mut recursion: f64, intersections: &mut Vec<Intersect>) {
	if overlap(&a.bounding_box(), &b.bounding_box()) {
		if let (PathSeg::Line(line), _) = (a.curve, b) {
			line_curve_intersections(line, b.curve, true, |a, b| valid_t(a) && valid_t(b), intersections);
			return;
		}
		if let (_, PathSeg::Line(line)) = (a, b.curve) {
			line_curve_intersections(line, a.curve, false, |a, b| valid_t(a) && valid_t(b), intersections);
			return;
		}
		// we are close enough to try linear approximation
		if recursion < (1 << 10) as f64 {
			if let Some(mut cross) = line_intersection(&Line { p0: a.start(), p1: a.end() }, &Line { p0: b.start(), p1: b.end() }) {
				// intersection t_value equals the recursive t_value + interpolated intersection value
				cross.t_a = a.start_t + cross.t_a * recursion;
				cross.t_b = b.start_t + cross.t_b * recursion;
				cross.quality = guess_quality(a.curve, b.curve, &cross);

				// log::debug!("checking: {:?}", cross.quality);
				if cross.quality <= CURVE_FIDELITY {
					intersections.push(cross);
					return;
				}
				// Eventually the points in the curve become to close together to split the curve meaningfully
				// Also provides a base case and prevents infinite recursion
				if a.available_precision() <= F64PRECISION || b.available_precision() <= F64PRECISION {
					log::debug!("precision reached");
					intersections.push(cross);
					return;
				}
			}

			// Alternate base case
			// Note: may occur for the less forgiving side of an PathSeg endpoint intersect
			if a.available_precision() <= F64PRECISION || b.available_precision() <= F64PRECISION {
				log::debug!("precision reached without finding intersect");
				return;
			}
		}
		recursion /= 2.0;
		let (a1, a2) = a.split(0.5);
		let (b1, b2) = b.split(0.5);
		path_intersections(&a1, &b1, recursion, intersections);
		path_intersections(&a1, &b2, recursion, intersections);
		path_intersections(&a2, &b1, recursion, intersections);
		path_intersections(&a2, &b2, recursion, intersections);
	}
}

pub fn line_curve_intersections<F>(line: &Line, curve: &PathSeg, is_line_a: bool, t_validate: F, intersections: &mut Vec<Intersect>)
where
	F: Fn(f64, f64) -> bool,
{
	if let (line, PathSeg::Line(line2)) = (line, curve) {
		if let Some(cross) = line_intersection(line, line2) {
			if t_validate(cross.t_a, cross.t_b) {
				intersections.push(cross);
			}
		}
	} else {
		// forced to construct a vec here because match arms must return same type, and E0716
		let roots = match curve {
			PathSeg::Quad(quad) => Vec::from(quad_line_intersect(line, quad)),
			PathSeg::Cubic(cubic) => Vec::from(cubic_line_intersect(line, cubic)),
			_ => vec![], //should never occur
		};
		intersections.extend(
			roots
				.iter()
				.filter_map(|time_option| {
					if let Some(time) = time_option {
						let point = match curve {
							PathSeg::Cubic(cubic) => cubic.eval(*time),
							PathSeg::Quad(quad) => quad.eval(*time),
							_ => Point::new(0.0, 0.0), //should never occur
						};
						// the intersection point should be on the line, unless FP math error produces bad results
						let line_time = line_t_value(line, &point).unwrap();
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

/// Optimization: inline? maybe...
/// For quality Q in the worst case, the point on curve "a" corresponding to "guess" is distance Q from the point on curve "b"
fn guess_quality(a: &PathSeg, b: &PathSeg, guess: &Intersect) -> f64 {
	let at_a = b.eval(guess.t_b);
	let at_b = a.eval(guess.t_a);
	at_a.distance(guess.point) + at_b.distance(guess.point)
}

///
pub fn same_curve_intersections(a: &PathSeg, b: &PathSeg) -> [Option<Intersect>; 2] {
	let mut b_on_a: Vec<Option<f64>> = [point_t_value(a, &b.start()), point_t_value(a, &b.end())].into_iter().collect();
	let mut a_on_b: Vec<Option<f64>> = [point_t_value(b, &a.start()), point_t_value(b, &a.end())].into_iter().collect();
	// I think, but have not mathematically shown, that if a and b are parts of the same curve then b_on_a and a_on_b should together have no more than three non-None elements. Which occurs when a or b is a cubic bezier which crosses itself
	let b_on_a_not_None = b_on_a.iter().filter_map(|o| *o).count();
	let a_on_b_not_None = a_on_b.iter().filter_map(|o| o).count();
	match b_on_a.len() + a_on_b.len() {
		2 | 3 => {
			let to_compare = if b_on_a.len() == 2 {
				b_on_a.sort_by(|val1, val2| (val1).partial_cmp(val2).unwrap_or(std::cmp::Ordering::Less));
				(*b, subdivide_path_seg(a, &mut b_on_a.iter().filter_map(|o| *o).collect::<Vec<f64>>().as_slice())[1].unwrap())
			} else if a_on_b.len() == 2 {
				a_on_b.sort_by(|val1, val2| (val1).partial_cmp(val2).unwrap_or(std::cmp::Ordering::Less));
				(*a, subdivide_path_seg(a, &mut a_on_b.iter().filter_map(|o| *o).collect::<Vec<f64>>().as_slice())[1].unwrap())
			} else {
				(
					match (b_on_a[0], b_on_a[1], a_on_b[0], a_on_b[1]) {
						(None, Some(_), _, Some(t_val)) | (None, Some(_), Some(t_val), _) => split_path_seg(b, t_val).1.unwrap(),
						(Some(_), None, _, Some(t_val)) | (Some(_), None, Some(t_val), _) => split_path_seg(b, t_val).0.unwrap(),
						_ => panic!(),
					},
					match (a_on_b[0], a_on_b[1], b_on_a[0], b_on_a[1]) {
						(None, Some(_), _, Some(t_val)) | (None, Some(_), Some(t_val), _) => split_path_seg(a, t_val).1.unwrap(),
						(Some(_), None, _, Some(t_val)) | (Some(_), None, Some(t_val), _) => split_path_seg(a, t_val).0.unwrap(),
						_ => panic!(),
					},
				)
			};

			[None, None]
		}
		_ => [None, None],
	}
	[None, None]
}

/// if p in on pathseg a, returns Some(t_value) for p
/// in the edge case where the path crosses itself, and p is at the cross, the first t_value found (but not necessarily the smallest) is returned
pub fn point_t_value(a: &PathSeg, p: &Point) -> Option<f64> {
	match a {
		PathSeg::Line(line) => line_t_value(line, p),
		PathSeg::Quad(quad) => {
			let [mut p0, p1, p2] = quadratic_bezier_coefficients(quad);
			p0 -= p.to_vec2();
			let x_roots = quadratic_real_roots(p0.x, p1.x, p2.x);
			quadratic_real_roots(p0.y, p1.y, p2.y)
				.into_iter()
				.find(|yt_option| x_roots.iter().any(|xt_option| yt_option.is_some() && xt_option.is_some() && (yt_option.unwrap() == xt_option.unwrap())))
				.flatten()
		}
		PathSeg::Cubic(cubic) => {
			let [mut p0, p1, p2, p3] = cubic_bezier_coefficients(cubic);
			p0 -= p.to_vec2();
			let x_roots = cubic_real_roots(p0.x, p1.x, p2.x, p3.x);
			cubic_real_roots(p0.y, p1.y, p2.y, p3.y)
				.into_iter()
				.find(|yt_option| x_roots.iter().any(|xt_option| yt_option.is_some() && xt_option.is_some() && (yt_option.unwrap() == xt_option.unwrap())))
				.flatten()
		}
	}
}

pub fn intersections(a: &BezPath, b: &BezPath) -> Vec<Intersect> {
	// print out paths for testing
	// log::info!("{:?}", a.to_svg());
	// log::info!("{:?}", b.to_svg());

	let mut intersections: Vec<Intersect> = Vec::new();
	// there is some duplicate computation of b_extrema here, but I doubt it's significant
	a.segments().enumerate().for_each(|(a_idx, a_seg)| {
		// extrema at endpoints should not be included here as they must be calculated for each subcurve
		// Note: below filtering may filter out extrema near the endpoints
		let a_extrema = a_seg
			.extrema()
			.iter()
			.filter_map(|t| if *t > F64PRECISION && *t < 1.0 - F64PRECISION { Some((a_seg.eval(*t), *t)) } else { None })
			.collect();
		b.segments().enumerate().for_each(|(b_idx, b_seg)| {
			let b_extrema = b_seg
				.extrema()
				.iter()
				.filter_map(|t| if *t > F64PRECISION && *t < 1.0 - F64PRECISION { Some((b_seg.eval(*t), *t)) } else { None })
				.collect();
			let mut intersects = Vec::new();
			path_intersections(&SubCurve::new(&a_seg, &a_extrema), &SubCurve::new(&b_seg, &b_extrema), 1.0, &mut intersects);
			for mut path_intersection in intersects {
				intersections.push({
					path_intersection.add_idx(a_idx.try_into().unwrap(), b_idx.try_into().unwrap());
					path_intersection
				});
			}
		})
	});

	// print out result for testing
	// log::info!("{:?}", intersections);

	intersections
}

/// returns intersection point as if lines extended forever
pub fn line_intersect_point(a: &Line, b: &Line) -> Option<Point> {
	line_intersection_unchecked(a, b).map(|intersect| intersect.point)
}

/// returns intersection point and t values, treating lines as Bezier curves
pub fn line_intersection(a: &Line, b: &Line) -> Option<Intersect> {
	if let Some(intersect) = line_intersection_unchecked(a, b) {
		if valid_t(intersect.t_a) && valid_t(intersect.t_b) {
			Some(intersect)
		} else {
			None
		}
	} else {
		None
	}
}

/// returns intersection point and t values, treating lines as rays
pub fn line_intersection_unchecked(a: &Line, b: &Line) -> Option<Intersect> {
	let slopes = DMat2::from_cols_array(&[(b.p1 - b.p0).x, (b.p1 - b.p0).y, (a.p0 - a.p1).x, (a.p0 - a.p1).y]);
	if slopes.determinant() == 0.0 {
		return None;
	}
	let t_values = slopes.inverse() * DVec2::new((a.p0 - b.p0).x, (a.p0 - b.p0).y);
	Some(Intersect::from((b.eval(t_values[0]), t_values[1], t_values[0])))
}

///if p in on line a, returns Some(t_value) for p
pub fn line_t_value(a: &Line, p: &Point) -> Option<f64> {
	let from_x = (p.x - a.p0.x) / (a.p1.x - a.p0.x);
	let from_y = (p.y - a.p0.y) / (a.p1.y - a.p0.y);
	if !from_x.is_normal() {
		if !from_y.is_normal() {
			None
		} else {
			Some(from_y)
		}
	} else if !from_y.is_normal() || from_x == from_y {
		Some(from_x)
	} else {
		None
	}
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

/// return real roots to cubic equation: f(t) = a0 + t*a1 + t^2*a2 + t^3*a3
/// this function uses the Cardano-Viete and Numerical Recipes algorithm, found here: https://quarticequations.com/Cubic.pdf
pub fn cubic_real_roots(mut a0: f64, mut a1: f64, mut a2: f64, a3: f64) -> [Option<f64>; 3] {
	use std::f64::consts::FRAC_PI_3 as PI_3;
	a0 /= a3;
	a1 /= a3;
	a2 /= a3;
	let q: f64 = a1 / 3.0 - a2 * a2 / 9.0;
	let r: f64 = (a1 * a2 - 3.0 * a0) / 6.0 - a2 * a2 * a2 / 27.0;
	let r2_q3 = r * r + q * q * q;
	if r2_q3 > 0.0 {
		#[allow(non_snake_case)] // allow name 'A' for consistency with algorithm
		let A = (r.abs() + r2_q3.sqrt()).cbrt();
		let t1 = match r {
			r if r >= 0.0 => A - q / A,
			r if r < 0.0 => q / A - A,
			_ => 0.0, // should never occur
		};
		[Some(t1 - a2 / 3.0), None, None]
	} else {
		let phi = match q > -F64PRECISION && q < F64PRECISION {
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

/// a quadratic bezier can be written x = p0 + t*p1 + t^2*p2 + t^3*p3, where x, p0, p1, p2, and p3 are vectors
/// this function returns [p0, p1, p2, p3]
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

/// return real roots to quadratic equation: f(t) = a0 + t*a1 + t^2*a2
pub fn quadratic_real_roots(a0: f64, a1: f64, a2: f64) -> [Option<f64>; 2] {
	let radicand = a1 * a1 - 4.0 * a2 * a0;
	if radicand < 0.0 {
		return [None, None];
	}
	[Some((-a1 + radicand.sqrt()) / (2.0 * a2)), Some((-a1 - radicand.sqrt()) / (2.0 * a2))]
}

/// a quadratic bezier can be written x = p0 + t*p1 + t^2*p2, where x, p0, p1, and p2 are vectors
/// this function returns [p0, p1, p2]
pub fn quadratic_bezier_coefficients(quad: &QuadBez) -> [Vec2; 3] {
	let p0 = quad.p0.to_vec2();
	let p1 = quad.p1.to_vec2();
	let p2 = quad.p2.to_vec2();
	let c0 = p0;
	let c1 = -2.0 * p0 + 2.0 * p1;
	let c2 = p0 - 2.0 * p1 + p2;
	[c0, c1, c2]
}

// return root to linear equation: f(t) = a0 + t*a1
pub fn linear_root(a0: f64, a1: f64) -> [Option<f64>; 1] {
	if a1 == 0.0 {
		return [None];
	}
	[Some(-a0 / a1)]
}

/// returns true if rectangles overlap, even if either rectangle has 0 area
/// uses kurbo::Rect{x0, y0, x1, y1} where x0 <= x1 and y0 <= y1
pub fn overlap(a: &Rect, b: &Rect) -> bool {
	a.x0 <= b.x1 && a.y0 <= b.y1 && b.x0 <= a.x1 && b.y0 <= a.y1
}

/// tests if a t value belongs to [0.0, 1.0)
/// uses F64PRECISION to allow a slightly larger range of values
pub fn valid_t(t: f64) -> bool {
	t > -F64PRECISION && t < 1.0
}

/// each of these tests has been visually, but not mathematically verified
/// These tests are all ignored because each test looks for exact floating point comparisons, so isn't flexible to small adjustments in the algorithm
mod tests {
	#[allow(unused_imports)] // this import is used
	use super::*;

	/// two intersect points, on different PathSegs
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
				a_seg_idx: 1,
				b_seg_idx: 2,
				quality: 0.0,
			},
			Intersect {
				point: Point::new(-694.7999877929688, 27.10000000000003),
				t_a: 0.2857142857142857,
				t_b: 0.6313327906904278,
				a_seg_idx: 2,
				b_seg_idx: 1,
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
				a_seg_idx: 1,
				b_seg_idx: 3,
				quality: 0.0,
			},
			Intersect {
				point: Point::new(-727.3175070060661, -608.5433117814998),
				t_a: 0.9731908875121124,
				t_b: 0.45548363569548905,
				a_seg_idx: 2,
				b_seg_idx: 1,
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
				a_seg_idx: 2,
				b_seg_idx: 1,
				quality: 0.0,
			},
			Intersect {
				point: Point::new(-512.8092221038916, -945.8843391320225),
				t_a: 0.35764790573087535,
				t_b: 0.1717096219530834,
				a_seg_idx: 3,
				b_seg_idx: 2,
				quality: 0.0,
			},
		];
		let result = intersections(&a, &b);
		assert_eq!(expected.len(), result.len());
		assert!(expected.iter().zip(result.iter()).fold(true, |equal, (a, b)| equal && a == b));
	}

	/// intersect points at ends of PathSegs
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
				a_seg_idx: 1,
				b_seg_idx: 3,
				quality: 0.00000000000002842170943040401,
			},
			Intersect {
				point: Point::new(-355.5702650533912, -209.683276560014),
				t_a: 0.9606918211578568,
				t_b: 0.28804943846673475,
				a_seg_idx: 3,
				b_seg_idx: 1,
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

		//TODO 3 real root case
		// for root in roots {
		// 	if let Some(num) = root {
		// 		print!("{:.32}", num);
		// 	}
		// }
	}
}
