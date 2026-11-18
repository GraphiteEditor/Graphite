use crate::metrics::{BORDER_DASHES, BORDER_WIDTH};
use glam::DVec2;
use std::f64::consts::{FRAC_PI_2, PI};
use std::ops::Range;
use vello_gpu::kurbo::{Arc, BezPath, CubicBez, Line, ParamCurve, PathEl, PathSeg, Point, QuadBez};

const MEASURE_TOLERANCE: f64 = 0.5;
const MEASURE_DEPTH: u32 = 20;
const ARC_TOLERANCE: f64 = 0.1;

struct Measured {
	distance: f64,
	segment: usize,
	t: f64,
}

pub(crate) fn dashed(path: &BezPath, [dash, gap]: [f64; 2]) -> BezPath {
	let mut dashes = BezPath::new();
	for contour in contours(path) {
		let measured = measure(&contour);
		let Some(length) = measured.last().map(|last| last.distance) else { continue };
		let mut distance = 0.;
		for (index, interval) in [dash, gap].into_iter().cycle().enumerate() {
			if distance >= length {
				break;
			}
			if index % 2 == 0 {
				piece(&contour, &measured, distance..(distance + interval).min(length), &mut dashes);
			}
			distance += interval;
		}
	}
	dashes
}

fn contours(path: &BezPath) -> Vec<Vec<PathSeg>> {
	let mut contours: Vec<Vec<PathSeg>> = Vec::new();
	let (mut start, mut last) = (Point::ZERO, Point::ZERO);
	for element in path.elements() {
		let segment = match *element {
			PathEl::MoveTo(point) => {
				contours.push(Vec::new());
				(start, last) = (point, point);
				continue;
			}
			PathEl::LineTo(point) => PathSeg::Line(Line::new(last, point)),
			PathEl::QuadTo(control, point) => PathSeg::Cubic(QuadBez::new(last, control, point).raise()),
			PathEl::CurveTo(first, second, point) => PathSeg::Cubic(CubicBez::new(last, first, second, point)),
			PathEl::ClosePath => PathSeg::Line(Line::new(last, start)),
		};
		last = segment.end();
		if let Some(contour) = contours.last_mut() {
			contour.push(segment);
		}
	}
	contours
}

fn measure(contour: &[PathSeg]) -> Vec<Measured> {
	let mut measured = Vec::new();
	let mut distance = 0.;
	for (segment, piece) in contour.iter().enumerate() {
		distance = match piece {
			PathSeg::Cubic(cubic) => measure_cubic(*cubic, distance, 0_f64..1., MEASURE_DEPTH, segment, &mut measured),
			_ => chord(piece.start(), piece.end(), distance, segment, 1., &mut measured),
		};
	}
	measured
}

fn measure_cubic(cubic: CubicBez, distance: f64, range: Range<f64>, depth: u32, segment: usize, measured: &mut Vec<Measured>) -> f64 {
	let strays = |control: Point, fraction: f64| {
		let on_chord = cubic.p0.lerp(cubic.p3, fraction);
		(control.x - on_chord.x).abs().max((control.y - on_chord.y).abs()) > MEASURE_TOLERANCE
	};
	if depth == 0 || !(strays(cubic.p1, 1. / 3.) || strays(cubic.p2, 2. / 3.)) {
		return chord(cubic.p0, cubic.p3, distance, segment, range.end, measured);
	}
	let (first, second) = cubic.subdivide();
	let middle = (range.start + range.end) / 2.;
	let distance = measure_cubic(first, distance, range.start..middle, depth - 1, segment, measured);
	measure_cubic(second, distance, middle..range.end, depth - 1, segment, measured)
}

fn chord(from: Point, to: Point, distance: f64, segment: usize, t: f64, measured: &mut Vec<Measured>) -> f64 {
	let next = distance + from.distance(to);
	if next > distance {
		measured.push(Measured { distance: next, segment, t });
	}
	next
}

fn at(measured: &[Measured], distance: f64) -> (usize, f64) {
	let index = measured.partition_point(|entry| entry.distance < distance).min(measured.len() - 1);
	let entry = &measured[index];
	let (start_distance, start_t) = match index.checked_sub(1).map(|previous| &measured[previous]) {
		Some(previous) if previous.segment == entry.segment => (previous.distance, previous.t),
		Some(previous) => (previous.distance, 0.),
		None => (0., 0.),
	};
	(entry.segment, start_t + (entry.t - start_t) * (distance - start_distance) / (entry.distance - start_distance))
}

fn piece(contour: &[PathSeg], measured: &[Measured], range: Range<f64>, dashes: &mut BezPath) {
	let (start_segment, start_t) = at(measured, range.start);
	let (end_segment, end_t) = at(measured, range.end);
	dashes.move_to(contour[start_segment].eval(start_t));
	let mut push = |segment: usize, range: Range<f64>| match contour[segment].subsegment(range) {
		PathSeg::Line(line) => dashes.line_to(line.p1),
		PathSeg::Quad(quad) => dashes.quad_to(quad.p1, quad.p2),
		PathSeg::Cubic(cubic) => dashes.curve_to(cubic.p1, cubic.p2, cubic.p3),
	};
	if start_segment == end_segment {
		push(start_segment, start_t..end_t);
		return;
	}
	push(start_segment, start_t..1.);
	for segment in start_segment + 1..end_segment {
		push(segment, 0. ..1.);
	}
	push(end_segment, 0. ..end_t);
}

enum BorderPart {
	Line(DVec2, DVec2),
	Corner(DVec2, f64),
}

pub(crate) fn border(min: DVec2, max: DVec2, radius: f64, gaps: &[[DVec2; 2]], dashed: bool) -> BezPath {
	let inset = BORDER_WIDTH / 2.;
	let arc_radius = radius - inset;
	let corner_length = flattened_arc_length(radius, [0., FRAC_PI_2]);
	let [left, top] = (min + inset).to_array();
	let [right, bottom] = (max - inset).to_array();
	let parts = [
		BorderPart::Line(DVec2::new(left + arc_radius, top), DVec2::new(right - arc_radius, top)),
		BorderPart::Corner(DVec2::new(right - arc_radius, top + arc_radius), -FRAC_PI_2),
		BorderPart::Line(DVec2::new(right, top + arc_radius), DVec2::new(right, bottom - arc_radius)),
		BorderPart::Corner(DVec2::new(right - arc_radius, bottom - arc_radius), 0.),
		BorderPart::Line(DVec2::new(right - arc_radius, bottom), DVec2::new(left + arc_radius, bottom)),
		BorderPart::Corner(DVec2::new(left + arc_radius, bottom - arc_radius), FRAC_PI_2),
		BorderPart::Line(DVec2::new(left, bottom - arc_radius), DVec2::new(left, top + arc_radius)),
		BorderPart::Corner(DVec2::new(left + arc_radius, top + arc_radius), PI),
	];
	let length = |part: &BorderPart| match part {
		BorderPart::Line(from, to) => from.distance(*to),
		BorderPart::Corner(..) => corner_length,
	};
	let point = |part: &BorderPart, fraction: f64| match *part {
		BorderPart::Line(from, to) => from.lerp(to, fraction),
		BorderPart::Corner(center, start_angle) => center + arc_radius * DVec2::from_angle(start_angle + fraction * FRAC_PI_2),
	};
	let starts: Vec<f64> = parts.iter().scan(0., |start, part| Some(std::mem::replace(start, *start + length(part)))).collect();
	let perimeter: f64 = parts.iter().map(length).sum();

	let mut left_out: Vec<[f64; 2]> = parts
		.iter()
		.zip(&starts)
		.flat_map(|(part, &start)| {
			let straight = matches!(part, BorderPart::Line(..));
			let inside = gaps.iter().filter_map(move |&gap| fractions_inside(|fraction| point(part, fraction), straight, gap));
			inside.map(move |[from, to]| [start + from * length(part), start + to * length(part)])
		})
		.collect();
	left_out.sort_by(|a, b| a[0].total_cmp(&b[0]));

	let shown = if dashed {
		let [dash, gap] = BORDER_DASHES;
		let length = perimeter.floor();
		let fewer = (length / (dash + gap)).floor();
		let gaps = [fewer, fewer + 1.].map(|count| (length - count * dash) / count);
		let (count, gap) = if gaps[1] > 0. && (gaps[1] - gap).abs() <= (gaps[0] - gap).abs() {
			(fewer + 1., gaps[1])
		} else {
			(fewer, gaps[0])
		};
		(0..count as usize).map(|index| [index as f64 * (dash + gap), index as f64 * (dash + gap) + dash]).collect()
	} else {
		vec![[0., perimeter]]
	};
	let visible = shown.into_iter().flat_map(|[mut from, to]| {
		let mut pieces = Vec::new();
		for &[gap_from, gap_to] in &left_out {
			if gap_from < to && gap_to > from {
				pieces.extend((gap_from > from).then_some([from, gap_from]));
				from = from.max(gap_to);
			}
		}
		pieces.extend((from < to).then_some([from, to]));
		pieces
	});

	let mut border = BezPath::new();
	for [from, to] in visible {
		let mut started = false;
		for (part, start) in parts.iter().zip(&starts) {
			if length(part) == 0. {
				continue;
			}
			let [a, b] = [from, to].map(|at| ((at - start) / length(part)).clamp(0., 1.));
			if b <= a {
				continue;
			}
			if !started {
				border.move_to(<(f64, f64)>::from(point(part, a)));
				started = true;
			}
			match *part {
				BorderPart::Line(..) => border.line_to(<(f64, f64)>::from(point(part, b))),
				BorderPart::Corner(center, start_angle) => {
					border.extend(Arc::new(<(f64, f64)>::from(center), (arc_radius, arc_radius), start_angle + a * FRAC_PI_2, (b - a) * FRAC_PI_2, 0.).append_iter(ARC_TOLERANCE))
				}
			}
		}
	}
	border
}

fn flattened_arc_length(radius: f64, [from, to]: [f64; 2]) -> f64 {
	let point = |angle: f64| radius * DVec2::from_angle(angle);
	let middle = (from + to) / 2.;
	let bulge = point(middle) - (point(from) + point(to)) / 2.;
	if bulge.abs().max_element() > MEASURE_TOLERANCE {
		flattened_arc_length(radius, [from, middle]) + flattened_arc_length(radius, [middle, to])
	} else {
		point(from).distance(point(to))
	}
}

fn fractions_inside(point: impl Fn(f64) -> DVec2, straight: bool, [low, high]: [DVec2; 2]) -> Option<[f64; 2]> {
	let mut range = [0_f64, 1_f64];
	for axis in 0..2 {
		let value = |fraction: f64| point(fraction)[axis];
		let [start, end] = [value(0.), value(1.)];
		if start.min(end) > high[axis] || start.max(end) < low[axis] {
			return None;
		}
		if start == end {
			continue;
		}
		let fraction_at = |bound: f64| {
			if (bound - start) * (end - start) <= 0. {
				return 0.;
			}
			if (bound - end) * (end - start) >= 0. {
				return 1.;
			}
			if straight {
				return (bound - start) / (end - start);
			}
			let [mut before, mut after] = [0_f64, 1_f64];
			for _ in 0..f64::MANTISSA_DIGITS {
				let middle = (before + after) / 2.;
				if (value(middle) - bound) * (end - start) < 0. {
					before = middle;
				} else {
					after = middle;
				}
			}
			after
		};
		let [a, b] = [fraction_at(low[axis]), fraction_at(high[axis])];
		range = [range[0].max(a.min(b)), range[1].min(a.max(b))];
	}
	(range[0] < range[1]).then_some(range)
}

#[cfg(test)]
mod tests {
	use super::*;
	use vello_gpu::kurbo::Shape;

	#[test]
	fn dashes_start_at_the_start_of_each_contour_and_are_cut_at_its_end() {
		let mut path = BezPath::new();
		path.move_to((0., 0.));
		path.line_to((12., 0.));
		path.move_to((0., 10.));
		path.line_to((4., 10.));
		let starts: Vec<Point> = dashed(&path, [3., 2.])
			.elements()
			.iter()
			.filter_map(|element| if let PathEl::MoveTo(point) = element { Some(*point) } else { None })
			.collect();
		assert_eq!(starts, [(0., 0.), (5., 0.), (10., 0.), (0., 10.)].map(Point::from));
		let ends: Vec<Point> = dashed(&path, [3., 2.])
			.elements()
			.iter()
			.filter_map(|element| if let PathEl::LineTo(point) = element { Some(*point) } else { None })
			.collect();
		assert_eq!(ends, [(3., 0.), (8., 0.), (12., 0.), (3., 10.)].map(Point::from));
	}

	#[test]
	fn a_curve_is_measured_along_chords_that_stray_from_it_by_at_most_half_a_pixel() {
		let mut path = BezPath::new();
		path.move_to((0., 0.));
		path.curve_to((0., 100.), (100., 100.), (100., 0.));
		let measured = measure(&contours(&path)[0]);
		let measured_length = measured.last().unwrap().distance;
		let length = vello_gpu::kurbo::ParamCurveArclen::arclen(&CubicBez::new((0., 0.), (0., 100.), (100., 100.), (100., 0.)), 1e-9);
		assert!(measured_length < length && length - measured_length < 1., "{measured_length} {length}");
		assert!(measured.windows(2).all(|pair| pair[0].t < pair[1].t), "the chords follow the curve");
	}

	#[test]
	fn the_dashes_of_a_border_follow_its_outer_edge_with_flattened_corners_and_the_gap_nearest_two_pixels() {
		let starts = |path: &BezPath| {
			path.elements()
				.iter()
				.filter_map(|element| if let PathEl::MoveTo(point) = element { Some(*point) } else { None })
				.collect::<Vec<_>>()
		};
		let layer = starts(&border(DVec2::ZERO, DVec2::new(912., 48.), 8., &[], true));
		let on_top = layer.iter().filter(|point| point.y == 0.5).map(|point| point.x);
		assert!(
			on_top.clone().all(|x| ((x - 8.) / 5. - ((x - 8.) / 5.).round()).abs() < 1e-9),
			"the dashes along the top of a layer are five pixels apart"
		);
		assert_eq!(on_top.count(), 180);
		assert_eq!(layer.len(), 381);

		assert_eq!(
			starts(&border(DVec2::ZERO, DVec2::new(120., 72.), 2., &[], true)).len(),
			76,
			"a node card, its corners measured as chords"
		);
	}

	#[test]
	fn a_border_is_left_out_inside_its_gaps_corners_included() {
		let [min, max] = [DVec2::ZERO, DVec2::new(100., 48.)];
		let starts = |path: &BezPath| path.elements().iter().filter(|element| matches!(element, PathEl::MoveTo(_))).count();
		assert_eq!(starts(&border(min, max, 8., &[], false)), 1);

		let left = [DVec2::new(-8., -2.), DVec2::new(30., 50.)];
		let right = [DVec2::new(88., 12.), DVec2::new(112., 36.)];
		let cut = border(min, max, 8., &[left, right], false);
		assert_eq!(starts(&cut), 2, "the right gap splits what the left gap leaves");
		let bounds = cut.bounding_box();
		assert!((bounds.x0 - 30.).abs() < 1e-9 && bounds.x1 == 99.5, "{bounds:?}");
		let on_the_right = cut.elements().iter().filter_map(|element| element.end_point()).filter(|point| point.x == 99.5);
		assert!(
			on_the_right.clone().all(|point| !(12.001..=35.999).contains(&point.y)),
			"the right gap leaves out the middle of the right side"
		);
		assert!(on_the_right.clone().any(|point| (point.y - 12.).abs() < 1e-9) && on_the_right.clone().any(|point| (point.y - 36.).abs() < 1e-9));
	}
}
