use super::*;
use crate::consts::*;

/// Functionality relating to core `Subpath` operations, such as constructors and `iter`.
impl Subpath {
	/// Create a new `Subpath` using a list of [ManipulatorGroup]s.
	/// A `Subpath` with less than 2 [ManipulatorGroup]s may not be closed.
	pub fn new(manipulator_groups: Vec<ManipulatorGroup>, closed: bool) -> Subpath {
		assert!(!closed || manipulator_groups.len() > 1);
		Subpath { manipulator_groups, closed }
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

	/// Returns true if and only if the `Subpath` contains at least one [ManipulatorGroup].
	pub fn is_empty(&self) -> bool {
		self.manipulator_groups.is_empty()
	}

	/// Returns the number of [ManipulatorGroup]s contained within the `Subpath`.
	pub fn len(&self) -> usize {
		self.manipulator_groups.len()
	}

	/// Returns an iterator of the [Bezier]s along the `Subpath`.
	pub fn iter(&self) -> SubpathIter {
		SubpathIter { sub_path: self, index: 0 }
	}

	/// Returns an SVG representation of the `Subpath`.
	pub fn to_svg(&self, options: ToSVGOptions) -> String {
		if self.is_empty() {
			return String::new();
		}

		let subpath_options = format!(r#"stroke="{}" stroke-width="{}" fill="none""#, options.curve_stroke_color, options.curve_stroke_width);
		let anchor_options = format!(
			r#"r="{}", stroke="{}" stroke-width="{}" fill="{}""#,
			options.anchor_radius, options.anchor_stroke_color, options.anchor_stroke_width, options.anchor_fill
		);
		let handle_point_options = format!(
			r#"r="{}", stroke="{}" stroke-width="{}" fill="{}""#,
			options.handle_point_radius, options.handle_point_stroke_color, options.handle_point_stroke_width, options.handle_point_fill
		);
		let handle_line_options = format!(r#"stroke="{}" stroke-width="{}" fill="none""#, options.handle_line_stroke_color, options.handle_line_stroke_width);

		let curve_start_argument = format!("{SVG_ARG_MOVE}{} {}", self[0].anchor.x, self[0].anchor.y);
		let mut curve_arguments: Vec<String> = self.iter().map(|bezier| bezier.svg_curve_argument()).collect();
		if self.closed {
			curve_arguments.push(String::from(SVG_ARG_CLOSED));
		}

		let anchor_circles = self
			.manipulator_groups
			.iter()
			.map(|point| format!(r#"<circle cx="{}" cy="{}" {}/>"#, point.anchor.x, point.anchor.y, anchor_options))
			.collect::<Vec<String>>();
		let handle_circles: Vec<String> = self
			.manipulator_groups
			.iter()
			.flat_map(|group| [group.in_handle, group.out_handle])
			.flatten()
			.map(|handle| format!(r#"<circle cx="{}" cy="{}" {}/>"#, handle.x, handle.y, handle_point_options))
			.collect();
		let handle_pieces: Vec<String> = self.iter().filter_map(|bezier| bezier.svg_handle_line_argument()).collect();

		format!(
			r#"<path d="{} {}" {}/><path d="{}" {}/>{}{}"#,
			curve_start_argument,
			curve_arguments.join(" "),
			subpath_options,
			handle_pieces.join(" "),
			handle_line_options,
			handle_circles.join(""),
			anchor_circles.join(""),
		)
	}
}
