use super::*;
use crate::consts::*;

use std::fmt::Write;

/// Functionality relating to core `Subpath` operations, such as constructors and `iter`.
impl Subpath {
	/// Create a new `Subpath` using a list of [ManipulatorGroup]s.
	/// A `Subpath` with less than 2 [ManipulatorGroup]s may not be closed.
	pub fn new(manipulator_groups: Vec<ManipulatorGroup>, closed: bool) -> Self {
		assert!(!closed || manipulator_groups.len() > 1, "A closed Subpath must contain more than 1 ManipulatorGroup.");
		Self { manipulator_groups, closed }
	}

	/// Create a `Subpath` consisting of 2 manipulator groups from a `Bezier`.
	pub fn from_bezier(bezier: Bezier) -> Self {
		Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: bezier.start(),
					in_handle: None,
					out_handle: bezier.handle_start(),
				},
				ManipulatorGroup {
					anchor: bezier.end(),
					in_handle: bezier.handle_end(),
					out_handle: None,
				},
			],
			false,
		)
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
		if !self.closed {
			number_of_curves -= 1
		}
		number_of_curves
	}

	pub fn find_curve_parametric(&self, t: f64) -> (Option<Bezier>, f64) {
		assert!((0.0..=1.).contains(&t));

		let number_of_curves = self.len_segments() as f64;
		let scaled_t = t * number_of_curves;

		let target_curve_index = scaled_t.floor() as i32;
		let target_curve_t = scaled_t % 1.;

		(self.iter().nth(target_curve_index as usize), target_curve_t)
	}

	/// Returns an iterator of the [Bezier]s along the `Subpath`.
	pub fn iter(&self) -> SubpathIter {
		SubpathIter { sub_path: self, index: 0 }
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
}
