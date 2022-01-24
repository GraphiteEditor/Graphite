use std::ops::Mul;

use crate::consts::F64PRECISION;
use glam::{DAffine2, DMat2, DVec2};
use kurbo::{BezPath, Line, ParamCurve, ParamCurveExtrema, PathSeg, Point, Rect, Shape};

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

#[derive(Debug, PartialEq)]
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
	local: [Point; 2], // local endpoints
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

	fn available_precision(&self) -> f64 {
		(self.start_t - self.end_t).abs()
	}

	// In a bounding box of A area, the points are at most A units apart
	fn size_precision_ratio(&self) -> f64 {
		self.bounding_box().area()
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

/**
Bezier Curve Intersection Algorithm
- TODO: How does f64 precision effect the algorithm?
- TODO: profile algorithm
- Bug: intersections of "perfectly alligned" line or curve
	- If the algorithm is rewritten to be non-recursive it can be restructured to be more breadth first then depth first.
	  This would allow easy recognition of the case where many (or all) bounding boxes intersect, this case is correlated with alligned curves
	- Alternatively, this bug can be solved only for the linear case, which probably covers the great majority of cases
- Bug: deep recursion can result in stack overflow
- Improvement: intersections on the end of segments
	- Intersections near the end of segments seem to be consistently lower 'quality', why?
- Improvement: algorithm behavior when curves have very different sizes
- Improvement: more adapative way to decide when "close enough"
- Improvement: quality metric?
- Optimization: any extra copying happening?
- Optimization: how efficiently does std::Vec::append work?
- Optimization: specialized line/quad/cubic combination algorithms
*/
fn path_intersections(a: &SubCurve, b: &SubCurve, mut recursion: f64, intersections: &mut Vec<Intersect>) {
	// special case
	if let (PathSeg::Line(line_a), PathSeg::Line(line_b)) = (a.curve, b.curve) {
		if let Some(cross) = line_intersection(line_a, line_b) {
			intersections.push(cross);
		}
	} else if overlap(&a.bounding_box(), &b.bounding_box()) {
		// we are close enough to try linear approximation
		if recursion < (1 << 10) as f64 {
			if let Some(mut cross) = line_intersection(&Line { p0: a.start(), p1: a.end() }, &Line { p0: b.start(), p1: b.end() }) {
				// intersection t_value equals the recursive t_value + interpolated intersection value
				cross.t_a = a.start_t + cross.t_a * recursion;
				cross.t_b = b.start_t + cross.t_b * recursion;
				cross.quality = guess_quality(a.curve, b.curve, &cross);

				// log::debug!("checking: {:?}", cross.quality);
				if cross.quality <= F64PRECISION {
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
			// Note: may occur for the less precise side of an PathSeg endpoint intersect
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

/// Optimization: inline? maybe...
/// For quality Q in the worst case, the point on curve "a" corresponding to "guess" is distance Q from the point on curve "b"
fn guess_quality(a: &PathSeg, b: &PathSeg, guess: &Intersect) -> f64 {
	let at_a = b.eval(guess.t_b);
	let at_b = a.eval(guess.t_a);
	at_a.distance(guess.point) + at_b.distance(guess.point)
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
			.filter_map(|t| if *t > F64PRECISION && *t < 1.0 - F64PRECISION { Some(a_seg.eval(*t)) } else { None })
			.collect();
		b.segments().enumerate().for_each(|(b_idx, b_seg)| {
			let b_extrema = b_seg
				.extrema()
				.iter()
				.filter_map(|t| if *t > F64PRECISION && *t < 1.0 - F64PRECISION { Some(b_seg.eval(*t)) } else { None })
				.collect();
			let mut intersects = Vec::new();
			path_intersections(&SubCurve::new(&a_seg, &a_extrema), &SubCurve::new(&b_seg, &b_extrema), 1.0, &mut intersects);
			for mut path_intersection in intersects {
				intersections.push({
					path_intersection.add_idx(a_idx, b_idx);
					path_intersection
				});
			}
		})
	});

	// print out result for testing
	// log::info!("{:?}", intersections);

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
/// uses kurbo::Rect{x0, y0, x1, y1} where x0 <= x1 and y0 <= y1
pub fn overlap(a: &Rect, b: &Rect) -> bool {
	a.x0 <= b.x1 && a.y0 <= b.y1 && b.x0 <= a.x1 && b.y0 <= a.y1
}

/// tests if a t value belongs to [0.0, 1.0]
/// uses F64PRECISION to allow a slightly larger range of values
fn valid_t(t: f64) -> bool {
	t > -F64PRECISION && t < 1.0
}

/// each of these tests has been visualy, but not mathematically verified
/// each test looks for exact floating point comparisions, so isn't flexible to small adjustments in the algorithm
mod tests {
	#[allow(unused_imports)] // this import is used
	use super::*;

	/// two intersect points, on different PathSegs
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
}
