use std::ops::Mul;

use glam::{DAffine2, DVec2, DMat2};
use kurbo::{BezPath, Line, PathSeg, Point, Shape, Rect, QuadBez, ParamCurve, ParamCurveExtrema};
use crate::boolean_ops::split_path_seg;

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
	pub t_a: f64,
	pub t_b: f64,
	pub a_seg_idx: usize,
	pub b_seg_idx: usize,
	pub mark: i8,
	pub quality: f64,
}

impl Intersect{
	pub fn add_idx(&mut self, a_idx: usize, b_idx: usize) {
		self.a_seg_idx = a_idx;
		self.b_seg_idx = b_idx;
	}
}

impl From<(Point, f64, f64)> for Intersect{
	fn from(place_time: (Point, f64, f64)) -> Self{
		Intersect{point: place_time.0, t_a: place_time.1, t_b: place_time.2, a_seg_idx: 0, b_seg_idx: 0, mark: -1, quality: 0.0}
	}
}

// because extrema are owned by each SubCurve ( and not refrenced ), they must be copied on each split
struct SubCurve<'a> {
	pub curve: &'a PathSeg,
	pub start: f64,
	pub end: f64,
	pub extrema: Vec<(Point, f64)>,
}

impl<'a> SubCurve<'a> {
	fn bounding_box(&self) -> Rect {
		let mut ll = self.curve.eval(self.start);
		let mut ur = self.curve.eval(self.end);
		vec![(self.curve.eval(self.start), self.start), (self.curve.eval(self.end), self.end)].iter().chain(self.extrema.iter())
			.for_each(|(p, _)|{
				if p.x < ll.x {ll.x = p.x;}
				if p.x > ur.x {ur.x = p.x;}
				if p.y < ll.y {ll.y = p.y;}
				if p.y > ur.y {ur.y = p.y;}
			});
		Rect {x0: ll.x, y0: ll.y, x1: ur.x, y1: ur.y}
	}

	fn eval(&self, t: f64) -> Point {
		self.curve.eval(t)
	}

	fn split(&self, t: f64) -> (SubCurve, SubCurve) {
		(SubCurve {curve: self.curve, start: self.start, end: t, extrema: self.extrema.clone()},
		 SubCurve {curve: self.curve, start: t, end: self.end, extrema: self.extrema.clone()})
	}
}

impl<'a> From< &'a PathSeg > for SubCurve<'a> {
	fn from(parent: &'a PathSeg) -> Self {
		// extrema contains local min/max, not the endpoints
		SubCurve {
			curve: parent,
			start: 0.0,
			end: 1.0,
			extrema: parent.extrema().iter().filter_map(|t| { if *t > 0.0 || *t < 1.0 { Some((parent.eval(*t), *t)) } else {None} } ).collect(),
		}
	}
}

fn line_raise(l: &Line) -> QuadBez{
	QuadBez{p0: l.p0, p1: l.p0, p2: l.p1}
}

fn promote_pathseg(a: &PathSeg, b: &PathSeg) -> (PathSeg, PathSeg) {
	let mut greater = PathSeg::from(*a);
	let mut smaller = PathSeg::from(*b);
	// these functions are defined here because they are quite specific
	fn precedence_fn(seg: &PathSeg) -> usize {match seg {PathSeg::Cubic(_) => 2, PathSeg::Quad(_) => 1, PathSeg::Line(_) => 0}}
	fn promote_fn(seg: &PathSeg) -> PathSeg {match seg{PathSeg::Quad(quadbez) => PathSeg::Cubic(quadbez.raise()), PathSeg::Line(ref line) => PathSeg::Quad(line_raise(line)), _ => *seg,}}

	if precedence_fn(b) > precedence_fn(a) {greater = PathSeg::from(*b); smaller = PathSeg::from(*a);}

	while precedence_fn(&greater) > precedence_fn(&smaller) {smaller = promote_fn(& smaller);}
	(greater, smaller)
}

#[allow(unused_macros)]
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

/// Rough algorithm
/// 	- Behavior: when shapes have indentical pathsegs algorithm returns endpoints as intersects?
/// 	- Bug: algorithm finds same intersection multiple times in same recursion path
/// 	- Improvement: more adapative way to decide when "close enough"
///   - improvement: quality metric
/// 	- Optimization: Don't actualy split the curve, just pass start/end values
/// 	- Optimization: Compute curve's derivitive once
/// 	- Optimization: bounding_box's dont need to be recomputed?
/// 	- Optimization: Lots of extra copying happening
/// 	- Optimization: how efficiently does std::Vec::append work?
/// 	- Optimization: specialized line/quad/cubic combination algorithms
fn path_intersections(a: &PathSeg, b: &PathSeg, mut recursion: usize, t_a: f64, t_b: f64) -> Vec<Intersect> {
	let mut intersections = Vec::new();
	//special case
	if let (PathSeg::Line(line_a), PathSeg::Line(line_b)) = (a, b){
		if let Some(cross) = line_intersection(&line_a, &line_b) {intersections.push(cross);}
	}
	else if overlap( &<PathSeg as ParamCurveExtrema>::bounding_box(&a), &<PathSeg as ParamCurveExtrema>::bounding_box(&b)) {
		recursion += 1;
		// bail out!! before lshift with overflow
		if recursion == 32 { return intersections; }
		// base case, we are close enough to try linear approximation
		if recursion > 15 { //arbitrarily chosen limit
			if let Some(mut cross) = line_intersection(&Line{p0: a.start(), p1: a.end()}, &Line{p0: b.start(), p1: b.end()}){
				// intersection t_value equals the recursive t_value + interpolated intersection value
				cross.t_a = t_a + cross.t_a / (1 << recursion) as f64;
				cross.t_b = t_b + cross.t_b / (1 << recursion) as f64;
				cross.quality = guess_quality(a, b, &cross);
				intersections.push(cross);
				return intersections;
			}
		}
		let (a1, a2) = split_path_seg(a, 0.5);
		let (b1, b2) = split_path_seg(b, 0.5);
		intersections.append(&mut path_intersections(&a1, &b1, recursion, t_a, t_b));
		intersections.append(&mut path_intersections(&a1, &b2, recursion, t_a, t_b + 1.0 / (1 << recursion) as f64));
		intersections.append(&mut path_intersections(&a2, &b1, recursion, t_a + 1.0 / (1 << recursion) as f64, t_b));
		intersections.append(&mut path_intersections(&a2, &b2, recursion, t_a + 1.0 / (1 << recursion) as f64, t_b + 1.0 / (1 << recursion) as f64));
	}
	intersections
}

fn guess_quality(a: &PathSeg, b: &PathSeg, guess: &Intersect) -> f64{
	let dist_a = guess.point - b.eval(guess.t_b);
	let dist_b = guess.point - a.eval(guess.t_a);
	// prevent division by 0
	return (2.0 / (1.0 + dist_a.x * dist_b.x * dist_a.y * dist_b.y )).abs()
}

pub fn intersections(a: &BezPath, b: &BezPath) -> Vec<Intersect>{
	let mut intersections: Vec<Intersect> = Vec::new();
	a.segments().enumerate().for_each(|(a_idx, a_seg)| b.segments().enumerate().for_each(|(b_idx, b_seg)|{
		for mut path_intersection in path_intersections(&a_seg, &b_seg, 0, 0.0, 0.0){
			intersections.push({path_intersection.add_idx(a_idx, b_idx); path_intersection});
		}
	}));
	intersections
}


pub fn intersection_candidates(a: &BezPath, b: &BezPath) -> Vec<(usize, usize)> {
	// optimization ideas
	//		- store computed bounding boxes
	let mut intersections = Vec::new();

	a.segments().enumerate().for_each(|(a_idx, a_seg)| b.segments().enumerate().for_each(|(b_idx, b_seg)| {
		if overlap(&<PathSeg as ParamCurveExtrema>::bounding_box(&a_seg), &<PathSeg as ParamCurveExtrema>::bounding_box(&b_seg)) {
			intersections.push((a_idx, b_idx));
		}
	}));
	intersections
}

/// returns intersection point as if lines extended forever
pub fn line_intersect_point(a: &Line, b: &Line) -> Option<Point> {
	let slopes = DMat2::from_cols_array(&[(b.p1 - b.p0).x, (b.p1 - b.p0).y,  (a.p0 - a.p1).x, (a.p0 - a.p1).y]);
	if slopes.determinant() == 0.0 {return None}
	let t_vals = slopes.inverse() * DVec2::new((b.p0 - a.p0).x, (b.p1 - a.p1).y);
	Some(b.eval(t_vals[0]))
}

/// returns intersection point and t values, treating lines as Bezier curves
pub fn line_intersection(a: &Line, b: &Line) -> Option<Intersect> {
	let slopes = DMat2::from_cols_array(&[(b.p1 - b.p0).x, (b.p1 - b.p0).y,  (a.p0 - a.p1).x, (a.p0 - a.p1).y]);
	if slopes.determinant() == 0.0 {return None;}
	let t_vals = slopes.inverse() * DVec2::new((a.p0 - b.p0).x, (a.p0 - b.p0).y);
	if !valid_t(t_vals[0]) || !valid_t(t_vals[1]) {return None;}
	Some(Intersect::from((b.eval(t_vals[0]), t_vals[1], t_vals[0])))
}

/// returns true rectangles overlap
/// does using slices here cause a slowdown?
pub fn overlap(a: &Rect, b: &Rect) -> bool {
	fn in_range(n: f64, range: &[f64]) -> bool { n >= range[0] && n <= range[1] }
	(in_range(b.x0, &[a.x0, a.x1]) || in_range(b.x1, &[a.x0, a.x1]) || in_range(a.x0, &[b.x0, b.x1]) || in_range(a.x1, &[b.x0, b.x1])) &&
	(in_range(b.y0, &[a.y0, a.y1]) || in_range(b.y1, &[a.y0, a.y1]) || in_range(a.y0, &[b.y0, b.y1]) || in_range(a.y1, &[b.y0, b.y1]))
}

fn valid_t(t: f64) -> bool {
	t > -F64PRECISION && t < 1.0 + F64PRECISION
}

pub fn get_arbitrary_point_on_path(path: &BezPath) -> Option<Point> {
	path.segments().next().map(|seg| match seg {
		PathSeg::Line(line) => line.p0,
		PathSeg::Quad(quad) => quad.p0,
		PathSeg::Cubic(cubic) => cubic.p0,
	})
}
