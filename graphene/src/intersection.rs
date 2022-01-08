use std::ops::Mul;

use glam::{DAffine2, DMat2, DVec2};
use kurbo::{BezPath, Line, ParamCurve, ParamCurveExtrema, PathSeg, Point, Rect, Shape};

pub const F64PRECISION: f64 = f64::EPSILON * 1000.0; // for f64 comparisons

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

pub fn intersect_quad_bez_path(quad: Quad, shape: &BezPath, closed: bool) -> bool {
	// check if outlines intersect
	if shape.segments().any(|path_segment| quad.lines().iter().any(|line| !path_segment.intersect_line(*line).is_empty())) {
		return true;
	}
	// check if selection is entirely within the shape
	if closed && shape.contains(to_point(quad.0[0])) {
		return true;
	}

	// check if shape is entirely within selection
	get_arbitrary_point_on_path(shape).map(|shape_point| quad.path().contains(shape_point)).unwrap_or_default()
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
/// TODO: refactor so actual curve data and Origin aren't seperate
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

pub struct Intersect {
	pub point: Point,
	pub t_a: f64,
	pub t_b: f64,
	pub a_seg_idx: usize,
	pub b_seg_idx: usize,
	pub quality: f64,
}

impl Intersect {
	pub fn add_idx(&mut self, a_idx: usize, b_idx: usize) {
		self.a_seg_idx = a_idx;
		self.b_seg_idx = b_idx;
	}

	pub fn seg_idx(&self, o: Origin) -> usize {
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
	local: [Point; 2], //local endpoints
	pub extrema: &'a Vec<Point>,
}

impl<'a> SubCurve<'a> {
	pub fn new(parent: &'a PathSeg, extrema: &'a Vec<Point>) -> Self {
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
		self.local.iter().chain(self.extrema.iter()).for_each(|p| {
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

	/// eval subcurve at t as though the subcurve is a bezier curve
	fn eval(&self, t: f64) -> Point {
		self.curve.eval(self.start_t + t * (self.end_t - self.start_t))
	}

	/// split subcurve at t, as though the subcurve is a bezier curve, where t is a value between 0.0 and 1.0
	fn split(&self, t: f64) -> (SubCurve, SubCurve) {
		let split_t = self.start_t + t * (self.end_t - self.start_t);
		(
			SubCurve {
				curve: self.curve,
				start_t: self.start_t,
				end_t: split_t,
				local: [self.curve.eval(self.start_t), self.curve.eval(split_t)],
				extrema: self.extrema,
			},
			SubCurve {
				curve: self.curve,
				start_t: split_t,
				end_t: self.end_t,
				local: [self.curve.eval(split_t), self.curve.eval(self.end_t)],
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

/**
Bezier Curve Intersection Algorithm
- TODO: How does f64 precision effect the algorithm?
- Bug: algorithm finds same intersection multiple times in same recursion path
- Bug: intersections of "perfectly alligned" line or curve
- Improvement: algorithm behavior when curves have very different sizes
- Improvement: more adapative way to decide when "close enough"
- Improvement: quality metric?
- Optimization: any extra copying happening?
- Optimization: how efficiently does std::Vec::append work?
- Optimization: specialized line/quad/cubic combination algorithms
*/
fn path_intersections(a: &SubCurve, b: &SubCurve, mut recursion: usize) -> Vec<Intersect> {
	let mut intersections = Vec::new();
	//special case
	if let (PathSeg::Line(line_a), PathSeg::Line(line_b)) = (a.curve, b.curve) {
		if let Some(cross) = line_intersection(line_a, line_b) {
			intersections.push(cross);
		}
	} else if overlap(&a.bounding_box(), &b.bounding_box()) {
		recursion += 1;
		// bail out!!, should instead bail out when we reach the precision limits of either shape
		// bail out before lshift with overflow
		if recursion == 32 {
			return intersections;
		}
		// base case, we are close enough to try linear approximation
		if recursion > 10 {
			//arbitrarily chosen limit
			if let Some(mut cross) = line_intersection(&Line { p0: a.start(), p1: a.end() }, &Line { p0: b.start(), p1: b.end() }) {
				// intersection t_value equals the recursive t_value + interpolated intersection value
				cross.t_a = a.start_t + cross.t_a / (1 << recursion) as f64;
				cross.t_b = b.start_t + cross.t_b / (1 << recursion) as f64;
				cross.quality = guess_quality(a.curve, b.curve, &cross);
				intersections.push(cross); //arbitrarily chosen threshold
				return intersections;
			}
			log::debug!("line no cross"); // some intersections end up here, sign that false positives are possible
		}
		let (a1, a2) = a.split(0.5);
		let (b1, b2) = b.split(0.5);
		intersections.append(&mut path_intersections(&a1, &b1, recursion));
		intersections.append(&mut path_intersections(&a1, &b2, recursion));
		intersections.append(&mut path_intersections(&a2, &b1, recursion));
		intersections.append(&mut path_intersections(&a2, &b2, recursion));
	}
	intersections
}

fn guess_quality(a: &PathSeg, b: &PathSeg, guess: &Intersect) -> f64 {
	let dist_a = guess.point - b.eval(guess.t_b);
	let dist_b = guess.point - a.eval(guess.t_a);
	// prevent division by 0
	return (2.0 / (1.0 + dist_a.x * dist_b.x * dist_a.y * dist_b.y)).abs();
}

pub fn intersections(a: &BezPath, b: &BezPath) -> Vec<Intersect> {
	let mut intersections: Vec<Intersect> = Vec::new();
	// there is some duplicate computation of b_extrema here, but i doubt it's significant
	a.segments().enumerate().for_each(|(a_idx, a_seg)| {
		// extrema at endpoints should not be included here as they must be calculated for each subcurve
		let a_extrema = a_seg
			.extrema()
			.iter()
			.filter_map(|t| if *t > F64PRECISION && *t < 1.0 - F64PRECISION { Some(a_seg.eval(*t)) } else { None })
			.collect();
		b.segments().enumerate().for_each(|(b_idx, b_seg)| {
			let b_extrema = b_seg
				.extrema()
				.iter()
				.filter_map(|t| if *t > F64PRECISION && *t < 1.0 - F64PRECISION { Some(b_seg.eval(*t)) } else { None })
				.collect();
			for mut path_intersection in path_intersections(&SubCurve::new(&a_seg, &a_extrema), &SubCurve::new(&b_seg, &b_extrema), 0) {
				intersections.push({
					path_intersection.add_idx(a_idx, b_idx);
					path_intersection
				});
			}
		})
	});
	intersections
}

pub fn intersection_candidates(a: &BezPath, b: &BezPath) -> Vec<(usize, usize)> {
	let mut intersections = Vec::new();

	a.segments().enumerate().for_each(|(a_idx, a_seg)| {
		b.segments().enumerate().for_each(|(b_idx, b_seg)| {
			if overlap(&<PathSeg as ParamCurveExtrema>::bounding_box(&a_seg), &<PathSeg as ParamCurveExtrema>::bounding_box(&b_seg)) {
				intersections.push((a_idx, b_idx));
			}
		})
	});
	intersections
}

/// returns intersection point as if lines extended forever
pub fn line_intersect_point(a: &Line, b: &Line) -> Option<Point> {
	let slopes = DMat2::from_cols_array(&[(b.p1 - b.p0).x, (b.p1 - b.p0).y, (a.p0 - a.p1).x, (a.p0 - a.p1).y]);
	if slopes.determinant() == 0.0 {
		return None;
	}
	let t_vals = slopes.inverse() * DVec2::new((a.p0 - b.p0).x, (a.p0 - b.p0).y);
	Some(b.eval(t_vals[0]))
}

/// returns intersection point and t values, treating lines as Bezier curves
pub fn line_intersection(a: &Line, b: &Line) -> Option<Intersect> {
	let slopes = DMat2::from_cols_array(&[(b.p1 - b.p0).x, (b.p1 - b.p0).y, (a.p0 - a.p1).x, (a.p0 - a.p1).y]);
	if slopes.determinant() == 0.0 {
		return None;
	}
	let t_vals = slopes.inverse() * DVec2::new((a.p0 - b.p0).x, (a.p0 - b.p0).y);
	if !valid_t(t_vals[0]) || !valid_t(t_vals[1]) {
		return None;
	}
	Some(Intersect::from((b.eval(t_vals[0]), t_vals[1], t_vals[0])))
}

/// returns true if rectangles overlap, even if either rectangle has 0 area
/// does using slices here cause a slowdown?
pub fn overlap(a: &Rect, b: &Rect) -> bool {
	fn in_range(n: f64, range: &[f64]) -> bool {
		n >= range[0] && n <= range[1]
	}
	fn in_range_e(n: f64, range: &[f64]) -> bool {
		n > range[0] && n < range[1]
	}
	(in_range(b.x0, &[a.x0, a.x1]) || in_range(b.x1, &[a.x0, a.x1]) || in_range_e(a.x0, &[b.x0, b.x1]) || in_range_e(a.x1, &[b.x0, b.x1]))
		&& (in_range(b.y0, &[a.y0, a.y1]) || in_range(b.y1, &[a.y0, a.y1]) || in_range_e(a.y0, &[b.y0, b.y1]) || in_range_e(a.y1, &[b.y0, b.y1]))
}

/// tests if a t value belongs to [0.0, 1.0]
/// uses F64PRECISION to allow a slightly larger range of values
fn valid_t(t: f64) -> bool {
	t > -F64PRECISION && t < 1.0 + F64PRECISION
}
