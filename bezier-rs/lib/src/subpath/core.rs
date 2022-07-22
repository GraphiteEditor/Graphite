use super::*;

impl SubPath {
	/// Create a new SubPath using a list of ManipulatorGroups.
	/// A SubPath with less than 2 ManipulatorGroups may not be closed.
	pub fn new(manipulator_groups: Vec<ManipulatorGroup>, closed: bool) -> SubPath {
		assert!(!closed || manipulator_groups.len() > 1);
		SubPath { manipulator_groups, closed }
	}

	/// Create a subpath consisting of 2 manipulator groups from a bezier.
	pub fn from_bezier(bezier: Bezier) -> Self {
		SubPath::new(
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

	/// Returns true if and only if the subpath contains at least one manipulator point
	pub fn is_empty(&self) -> bool {
		self.manipulator_groups.is_empty()
	}

	/// Returns the number of ManipulatorGroups contained within the subpath.
	pub fn len(&self) -> usize {
		self.manipulator_groups.len()
	}

	/// Returns an iterator of the Beziers along the subpath.
	pub fn iter(&self) -> SubPathIter {
		SubPathIter { sub_path: self, index: 0 }
	}

	/// Returns an SVG representation of the `SubPath`.
	pub fn to_svg(&self, options: ToSVGOptions) -> String {
		if self.is_empty() {
			return String::new();
		}

		let subpath_options = format!(r#"stroke="{}" stroke-width="{}" fill="transparent""#, options.curve_stroke_color, options.curve_stroke_width);
		let anchor_options = format!(
			r#"r="{}", stroke="{}" stroke-width="{}" fill="{}""#,
			options.anchor_radius, options.anchor_stroke_color, options.anchor_stroke_width, options.anchor_fill
		);
		let handle_point_options = format!(
			r#"r="{}", stroke="{}" stroke-width="{}" fill="{}""#,
			options.handle_point_radius, options.handle_point_stroke_color, options.handle_point_stroke_width, options.handle_point_fill
		);
		let handle_line_options = format!(
			r#"stroke="{}" stroke-width="{}" fill="transparent""#,
			options.handle_line_stroke_color, options.handle_line_stroke_width
		);

		let anchor_circles = self
			.manipulator_groups
			.iter()
			.map(|point| format!(r#"<circle cx="{}" cy="{}" {}/>"#, point.anchor.x, point.anchor.y, anchor_options))
			.collect::<Vec<String>>();
		let mut path_pieces = vec![format!("M {} {}", self[0].anchor.x, self[0].anchor.y)];
		let mut handle_pieces = Vec::new();
		let mut handle_circles = Vec::new();

		for start_index in 0..self.len() + (self.closed as usize) - 1 {
			let end_index = (start_index + 1) % self.len();

			let start = self[start_index].anchor;
			let end = self[end_index].anchor;
			let handle1 = self[start_index].out_handle;
			let handle2 = self[end_index].in_handle;

			let main_path: String;
			if let (Some(h1), Some(h2)) = (handle1, handle2) {
				handle_pieces.push(format!("M {} {} L {} {}", start.x, start.y, h1.x, h1.y));
				handle_circles.push(format!(r#"<circle cx="{}" cy="{}" {}/>"#, h1.x, h1.y, handle_point_options));
				handle_pieces.push(format!("M {} {} L {} {}", end.x, end.y, h2.x, h2.y));
				handle_circles.push(format!(r#"<circle cx="{}" cy="{}" {}/>"#, h2.x, h2.y, handle_point_options));
				main_path = format!("C {} {} {} {}", h1.x, h1.y, h2.x, h2.y);
			} else if let Some(handle) = handle1.or(handle2) {
				handle_pieces.push(format!("M {} {} L {} {}", start.x, start.y, handle.x, handle.y));
				handle_pieces.push(format!("M {} {} L {} {}", end.x, end.y, handle.x, handle.y));
				handle_circles.push(format!(r#"<circle cx="{}" cy="{}" {}/>"#, handle.x, handle.y, handle_point_options));
				main_path = format!("Q {} {}", handle.x, handle.y);
			} else {
				main_path = String::from("L");
			}
			path_pieces.push(format!("{} {} {}", main_path, end.x, end.y));
		}
		if self.closed {
			path_pieces.push(String::from("Z"));
		}

		format!(
			r#"<path d="{}" {}/><path d="{}" {}/>{}{}"#,
			path_pieces.join(" "),
			subpath_options,
			handle_pieces.join(" "),
			handle_line_options,
			handle_circles.join(""),
			anchor_circles.join(""),
		)
	}
}
