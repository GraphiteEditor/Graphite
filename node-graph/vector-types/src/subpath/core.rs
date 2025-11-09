use super::consts::*;
use super::*;
use crate::vector::misc::{SpiralType, point_to_dvec2};
use glam::DVec2;
use kurbo::PathSeg;
use std::f64::consts::TAU;

pub struct PathSegPoints {
	pub p0: DVec2,
	pub p1: Option<DVec2>,
	pub p2: Option<DVec2>,
	pub p3: DVec2,
}

impl PathSegPoints {
	pub fn new(p0: DVec2, p1: Option<DVec2>, p2: Option<DVec2>, p3: DVec2) -> Self {
		Self { p0, p1, p2, p3 }
	}
}

pub fn pathseg_points(segment: PathSeg) -> PathSegPoints {
	match segment {
		PathSeg::Line(line) => PathSegPoints::new(point_to_dvec2(line.p0), None, None, point_to_dvec2(line.p1)),
		PathSeg::Quad(quad) => PathSegPoints::new(point_to_dvec2(quad.p0), None, Some(point_to_dvec2(quad.p1)), point_to_dvec2(quad.p2)),
		PathSeg::Cubic(cube) => PathSegPoints::new(point_to_dvec2(cube.p0), Some(point_to_dvec2(cube.p1)), Some(point_to_dvec2(cube.p2)), point_to_dvec2(cube.p3)),
	}
}

/// Functionality relating to core `Subpath` operations, such as constructors and `iter`.
impl<PointId: Identifier> Subpath<PointId> {
	/// Create a new `Subpath` using a list of [ManipulatorGroup]s.
	/// A `Subpath` with less than 2 [ManipulatorGroup]s may not be closed.
	#[track_caller]
	pub fn new(manipulator_groups: Vec<ManipulatorGroup<PointId>>, closed: bool) -> Self {
		assert!(!closed || !manipulator_groups.is_empty(), "A closed Subpath must contain more than 0 ManipulatorGroups.");
		Self { manipulator_groups, closed }
	}

	/// Create a `Subpath` consisting of 2 manipulator groups from a `Bezier`.
	pub fn from_bezier(segment: PathSeg) -> Self {
		let PathSegPoints { p0, p1, p2, p3 } = pathseg_points(segment);
		Subpath::new(vec![ManipulatorGroup::new(p0, None, p1), ManipulatorGroup::new(p3, p2, None)], false)
	}

	/// Creates a subpath from a slice of [Bezier]. When two consecutive Beziers do not share an end and start point, this function
	/// resolves the discrepancy by simply taking the start-point of the second Bezier as the anchor of the Manipulator Group.
	pub fn from_beziers(beziers: &[PathSeg], closed: bool) -> Self {
		assert!(!closed || beziers.len() > 1, "A closed Subpath must contain at least 1 Bezier.");
		if beziers.is_empty() {
			return Subpath::new(vec![], closed);
		}

		let beziers: Vec<_> = beziers.iter().map(|b| pathseg_points(*b)).collect();

		let first = beziers.first().unwrap();
		let mut manipulator_groups = vec![ManipulatorGroup {
			anchor: first.p0,
			in_handle: None,
			out_handle: first.p1,
			id: PointId::new(),
		}];
		let mut inner_groups: Vec<ManipulatorGroup<PointId>> = beziers
			.windows(2)
			.map(|bezier_pair| ManipulatorGroup {
				anchor: bezier_pair[1].p0,
				in_handle: bezier_pair[0].p2,
				out_handle: bezier_pair[1].p1,
				id: PointId::new(),
			})
			.collect::<Vec<ManipulatorGroup<PointId>>>();
		manipulator_groups.append(&mut inner_groups);

		let last = beziers.last().unwrap();
		if !closed {
			manipulator_groups.push(ManipulatorGroup {
				anchor: last.p3,
				in_handle: last.p2,
				out_handle: None,
				id: PointId::new(),
			});
			return Subpath::new(manipulator_groups, false);
		}

		manipulator_groups[0].in_handle = last.p2;
		Subpath::new(manipulator_groups, true)
	}

	/// Returns true if the `Subpath` contains no [ManipulatorGroup].
	pub fn is_empty(&self) -> bool {
		self.manipulator_groups.is_empty()
	}

	/// Returns the number of [ManipulatorGroup]s contained within the `Subpath`.
	pub fn len(&self) -> usize {
		self.manipulator_groups.len()
	}

	/// Returns the number of segments contained within the `Subpath`.
	pub fn len_segments(&self) -> usize {
		let mut number_of_curves = self.len();
		if !self.closed && number_of_curves > 0 {
			number_of_curves -= 1
		}
		number_of_curves
	}

	/// Returns a copy of the bezier segment at the given segment index, if this segment exists.
	pub fn get_segment(&self, segment_index: usize) -> Option<PathSeg> {
		if segment_index >= self.len_segments() {
			return None;
		}
		Some(self[segment_index].to_bezier(&self[(segment_index + 1) % self.len()]))
	}

	/// Returns an iterator of the [Bezier]s along the `Subpath`.
	pub fn iter(&self) -> SubpathIter<'_, PointId> {
		SubpathIter {
			subpath: self,
			index: 0,
			is_always_closed: false,
		}
	}

	/// Returns an iterator of the [Bezier]s along the `Subpath` always considering it as a closed subpath.
	pub fn iter_closed(&self) -> SubpathIter<'_, PointId> {
		SubpathIter {
			subpath: self,
			index: 0,
			is_always_closed: true,
		}
	}

	/// Returns a slice of the [ManipulatorGroup]s in the `Subpath`.
	pub fn manipulator_groups(&self) -> &[ManipulatorGroup<PointId>] {
		&self.manipulator_groups
	}

	/// Returns a mutable reference to the [ManipulatorGroup]s in the `Subpath`.
	pub fn manipulator_groups_mut(&mut self) -> &mut Vec<ManipulatorGroup<PointId>> {
		&mut self.manipulator_groups
	}

	/// Returns a vector of all the anchors (DVec2) for this `Subpath`.
	pub fn anchors(&self) -> Vec<DVec2> {
		self.manipulator_groups().iter().map(|group| group.anchor).collect()
	}

	/// Returns if the Subpath is equivalent to a single point.
	pub fn is_point(&self) -> bool {
		if self.is_empty() {
			return false;
		}
		let point = self.manipulator_groups[0].anchor;
		self.manipulator_groups
			.iter()
			.all(|manipulator_group| manipulator_group.anchor.abs_diff_eq(point, MAX_ABSOLUTE_DIFFERENCE))
	}

	/// Construct a [Subpath] from an iter of anchor positions.
	pub fn from_anchors(anchor_positions: impl IntoIterator<Item = DVec2>, closed: bool) -> Self {
		Self::new(anchor_positions.into_iter().map(|anchor| ManipulatorGroup::new_anchor(anchor)).collect(), closed)
	}

	pub fn from_anchors_linear(anchor_positions: impl IntoIterator<Item = DVec2>, closed: bool) -> Self {
		Self::new(anchor_positions.into_iter().map(|anchor| ManipulatorGroup::new_anchor_linear(anchor)).collect(), closed)
	}

	/// Constructs a rectangle with `corner1` and `corner2` as the two corners.
	pub fn new_rect(corner1: DVec2, corner2: DVec2) -> Self {
		Self::from_anchors_linear([corner1, DVec2::new(corner2.x, corner1.y), corner2, DVec2::new(corner1.x, corner2.y)], true)
	}

	/// Constructs a rounded rectangle with `corner1` and `corner2` as the two corners and `corner_radii` as the radii of the corners: `[top_left, top_right, bottom_right, bottom_left]`.
	pub fn new_rounded_rect(corner1: DVec2, corner2: DVec2, corner_radii: [f64; 4]) -> Self {
		if corner_radii.iter().all(|radii| radii.abs() < f64::EPSILON * 100.) {
			return Self::new_rect(corner1, corner2);
		}

		use std::f64::consts::{FRAC_1_SQRT_2, PI};

		let new_arc = |center: DVec2, corner: DVec2, radius: f64| -> Vec<ManipulatorGroup<PointId>> {
			let point1 = center + DVec2::from_angle(-PI * 0.25).rotate(corner - center) * FRAC_1_SQRT_2;
			let point2 = center + DVec2::from_angle(PI * 0.25).rotate(corner - center) * FRAC_1_SQRT_2;
			if radius == 0. {
				return vec![ManipulatorGroup::new_anchor(point1), ManipulatorGroup::new_anchor(point2)];
			}

			// Based on https://pomax.github.io/bezierinfo/#circles_cubic
			const HANDLE_OFFSET_FACTOR: f64 = 0.551784777779014;
			let handle_offset = radius * HANDLE_OFFSET_FACTOR;
			vec![
				ManipulatorGroup::new(point1, None, Some(point1 + handle_offset * (corner - point1).normalize())),
				ManipulatorGroup::new(point2, Some(point2 + handle_offset * (corner - point2).normalize()), None),
			]
		};
		Self::new(
			[
				new_arc(DVec2::new(corner1.x + corner_radii[0], corner1.y + corner_radii[0]), DVec2::new(corner1.x, corner1.y), corner_radii[0]),
				new_arc(DVec2::new(corner2.x - corner_radii[1], corner1.y + corner_radii[1]), DVec2::new(corner2.x, corner1.y), corner_radii[1]),
				new_arc(DVec2::new(corner2.x - corner_radii[2], corner2.y - corner_radii[2]), DVec2::new(corner2.x, corner2.y), corner_radii[2]),
				new_arc(DVec2::new(corner1.x + corner_radii[3], corner2.y - corner_radii[3]), DVec2::new(corner1.x, corner2.y), corner_radii[3]),
			]
			.concat(),
			true,
		)
	}

	/// Constructs an ellipse with `corner1` and `corner2` as the two corners of the bounding box.
	pub fn new_ellipse(corner1: DVec2, corner2: DVec2) -> Self {
		let size = (corner1 - corner2).abs();
		let center = (corner1 + corner2) / 2.;
		let top = DVec2::new(center.x, corner1.y);
		let bottom = DVec2::new(center.x, corner2.y);
		let left = DVec2::new(corner1.x, center.y);
		let right = DVec2::new(corner2.x, center.y);

		// Based on https://pomax.github.io/bezierinfo/#circles_cubic
		const HANDLE_OFFSET_FACTOR: f64 = 0.551784777779014;
		let handle_offset = size * HANDLE_OFFSET_FACTOR * 0.5;

		let manipulator_groups = vec![
			ManipulatorGroup::new(top, Some(top - handle_offset * DVec2::X), Some(top + handle_offset * DVec2::X)),
			ManipulatorGroup::new(right, Some(right - handle_offset * DVec2::Y), Some(right + handle_offset * DVec2::Y)),
			ManipulatorGroup::new(bottom, Some(bottom + handle_offset * DVec2::X), Some(bottom - handle_offset * DVec2::X)),
			ManipulatorGroup::new(left, Some(left + handle_offset * DVec2::Y), Some(left - handle_offset * DVec2::Y)),
		];
		Self::new(manipulator_groups, true)
	}

	/// Constructs an arc by a `radius`, `angle_start` and `angle_size`. Angles must be in radians. Slice option makes it look like pie or pacman.
	pub fn new_arc(radius: f64, start_angle: f64, sweep_angle: f64, arc_type: ArcType) -> Self {
		// Prevents glitches from numerical imprecision that have been observed during animation playback after about a minute
		let start_angle = start_angle % (std::f64::consts::TAU * 2.);
		let sweep_angle = sweep_angle % (std::f64::consts::TAU * 2.);

		let original_start_angle = start_angle;
		let sweep_angle_sign = sweep_angle.signum();

		let mut start_angle = 0.;
		let mut sweep_angle = sweep_angle.abs();

		if (sweep_angle / std::f64::consts::TAU).floor() as u32 % 2 == 0 {
			sweep_angle %= std::f64::consts::TAU;
		} else {
			start_angle = sweep_angle % std::f64::consts::TAU;
			sweep_angle = std::f64::consts::TAU - start_angle;
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

		let mut manipulator_groups = Vec::with_capacity(segments);
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

			manipulator_groups.push(ManipulatorGroup::new(start, prev_in_handle, Some(handle_start)));
			prev_in_handle = Some(handle_end);
			prev_end = end;
		}
		manipulator_groups.push(ManipulatorGroup::new(prev_end, prev_in_handle, None));

		if slice {
			manipulator_groups.push(ManipulatorGroup::new(center, None, None));
		}

		Self::new(manipulator_groups, closed || slice)
	}

	/// Constructs a regular polygon (ngon). Based on `sides` and `radius`, which is the distance from the center to any vertex.
	pub fn new_regular_polygon(center: DVec2, sides: u64, radius: f64) -> Self {
		let sides = sides.max(3);
		let angle_increment = std::f64::consts::TAU / (sides as f64);
		let anchor_positions = (0..sides).map(|i| {
			let angle = (i as f64) * angle_increment - std::f64::consts::FRAC_PI_2;
			let center = center + DVec2::ONE * radius;
			DVec2::new(center.x + radius * f64::cos(angle), center.y + radius * f64::sin(angle)) * 0.5
		});
		Self::from_anchors(anchor_positions, true)
	}

	/// Constructs a star polygon (n-star). See [new_regular_polygon], but with interspersed vertices at an `inner_radius`.
	pub fn new_star_polygon(center: DVec2, sides: u64, radius: f64, inner_radius: f64) -> Self {
		let sides = sides.max(2);
		let angle_increment = 0.5 * std::f64::consts::TAU / (sides as f64);
		let anchor_positions = (0..sides * 2).map(|i| {
			let angle = (i as f64) * angle_increment - std::f64::consts::FRAC_PI_2;
			let center = center + DVec2::ONE * radius;
			let r = if i % 2 == 0 { radius } else { inner_radius };
			DVec2::new(center.x + r * f64::cos(angle), center.y + r * f64::sin(angle)) * 0.5
		});
		Self::from_anchors(anchor_positions, true)
	}

	/// Constructs a line from `p1` to `p2`
	pub fn new_line(p1: DVec2, p2: DVec2) -> Self {
		Self::from_anchors([p1, p2], false)
	}

	pub fn new_spiral(a: f64, outer_radius: f64, turns: f64, start_angle: f64, delta_theta: f64, spiral_type: SpiralType) -> Self {
		let mut manipulator_groups = Vec::new();
		let mut prev_in_handle = None;
		let theta_end = turns * std::f64::consts::TAU + start_angle;

		let b = calculate_b(a, turns, outer_radius, spiral_type);

		let mut theta = start_angle;
		while theta < theta_end {
			let theta_next = f64::min(theta + delta_theta, theta_end);

			let p0 = spiral_point(theta, a, b, spiral_type);
			let p3 = spiral_point(theta_next, a, b, spiral_type);
			let t0 = spiral_tangent(theta, a, b, spiral_type);
			let t1 = spiral_tangent(theta_next, a, b, spiral_type);

			let arc_len = spiral_arc_length(theta, theta_next, a, b, spiral_type);
			let d = arc_len / 3.;

			let p1 = p0 + d * t0;
			let p2 = p3 - d * t1;

			manipulator_groups.push(ManipulatorGroup::new(p0, prev_in_handle, Some(p1)));
			prev_in_handle = Some(p2);

			// If final segment, end with anchor at theta_end
			if (theta_next - theta_end).abs() < f64::EPSILON {
				manipulator_groups.push(ManipulatorGroup::new(p3, prev_in_handle, None));
				break;
			}

			theta = theta_next;
		}

		Self::new(manipulator_groups, false)
	}
}

pub fn calculate_b(a: f64, turns: f64, outer_radius: f64, spiral_type: SpiralType) -> f64 {
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
pub fn spiral_tangent(theta: f64, a: f64, b: f64, spiral_type: SpiralType) -> DVec2 {
	match spiral_type {
		SpiralType::Archimedean => archimedean_spiral_tangent(theta, a, b),
		SpiralType::Logarithmic => log_spiral_tangent(theta, a, b),
	}
}

/// Computes arc length between two angles for the given spiral type.
pub fn spiral_arc_length(theta_start: f64, theta_end: f64, a: f64, b: f64, spiral_type: SpiralType) -> f64 {
	match spiral_type {
		SpiralType::Archimedean => archimedean_spiral_arc_length(theta_start, theta_end, a, b),
		SpiralType::Logarithmic => log_spiral_arc_length(theta_start, theta_end, a, b),
	}
}

/// Returns a point on a logarithmic spiral at angle `theta`.
pub fn log_spiral_point(theta: f64, a: f64, b: f64) -> DVec2 {
	let r = a * (b * theta).exp(); // a * e^(bθ)
	DVec2::new(r * theta.cos(), -r * theta.sin())
}

/// Computes arc length along a logarithmic spiral between two angles.
pub fn log_spiral_arc_length(theta_start: f64, theta_end: f64, a: f64, b: f64) -> f64 {
	let factor = (1. + b * b).sqrt();
	(a / b) * factor * ((b * theta_end).exp() - (b * theta_start).exp())
}

/// Returns the tangent direction of a logarithmic spiral at angle `theta`.
pub fn log_spiral_tangent(theta: f64, a: f64, b: f64) -> DVec2 {
	let r = a * (b * theta).exp();
	let dx = r * (b * theta.cos() - theta.sin());
	let dy = r * (b * theta.sin() + theta.cos());

	DVec2::new(dx, -dy).normalize_or(DVec2::X)
}

/// Returns a point on an Archimedean spiral at angle `theta`.
pub fn archimedean_spiral_point(theta: f64, a: f64, b: f64) -> DVec2 {
	let r = a + b * theta;
	DVec2::new(r * theta.cos(), -r * theta.sin())
}

/// Returns the tangent direction of an Archimedean spiral at angle `theta`.
pub fn archimedean_spiral_tangent(theta: f64, a: f64, b: f64) -> DVec2 {
	let r = a + b * theta;
	let dx = b * theta.cos() - r * theta.sin();
	let dy = b * theta.sin() + r * theta.cos();
	DVec2::new(dx, -dy).normalize_or(DVec2::X)
}

/// Computes arc length along an Archimedean spiral between two angles.
pub fn archimedean_spiral_arc_length(theta_start: f64, theta_end: f64, a: f64, b: f64) -> f64 {
	archimedean_spiral_arc_length_origin(theta_end, a, b) - archimedean_spiral_arc_length_origin(theta_start, a, b)
}

/// Computes arc length from origin to a point on Archimedean spiral at angle `theta`.
pub fn archimedean_spiral_arc_length_origin(theta: f64, a: f64, b: f64) -> f64 {
	let r = a + b * theta;
	let sqrt_term = (r * r + b * b).sqrt();
	(r * sqrt_term + b * b * ((r + sqrt_term).ln())) / (2. * b)
}
