//! Constructors for the primitive shapes used by the vector generator nodes.
//!
//! Anchor order and winding direction are load-bearing, since fills rely on every generator agreeing.

use crate::vector::misc::{ArcType, SpiralType, bezpath_from_anchors_and_handles};
use glam::DVec2;
use kurbo::BezPath;
use std::f64::consts::TAU;

/// Constant from <https://pomax.github.io/bezierinfo/#circles_cubic>
const HANDLE_OFFSET_FACTOR: f64 = 0.551784777779014;

/// An anchor point with its optional incoming and outgoing handle positions, in absolute coordinates.
#[derive(Clone)]
struct Anchor {
	position: DVec2,
	in_handle: Option<DVec2>,
	out_handle: Option<DVec2>,
}

impl Anchor {
	fn new(position: DVec2, in_handle: Option<DVec2>, out_handle: Option<DVec2>) -> Self {
		Self { position, in_handle, out_handle }
	}

	fn sharp(position: DVec2) -> Self {
		Self::new(position, None, None)
	}
}

fn bezpath_from_anchors(anchors: &[Anchor], closed: bool) -> BezPath {
	bezpath_from_anchors_and_handles(anchors.iter().map(|anchor| (anchor.position, anchor.in_handle, anchor.out_handle)), closed)
}

/// Stitches a sequence of sharp (handleless) anchors into a polyline, or a closed polygon.
pub fn polyline_bezpath(positions: impl IntoIterator<Item = DVec2>, closed: bool) -> BezPath {
	let anchors: Vec<Anchor> = positions.into_iter().map(Anchor::sharp).collect();
	bezpath_from_anchors(&anchors, closed)
}

/// Constructs a rectangle with `corner1` and `corner2` as the two corners.
pub fn rectangle_bezpath(corner1: DVec2, corner2: DVec2) -> BezPath {
	polyline_bezpath([corner1, DVec2::new(corner2.x, corner1.y), corner2, DVec2::new(corner1.x, corner2.y)], true)
}

/// Constructs a rounded rectangle with `corner1` and `corner2` as the two corners and `corner_radii` as the radii of the corners: `[top_left, top_right, bottom_right, bottom_left]`.
pub fn rounded_rectangle_bezpath(corner1: DVec2, corner2: DVec2, corner_radii: [f64; 4]) -> BezPath {
	if corner_radii.iter().all(|radius| radius.abs() < f64::EPSILON * 100.) {
		return rectangle_bezpath(corner1, corner2);
	}

	use std::f64::consts::{FRAC_1_SQRT_2, PI};

	// The pair of anchors where one rounded corner's arc leaves and rejoins the straight edges
	let corner_anchors = |center: DVec2, corner: DVec2, radius: f64| -> Vec<Anchor> {
		let point1 = center + DVec2::from_angle(-PI * 0.25).rotate(corner - center) * FRAC_1_SQRT_2;
		let point2 = center + DVec2::from_angle(PI * 0.25).rotate(corner - center) * FRAC_1_SQRT_2;
		if radius == 0. {
			return vec![Anchor::sharp(point1), Anchor::sharp(point2)];
		}

		let handle_offset = radius * HANDLE_OFFSET_FACTOR;
		vec![
			Anchor::new(point1, None, Some(point1 + handle_offset * (corner - point1).normalize())),
			Anchor::new(point2, Some(point2 + handle_offset * (corner - point2).normalize()), None),
		]
	};

	let anchors = [
		corner_anchors(DVec2::new(corner1.x + corner_radii[0], corner1.y + corner_radii[0]), DVec2::new(corner1.x, corner1.y), corner_radii[0]),
		corner_anchors(DVec2::new(corner2.x - corner_radii[1], corner1.y + corner_radii[1]), DVec2::new(corner2.x, corner1.y), corner_radii[1]),
		corner_anchors(DVec2::new(corner2.x - corner_radii[2], corner2.y - corner_radii[2]), DVec2::new(corner2.x, corner2.y), corner_radii[2]),
		corner_anchors(DVec2::new(corner1.x + corner_radii[3], corner2.y - corner_radii[3]), DVec2::new(corner1.x, corner2.y), corner_radii[3]),
	]
	.concat();

	bezpath_from_anchors(&anchors, true)
}

/// Constructs an ellipse with `corner1` and `corner2` as the two corners of the bounding box.
pub fn ellipse_bezpath(corner1: DVec2, corner2: DVec2) -> BezPath {
	let size = (corner1 - corner2).abs();
	let center = (corner1 + corner2) / 2.;
	let top = DVec2::new(center.x, corner1.y);
	let bottom = DVec2::new(center.x, corner2.y);
	let left = DVec2::new(corner1.x, center.y);
	let right = DVec2::new(corner2.x, center.y);

	let handle_offset = size * HANDLE_OFFSET_FACTOR * 0.5;

	let anchors = [
		Anchor::new(top, Some(top - handle_offset * DVec2::X), Some(top + handle_offset * DVec2::X)),
		Anchor::new(right, Some(right - handle_offset * DVec2::Y), Some(right + handle_offset * DVec2::Y)),
		Anchor::new(bottom, Some(bottom + handle_offset * DVec2::X), Some(bottom - handle_offset * DVec2::X)),
		Anchor::new(left, Some(left + handle_offset * DVec2::Y), Some(left - handle_offset * DVec2::Y)),
	];

	bezpath_from_anchors(&anchors, true)
}

/// Constructs an arc by a `radius`, `start_angle` and `sweep_angle`. Angles must be in radians. The arc type makes it look like a pie or pacman.
pub fn arc_bezpath(radius: f64, start_angle: f64, sweep_angle: f64, arc_type: ArcType) -> BezPath {
	// Prevents glitches from numerical imprecision that have been observed during animation playback after about a minute
	let start_angle = start_angle % (TAU * 2.);
	let sweep_angle = sweep_angle % (TAU * 2.);

	let original_start_angle = start_angle;
	let sweep_angle_sign = sweep_angle.signum();

	let mut start_angle = 0.;
	let mut sweep_angle = sweep_angle.abs();

	if ((sweep_angle / TAU).floor() as u32).is_multiple_of(2) {
		sweep_angle %= TAU;
	} else {
		start_angle = sweep_angle % TAU;
		sweep_angle = TAU - start_angle;
	}

	sweep_angle *= sweep_angle_sign;
	start_angle *= sweep_angle_sign;
	start_angle += original_start_angle;

	let closed = arc_type == ArcType::Closed;
	let slice = arc_type == ArcType::PieSlice;

	let center = DVec2::new(0., 0.);
	let segments = (sweep_angle.abs() / (std::f64::consts::PI / 4.)).ceil().max(1.) as usize;
	let step = sweep_angle / segments as f64;
	let factor = 4. / 3. * (step / 2.).sin() / (1. + (step / 2.).cos());

	let mut anchors = Vec::with_capacity(segments);
	let mut prev_in_handle = None;
	let mut prev_end = DVec2::new(0., 0.);

	for i in 0..segments {
		let start_angle = start_angle + step * i as f64;
		let end_angle = start_angle + step;
		let start_vec = DVec2::from_angle(start_angle);
		let end_vec = DVec2::from_angle(end_angle);

		let start = center + radius * start_vec;
		let end = center + radius * end_vec;

		let handle_start = start + start_vec.perp() * radius * factor;
		let handle_end = end - end_vec.perp() * radius * factor;

		anchors.push(Anchor::new(start, prev_in_handle, Some(handle_start)));
		prev_in_handle = Some(handle_end);
		prev_end = end;
	}
	anchors.push(Anchor::new(prev_end, prev_in_handle, None));

	if slice {
		anchors.push(Anchor::sharp(center));
	}

	bezpath_from_anchors(&anchors, closed || slice)
}

/// Constructs a regular polygon (ngon). Based on `sides` and `radius`, which is the distance from the center to any vertex.
pub fn regular_polygon_bezpath(center: DVec2, sides: u64, radius: f64) -> BezPath {
	let sides = sides.max(3);
	let angle_increment = TAU / (sides as f64);
	let positions = (0..sides).map(|i| {
		let angle = (i as f64) * angle_increment - std::f64::consts::FRAC_PI_2;
		center + radius * DVec2::new(f64::cos(angle), f64::sin(angle))
	});

	polyline_bezpath(positions, true)
}

/// Constructs a star polygon (n-star). See [`regular_polygon_bezpath`], but with interspersed vertices at an `inner_radius`.
pub fn star_polygon_bezpath(center: DVec2, sides: u64, radius: f64, inner_radius: f64) -> BezPath {
	let sides = sides.max(2);
	let angle_increment = 0.5 * TAU / (sides as f64);
	let positions = (0..sides * 2).map(|i| {
		let angle = (i as f64) * angle_increment - std::f64::consts::FRAC_PI_2;
		let radius = if i % 2 == 0 { radius } else { inner_radius };
		center + radius * DVec2::new(f64::cos(angle), f64::sin(angle))
	});

	polyline_bezpath(positions, true)
}

/// Constructs a line from `point1` to `point2`.
pub fn line_bezpath(point1: DVec2, point2: DVec2) -> BezPath {
	polyline_bezpath([point1, point2], false)
}

/// Constructs an arrow shape from start and end points with parametric control over dimensions.
pub fn arrow_bezpath(start: DVec2, end: DVec2, shaft_width: f64, head_width: f64, head_length: f64) -> BezPath {
	let delta = end - start;
	let length = delta.length();

	// Degenerate case: return a point
	if length < 1e-10 {
		return polyline_bezpath([start], true);
	}

	let direction = delta / length;
	let perpendicular = DVec2::new(-direction.y, direction.x);

	let half_shaft = shaft_width * 0.5;
	let half_head = head_width * 0.5;
	let head_base_distance = (length - head_length).max(0.);
	let head_base = start + direction * head_base_distance;

	// Arrow path starts at the tail, traces around the shape, and returns to the tail
	let positions = [
		start,                                  // Tail center (origin)
		start + perpendicular * half_shaft,     // Tail top
		head_base + perpendicular * half_shaft, // Head base top (shaft)
		head_base + perpendicular * half_head,  // Head base top (wide)
		end,                                    // Tip
		head_base - perpendicular * half_head,  // Head base bottom (wide)
		head_base - perpendicular * half_shaft, // Head base bottom (shaft)
		start - perpendicular * half_shaft,     // Tail bottom
	];

	polyline_bezpath(positions, true)
}

/// Constructs a spiral winding from an inner radius `a` out to `outer_radius`, sampled every `delta_theta` radians.
pub fn spiral_bezpath(a: f64, outer_radius: f64, turns: f64, start_angle: f64, delta_theta: f64, spiral_type: SpiralType) -> BezPath {
	let mut anchors = Vec::new();
	let mut prev_in_handle = None;
	let theta_end = turns * TAU + start_angle;

	let a = if spiral_type == SpiralType::Logarithmic { a.max(1e-10) } else { a };
	let b = calculate_growth_factor(a, turns, outer_radius, spiral_type);

	let mut theta = start_angle;
	while theta < theta_end {
		let theta_next = f64::min(theta + delta_theta, theta_end);

		let p0 = spiral_point(theta, a, b, spiral_type);
		let p3 = spiral_point(theta_next, a, b, spiral_type);
		let t0 = spiral_tangent(theta, a, b, spiral_type);
		let t1 = spiral_tangent(theta_next, a, b, spiral_type);

		let arc_length = spiral_arc_length(theta, theta_next, a, b, spiral_type);
		let handle_distance = arc_length / 3.;

		let p1 = p0 + handle_distance * t0;
		let p2 = p3 - handle_distance * t1;

		anchors.push(Anchor::new(p0, prev_in_handle, Some(p1)));
		prev_in_handle = Some(p2);

		// If final segment, end with anchor at theta_end
		if (theta_next - theta_end).abs() < f64::EPSILON {
			anchors.push(Anchor::new(p3, prev_in_handle, None));
			break;
		}

		theta = theta_next;
	}

	bezpath_from_anchors(&anchors, false)
}

/// Constructs a teardrop with `corner1` and `corner2` as the two corners of the bounding box.
pub fn teardrop_bezpath(corner1: DVec2, corner2: DVec2, velocity: f64) -> BezPath {
	let size = (corner1 - corner2).abs();

	// the bottom half of the teardrop is a circle, upon which these calculations are heavily based
	let circle_center = DVec2::new((corner1.x + corner2.x) / 2., (corner1.y + (2. * velocity - 1.) * corner2.y) / (2. * velocity));

	let top = DVec2::new(circle_center.x, corner1.y);
	let bottom = DVec2::new(circle_center.x, corner2.y);
	let left = DVec2::new(corner1.x, circle_center.y);
	let right = DVec2::new(corner2.x, circle_center.y);

	// because we modify the dimensions vertically, the handle_offset remains the same as
	// for a circle *horizontally*, but the vertical handle_offset must be adjusted
	let horizontal_handle_offset = size * HANDLE_OFFSET_FACTOR * 0.5;
	let vertical_handle_offset = horizontal_handle_offset / velocity;

	// I've found that the teardrop looks better when its sides go up a little steeper than they go down
	let roundness_multiplier = 1.6;
	let roundness = vertical_handle_offset * roundness_multiplier;

	// let roundness_multiplier = 0.6;
	// let roundness = vertical_handle_offset * roundness_multiplier * pointiness;

	// both handles for the top point of the teardrop, to make it pointier
	let point_handle_multiplier = 0.28;
	let point_handles = Some(top + size * point_handle_multiplier * DVec2::Y);

	let anchors = [
		Anchor::new(top, point_handles, point_handles),
		Anchor::new(right, Some(right - roundness * DVec2::Y), Some(right + vertical_handle_offset * DVec2::Y)),
		Anchor::new(bottom, Some(bottom + horizontal_handle_offset * DVec2::X), Some(bottom - horizontal_handle_offset * DVec2::X)),
		Anchor::new(left, Some(left + vertical_handle_offset * DVec2::Y), Some(left - roundness * DVec2::Y)),
	];

	bezpath_from_anchors(&anchors, true)
}

pub fn calculate_growth_factor(a: f64, turns: f64, outer_radius: f64, spiral_type: SpiralType) -> f64 {
	match spiral_type {
		SpiralType::Archimedean => {
			let total_theta = turns * TAU;
			(outer_radius - a) / total_theta
		}
		SpiralType::Logarithmic => {
			let total_theta = turns * TAU;
			((outer_radius.abs() / a).ln()) / total_theta
		}
	}
}

/// Returns a point on the given spiral type at angle `theta`.
pub fn spiral_point(theta: f64, a: f64, b: f64, spiral_type: SpiralType) -> DVec2 {
	match spiral_type {
		SpiralType::Archimedean => archimedean_spiral_point(theta, a, b),
		SpiralType::Logarithmic => log_spiral_point(theta, a, b),
	}
}

/// Returns the tangent direction at angle `theta` for the given spiral type.
fn spiral_tangent(theta: f64, a: f64, b: f64, spiral_type: SpiralType) -> DVec2 {
	match spiral_type {
		SpiralType::Archimedean => archimedean_spiral_tangent(theta, a, b),
		SpiralType::Logarithmic => log_spiral_tangent(theta, a, b),
	}
}

/// Computes arc length between two angles for the given spiral type.
fn spiral_arc_length(theta_start: f64, theta_end: f64, a: f64, b: f64, spiral_type: SpiralType) -> f64 {
	match spiral_type {
		SpiralType::Archimedean => archimedean_spiral_arc_length(theta_start, theta_end, a, b),
		SpiralType::Logarithmic => log_spiral_arc_length(theta_start, theta_end, a, b),
	}
}

/// Returns a point on a logarithmic spiral at angle `theta`.
fn log_spiral_point(theta: f64, a: f64, b: f64) -> DVec2 {
	let r = a * (b * theta).exp(); // a * e^(bθ)
	DVec2::new(r * theta.cos(), -r * theta.sin())
}

/// Computes arc length along a logarithmic spiral between two angles.
fn log_spiral_arc_length(theta_start: f64, theta_end: f64, a: f64, b: f64) -> f64 {
	let factor = (1. + b * b).sqrt();
	(a / b) * factor * ((b * theta_end).exp() - (b * theta_start).exp())
}

/// Returns the tangent direction of a logarithmic spiral at angle `theta`.
fn log_spiral_tangent(theta: f64, a: f64, b: f64) -> DVec2 {
	let r = a * (b * theta).exp();
	let dx = r * (b * theta.cos() - theta.sin());
	let dy = r * (b * theta.sin() + theta.cos());

	DVec2::new(dx, -dy).normalize_or(DVec2::X)
}

/// Returns a point on an Archimedean spiral at angle `theta`.
fn archimedean_spiral_point(theta: f64, a: f64, b: f64) -> DVec2 {
	let r = a + b * theta;
	DVec2::new(r * theta.cos(), -r * theta.sin())
}

/// Returns the tangent direction of an Archimedean spiral at angle `theta`.
fn archimedean_spiral_tangent(theta: f64, a: f64, b: f64) -> DVec2 {
	let r = a + b * theta;
	let dx = b * theta.cos() - r * theta.sin();
	let dy = b * theta.sin() + r * theta.cos();
	DVec2::new(dx, -dy).normalize_or(DVec2::X)
}

/// Computes arc length along an Archimedean spiral between two angles.
fn archimedean_spiral_arc_length(theta_start: f64, theta_end: f64, a: f64, b: f64) -> f64 {
	archimedean_spiral_arc_length_origin(theta_end, a, b) - archimedean_spiral_arc_length_origin(theta_start, a, b)
}

/// Computes arc length from origin to a point on Archimedean spiral at angle `theta`.
fn archimedean_spiral_arc_length_origin(theta: f64, a: f64, b: f64) -> f64 {
	let r = a + b * theta;
	let sqrt_term = (r * r + b * b).sqrt();
	(r * sqrt_term + b * b * ((r + sqrt_term).ln())) / (2. * b)
}
