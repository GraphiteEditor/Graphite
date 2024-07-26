use super::*;
use crate::consts::*;

use glam::DVec2;
use std::fmt::Write;

/// Functionality relating to core `Subpath` operations, such as constructors and `iter`.
impl<PointId: crate::Identifier> Subpath<PointId> {
	/// Create a new `Subpath` using a list of [ManipulatorGroup]s.
	/// A `Subpath` with less than 2 [ManipulatorGroup]s may not be closed.
	#[track_caller]
	pub fn new(manipulator_groups: Vec<ManipulatorGroup<PointId>>, closed: bool) -> Self {
		assert!(!closed || manipulator_groups.len() > 1, "A closed Subpath must contain more than 1 ManipulatorGroup.");
		Self { manipulator_groups, closed }
	}

	/// Create a `Subpath` consisting of 2 manipulator groups from a `Bezier`.
	pub fn from_bezier(bezier: &Bezier) -> Self {
		Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: bezier.start(),
					in_handle: None,
					out_handle: bezier.handle_start(),
					id: PointId::new(),
				},
				ManipulatorGroup {
					anchor: bezier.end(),
					in_handle: bezier.handle_end(),
					out_handle: None,
					id: PointId::new(),
				},
			],
			false,
		)
	}

	/// Creates a subpath from a slice of [Bezier]. When two consecutive Beziers do not share an end and start point, this function
	/// resolves the discrepancy by simply taking the start-point of the second Bezier as the anchor of the Manipulator Group.
	pub fn from_beziers(beziers: &[Bezier], closed: bool) -> Self {
		assert!(!closed || beziers.len() > 1, "A closed Subpath must contain at least 1 Bezier.");
		if beziers.is_empty() {
			return Subpath::new(vec![], closed);
		}

		let first = beziers.first().unwrap();
		let mut manipulator_groups = vec![ManipulatorGroup {
			anchor: first.start(),
			in_handle: None,
			out_handle: first.handle_start(),
			id: PointId::new(),
		}];
		let mut inner_groups: Vec<ManipulatorGroup<PointId>> = beziers
			.windows(2)
			.map(|bezier_pair| ManipulatorGroup {
				anchor: bezier_pair[1].start(),
				in_handle: bezier_pair[0].handle_end(),
				out_handle: bezier_pair[1].handle_start(),
				id: PointId::new(),
			})
			.collect::<Vec<ManipulatorGroup<PointId>>>();
		manipulator_groups.append(&mut inner_groups);

		let last = beziers.last().unwrap();
		if !closed {
			manipulator_groups.push(ManipulatorGroup {
				anchor: last.end(),
				in_handle: last.handle_end(),
				out_handle: None,
				id: PointId::new(),
			});
			return Subpath::new(manipulator_groups, false);
		}

		manipulator_groups[0].in_handle = last.handle_end();
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
	pub fn get_segment(&self, segment_index: usize) -> Option<Bezier> {
		if segment_index >= self.len_segments() {
			return None;
		}
		Some(self[segment_index].to_bezier(&self[(segment_index + 1) % self.len()]))
	}

	/// Returns an iterator of the [Bezier]s along the `Subpath`.
	pub fn iter(&self) -> SubpathIter<PointId> {
		SubpathIter {
			subpath: self,
			index: 0,
			is_always_closed: false,
		}
	}

	/// Returns an iterator of the [Bezier]s along the `Subpath` always considering it as a closed subpath.
	pub fn iter_closed(&self) -> SubpathIter<PointId> {
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

	/// Appends to the `svg` mutable string with an SVG shape representation of the curve.
	pub fn curve_to_svg(&self, svg: &mut String, attributes: String) {
		let curve_start_argument = format!("{SVG_ARG_MOVE}{} {}", self[0].anchor.x, self[0].anchor.y);
		let mut curve_arguments: Vec<String> = self.iter().map(|bezier| bezier.svg_curve_argument()).collect();
		if self.closed {
			curve_arguments.push(String::from(SVG_ARG_CLOSED));
		}

		let _ = write!(svg, r#"<path d="{} {}" {attributes}/>"#, curve_start_argument, curve_arguments.join(" "));
	}

	/// Write the curve argument to the string (the d="..." part)
	pub fn subpath_to_svg(&self, svg: &mut String, transform: glam::DAffine2) -> std::fmt::Result {
		if self.is_empty() {
			return Ok(());
		}
		let start = transform.transform_point2(self[0].anchor);
		write!(svg, "{SVG_ARG_MOVE}{:.6},{:.6}", start.x, start.y)?;
		for bezier in self.iter() {
			bezier.apply_transformation(|pos| transform.transform_point2(pos)).write_curve_argument(svg)?;
			svg.push(' ');
		}
		if self.closed {
			svg.push_str(SVG_ARG_CLOSED);
		}
		Ok(())
	}

	/// Appends to the `svg` mutable string with an SVG shape representation of the handle lines.
	pub fn handle_lines_to_svg(&self, svg: &mut String, attributes: String) {
		let handle_lines: Vec<String> = self.iter().filter_map(|bezier| bezier.svg_handle_line_argument()).collect();
		let _ = write!(svg, r#"<path d="{}" {attributes}/>"#, handle_lines.join(" "));
	}

	/// Appends to the `svg` mutable string with an SVG shape representation of the anchors.
	pub fn anchors_to_svg(&self, svg: &mut String, attributes: String) {
		let anchors = self
			.manipulator_groups
			.iter()
			.map(|point| format!(r#"<circle cx="{}" cy="{}" {attributes}/>"#, point.anchor.x, point.anchor.y))
			.collect::<Vec<String>>();
		let _ = write!(svg, "{}", anchors.concat());
	}

	/// Appends to the `svg` mutable string with an SVG shape representation of the handles.
	pub fn handles_to_svg(&self, svg: &mut String, attributes: String) {
		let handles = self
			.manipulator_groups
			.iter()
			.flat_map(|group| [group.in_handle, group.out_handle])
			.flatten()
			.map(|handle| format!(r#"<circle cx="{}" cy="{}" {attributes}/>"#, handle.x, handle.y))
			.collect::<Vec<String>>();
		let _ = write!(svg, "{}", handles.concat());
	}

	/// Returns an SVG representation of the `Subpath`.
	/// Appends to the `svg` mutable string with an SVG shape representation that includes the curve, the handle lines, the anchors, and the handles.
	pub fn to_svg(&self, svg: &mut String, curve_attributes: String, anchor_attributes: String, handle_attributes: String, handle_line_attributes: String) {
		if !curve_attributes.is_empty() {
			self.curve_to_svg(svg, curve_attributes);
		}
		if !handle_line_attributes.is_empty() {
			self.handle_lines_to_svg(svg, handle_line_attributes);
		}
		if !anchor_attributes.is_empty() {
			self.anchors_to_svg(svg, anchor_attributes);
		}
		if !handle_attributes.is_empty() {
			self.handles_to_svg(svg, handle_attributes);
		}
	}

	/// Construct a [Subpath] from an iter of anchor positions.
	pub fn from_anchors(anchor_positions: impl IntoIterator<Item = DVec2>, closed: bool) -> Self {
		Self::new(anchor_positions.into_iter().map(|anchor| ManipulatorGroup::new_anchor(anchor)).collect(), closed)
	}

	/// Constructs a rectangle with `corner1` and `corner2` as the two corners.
	pub fn new_rect(corner1: DVec2, corner2: DVec2) -> Self {
		Self::from_anchors([corner1, DVec2::new(corner2.x, corner1.y), corner2, DVec2::new(corner1.x, corner2.y)], true)
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

	/// Constructs a regular polygon (ngon). Based on `sides` and `radius`, which is the distance from the center to any vertex.
	pub fn new_regular_polygon(center: DVec2, sides: u64, radius: f64) -> Self {
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

	/// Construct a cubic spline from a list of points.
	/// Based on <https://mathworld.wolfram.com/CubicSpline.html>.
	pub fn new_cubic_spline(points: Vec<DVec2>) -> Self {
		if points.len() < 2 {
			return Self::new(Vec::new(), false);
		}

		// Number of points = number of points to find handles for
		let len_points = points.len();

		let out_handles = solve_spline_first_handle(&points);

		let mut subpath = Subpath::new(Vec::new(), false);

		// given the second point in the n'th cubic bezier, the third point is given by 2 * points[n+1] - b[n+1].
		// to find 'handle1_pos' for the n'th point we need the n-1 cubic bezier
		subpath.manipulator_groups.push(ManipulatorGroup::new(points[0], None, Some(out_handles[0])));
		for i in 1..len_points - 1 {
			subpath
				.manipulator_groups
				.push(ManipulatorGroup::new(points[i], Some(2. * points[i] - out_handles[i]), Some(out_handles[i])));
		}
		subpath
			.manipulator_groups
			.push(ManipulatorGroup::new(points[len_points - 1], Some(2. * points[len_points - 1] - out_handles[len_points - 1]), None));

		subpath
	}

	#[cfg(feature = "kurbo")]
	pub fn to_vello_path(&self, transform: glam::DAffine2, path: &mut kurbo::BezPath) {
		use crate::BezierHandles;

		let to_point = |p: DVec2| {
			let p = transform.transform_point2(p);
			kurbo::Point::new(p.x, p.y)
		};
		path.move_to(to_point(self.iter().next().unwrap().start));
		for segment in self.iter() {
			match segment.handles {
				BezierHandles::Linear => path.line_to(to_point(segment.end)),
				BezierHandles::Quadratic { handle } => path.quad_to(to_point(handle), to_point(segment.end)),
				BezierHandles::Cubic { handle_start, handle_end } => path.curve_to(to_point(handle_start), to_point(handle_end), to_point(segment.end)),
			}
		}
		if self.closed {
			path.close_path();
		}
	}
}

pub fn solve_spline_first_handle(points: &[DVec2]) -> Vec<DVec2> {
	let len_points = points.len();

	// Matrix coefficients a, b and c (see https://mathworld.wolfram.com/CubicSpline.html).
	// Because the 'a' coefficients are all 1, they need not be stored.
	// This algorithm does a variation of the above algorithm.
	// Instead of using the traditional cubic (a + bt + ct^2 + dt^3), we use the bezier cubic.

	let mut b = vec![DVec2::new(4., 4.); len_points];
	b[0] = DVec2::new(2., 2.);
	b[len_points - 1] = DVec2::new(2., 2.);

	let mut c = vec![DVec2::new(1., 1.); len_points];

	// 'd' is the the second point in a cubic bezier, which is what we solve for
	let mut d = vec![DVec2::ZERO; len_points];

	d[0] = DVec2::new(2. * points[1].x + points[0].x, 2. * points[1].y + points[0].y);
	d[len_points - 1] = DVec2::new(3. * points[len_points - 1].x, 3. * points[len_points - 1].y);
	for idx in 1..(len_points - 1) {
		d[idx] = DVec2::new(4. * points[idx].x + 2. * points[idx + 1].x, 4. * points[idx].y + 2. * points[idx + 1].y);
	}

	// Solve with Thomas algorithm (see https://en.wikipedia.org/wiki/Tridiagonal_matrix_algorithm)
	// Now we do row operations to eliminate `a` coefficients.
	c[0] /= -b[0];
	d[0] /= -b[0];
	#[allow(clippy::assign_op_pattern)]
	for i in 1..len_points {
		b[i] += c[i - 1];
		// For some reason this `+=` version makes the borrow checker mad:
		// d[i] += d[i-1]
		d[i] = d[i] + d[i - 1];
		c[i] /= -b[i];
		d[i] /= -b[i];
	}

	// At this point b[i] == -a[i + 1] and a[i] == 0.
	// Now we do row operations to eliminate 'c' coefficients and solve.
	d[len_points - 1] *= -1.;
	#[allow(clippy::assign_op_pattern)]
	for i in (0..len_points - 1).rev() {
		d[i] = d[i] - (c[i] * d[i + 1]);
		d[i] *= -1.; // d[i] /= b[i]
	}

	d
}
