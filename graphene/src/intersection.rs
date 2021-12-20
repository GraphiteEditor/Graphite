use std::ops::Mul;

use glam::{DAffine2, DVec2, DMat4, DMat3, DMat2};
use kurbo::{BezPath, Line, PathSeg, Point, Shape, QuadBez, ParamCurve, ParamCurveExtrema};

pub const F64PRECISION: f64 = 0.00000001;

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

pub struct Intersect{
	pub point: Point,
	pub t: f64,
	pub a_seg_idx: usize,
	pub b_seg_idx: usize,
	pub mark: i8,
}

impl Intersect{
	pub fn add_idx(self, a_idx: usize, b_idx: usize) -> Self {
		self.a_seg_idx = a_idx;
		self.b_seg_idx = b_idx;
		self
	}
}

impl From<(Point, f64)> for Intersect{
	fn from(place_time: (Point, f64)) -> Self{
		Intersect{point: place_time.0, t: place_time.1, a_seg_idx: 0, b_seg_idx: 0, mark: -1}
	}
}

/// return real roots to cubic equation: f(t) = a0 + t*a1 + t^2*a2 + t^3*a3
/// this function uses the Cardano-Viete algorithm, which I found here: https://quarticequations.com/Cubic.pdf
pub fn cubic_real_roots(a0: f64, a1: f64, a2: f64, a3: f64) -> Vec<Option<f64>> {
	use std::f64::consts::FRAC_PI_3 as PI_3;
	a0 = a0 / a3; a1 = a1 / a3; a2 = a2 / a3;
	let q: f64 = a1/3.0 - a2*a2/9.0;
	let r: f64 = (a1*a3 - 3.0*a0)/6.0 - a2*a2/27.0;
	let r2_q3 = r*r - q*q*q;
	if r2_q3 > 0.0 {
		return vec![Some((r + r2_q3.sqrt()).cbrt() + (r - r2_q3.sqrt()).cbrt() - a2/3.0), None, None];
	}
	else {
		let phi = match q > -F64PRECISION && q < F64PRECISION{
			true => 0.0,
			false => (r/(-q).powf(3.0/2.0)).acos(),
		};
		return vec![Some(2.0*(-q).sqrt()*(phi/2.0).cos() - a2/3.0),
			Some(2.0*(-q).sqrt()*(phi/2.0 + 2.0 * PI_3).cos() - a2/3.0),
			Some(2.0*(-q).sqrt()*(phi/2.0 - 2.0 * PI_3).cos() - a2/3.0)];
	}
}

/// return real roots to quadratic equation: f(t) = a0 + t*a1 + t^2*a2
pub fn quadratic_real_roots(a0: f64, a1: f64, a2: f64) -> Vec<Option<f64>> {
	let radicand = a1 * a1 - 4.0 * a2 * a0;
	if radicand < 0.0 { return vec![None, None]; }
	vec![Some((-a1 + radicand.sqrt())/(2.0 * a2)), Some((-a1 - radicand.sqrt())/(2.0*a2))]
}

// return root to linear equation: f(t) = a0 + t*a1
pub fn linear_root(a0: f64, a1: f64) -> Vec<Option<f64>> {
	if a1 == 0.0 {return vec![None];}
	vec![Some(-a0 / a1)]
}

fn line_raise(l: &Line) -> QuadBez{
	QuadBez{p0: l.p0, p1: l.p0, p2: l.p1}
}

fn promote_pathseg(a: &PathSeg, b: &PathSeg) -> (PathSeg, PathSeg) {
	let greater = PathSeg::from(*a);
	let mut smaller = PathSeg::from(*b);
	// these functions are defined here because they are quite specific
	fn precedence_fn(seg: &PathSeg) -> usize {match seg {PathSeg::Cubic(_) => 2, PathSeg::Quad(_) => 1, PathSeg::Line(_) => 0}}
	fn promote_fn(seg: &PathSeg) -> PathSeg {match seg{PathSeg::Quad(quadbez) => PathSeg::Cubic(quadbez.raise()), PathSeg::Line(ref line) => PathSeg::Quad(line_raise(line)), _ => *seg,}}

	if precedence_fn(b) > precedence_fn(a) {greater = PathSeg::from(*b); smaller = PathSeg::from(*a);}

	while precedence_fn(&greater) > precedence_fn(&smaller) {smaller = promote_fn(& smaller);}
	(greater, smaller)
}

pub const C_CUBIC: DMat4 = DMat4::from_cols_array(&[1.0, 0.0, 0.0, 0.0,  -3.0, 3.0, 0.0, 0.0,  3.0, -6.0, 3.0, 0.0,  -1.0, 3.0, -3.0, 1.0]);
pub const C_QUAD: DMat3 = DMat3::from_cols_array(&[1.0, 0.0, 0.0,  -2.0, 2.0, 0.0,  1.0, -2.0, 1.0]);
pub const C_LINE: DMat2 = DMat2::from_cols_array(&[1.0, 0.0,  -1.0, 1.0]);
macro_rules! mat_from_points {
	($p1:expr, $p2:expr, $p3:expr, $p4:expr) => {
		DMat4::from_cols_array(&[$p1.x, $p1.y, 0.0, 0.0,  $p2.x, $p2.y, 0.0, 0.0,  $p3.x, $p3.y, 0.0, 0.0,  $p4.x, $p4.y, 0.0, 0.0])
	};
	($p1:expr, $p2:expr, $p3:expr) => {
		DMat3::from_cols_array(&[$p1.x, $p1.y, 0.0,  $p2.x, $p2.y, 0.0,  $p3.x, $p3.y, 0.0])
	};
	($p1:expr, $p2:expr) => {
		DMat2::from_cols_array(&[$p1.x, $p1.y,  $p2.x, $p2.y])
	}
}
pub fn get_path_intersection(a: & PathSeg, b: & PathSeg) -> Vec<Intersect> {
	let mut intersections = Vec::new();
	//filter for properly sized t values
	//rounding errors in t values
	match promote_pathseg(a, b) {
		(PathSeg::Cubic(c1), PathSeg::Cubic(c2)) => {
			let coeff = (mat_from_points!(c1.p0 - c2.p0, c1.p1 - c2.p1, c1.p2 - c2.p2, c1.p3 - c2.p3) * C_CUBIC).to_cols_array();
			let mut t_vals: Vec<f64> = cubic_real_roots(coeff[0], coeff[4], coeff[8], coeff[12]).iter()
				.filter_map(|root| {
					if let Some(val) = root {
						if *val > -F64PRECISION && *val < 1.0+F64PRECISION { return Some(*val); }
					}
					None
				}).collect();
			intersections.append(&mut cubic_real_roots(coeff[1], coeff[5], coeff[9], coeff[13]).iter()
				.filter_map(|root| {
					if let Some(val) = root {
						if t_vals.contains(val) { return Some(Intersect::from((c1.eval(*val), *val))); }
					}
					None
				}).collect());
		}
		(PathSeg::Quad(q1), PathSeg::Quad(q2)) => {
			let coeff = (mat_from_points!(q1.p0 - q2.p0, q1.p1 - q2.p1, q1.p2 - q2.p2) * C_QUAD).to_cols_array();
			let t_val: Vec<f64> = quadratic_real_roots(coeff[0], coeff[3], coeff[6]).iter()
				.filter_map(|root| {
					if let Some(val) = root {
						if *val > -F64PRECISION && *val < 1.0+F64PRECISION { return Some(*val); }
					}
					None
				}).collect();
			intersections.append(&mut quadratic_real_roots(coeff[1], coeff[4], coeff[7]).iter()
				.filter_map(|root| {
					if let Some(val) = root {
						if t_val.contains(val) { return Some(Intersect::from((q1.eval(*val), *val))); }
					}
					None
				}).collect());
		}
		(PathSeg::Line(l1), PathSeg::Line(l2)) => {
			let coeff = (mat_from_points!(l1.p0 - l2.p0, l1.p1 - l2.p1) * C_LINE).to_cols_array();
 		}
	}
	intersections
}

pub fn get_intersections(a: &BezPath, b: &BezPath) -> Vec<Intersect>{
	let intersections = Vec::new();
	let to_check = get_intersection_candidates(a, b);
	let a_segs: Vec<PathSeg> = a.segments().collect();
	let b_segs: Vec<PathSeg> = b.segments().collect();
	for (a_idx, b_idx) in to_check{
		// a_idx and b_idx should both be valid indices
		for path_intersection in get_path_intersection(a_segs.get(a_idx).unwrap(), b_segs.get(b_idx).unwrap()){
			intersections.push(path_intersection.add_idx(a_idx, b_idx));
		}
	}
	intersections
}

pub fn get_intersection_candidates(a: &BezPath, b: &BezPath) -> Vec<(usize, usize)> {
	// optimization ideas
	//		- store computed bounding boxes
	let mut intersections = Vec::new();
	a.segments().enumerate().for_each(|(a_idx, a_seg)| b.segments().enumerate().for_each(|(b_idx, b_seg)| {
		if <PathSeg as ParamCurveExtrema>::bounding_box(&a_seg).intersect(<PathSeg as ParamCurveExtrema>::bounding_box(&b_seg)).area() > 0.0{
			intersections.push((a_idx, b_idx));
		}
	}));
	intersections
}

pub fn get_arbitrary_point_on_path(path: &BezPath) -> Option<Point> {
	path.segments().next().map(|seg| match seg {
		PathSeg::Line(line) => line.p0,
		PathSeg::Quad(quad) => quad.p0,
		PathSeg::Cubic(cubic) => cubic.p0,
	})
}
