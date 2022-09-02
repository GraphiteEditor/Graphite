/// Structure to represent optional parameters that can be passed to the `into_svg` function.
pub struct ToSVGOptions {
	/// Color of the line segments along the `Subpath`. Defaulted to `black`.
	pub curve_stroke_color: String,
	/// Width of the line segments along the `Subpath`. Defaulted to `2.`.
	pub curve_stroke_width: f64,
	/// Stroke color outlining circles marking anchors on the `Subpath`. Defaulted to `black`.
	pub anchor_stroke_color: String,
	/// Stroke width outlining circles marking anchors on the `Subpath`. Defaulted to `2.`.
	pub anchor_stroke_width: f64,
	/// Radius of the circles marking anchors on the `Subpath`. Defaulted to `4.`.
	pub anchor_radius: f64,
	/// Fill color of the circles marking anchors on the `Subpath`. Defaulted to `white`.
	pub anchor_fill: String,
	/// Color of the line segments connecting anchors to handle points. Defaulted to `gray`.
	pub handle_line_stroke_color: String,
	/// Width of the line segments connecting anchors to handle points. Defaulted to `1.`.
	pub handle_line_stroke_width: f64,
	/// Stroke color outlining circles marking the handles of `Subpath`. Defaulted to `gray`.
	pub handle_point_stroke_color: String,
	/// Stroke color outlining circles marking the handles of `Subpath`. Defaulted to `1.5`.
	pub handle_point_stroke_width: f64,
	/// Radius of the circles marking the handles of `Subpath`. Defaulted to `3.`.
	pub handle_point_radius: f64,
	/// Fill color of the circles marking the handles of `Subpath`. Defaulted to `white`.
	pub handle_point_fill: String,
}

impl ToSVGOptions {
	/// Combine and format curve styling options for an SVG path.
	pub(crate) fn formatted_curve_arguments(&self) -> String {
		format!(r#"stroke="{}" stroke-width="{}" fill="none""#, self.curve_stroke_color, self.curve_stroke_width)
	}

	/// Combine and format anchor styling options an SVG circle.
	pub(crate) fn formatted_anchor_arguments(&self) -> String {
		format!(
			r#"r="{}", stroke="{}" stroke-width="{}" fill="{}""#,
			self.anchor_radius, self.anchor_stroke_color, self.anchor_stroke_width, self.anchor_fill
		)
	}

	/// Combine and format handle point styling options for an SVG circle.
	pub(crate) fn formatted_handle_point_arguments(&self) -> String {
		format!(
			r#"r="{}", stroke="{}" stroke-width="{}" fill="{}""#,
			self.handle_point_radius, self.handle_point_stroke_color, self.handle_point_stroke_width, self.handle_point_fill
		)
	}

	/// Combine and format handle line styling options an SVG path.
	pub(crate) fn formatted_handle_line_arguments(&self) -> String {
		format!(r#"stroke="{}" stroke-width="{}" fill="none""#, self.handle_line_stroke_color, self.handle_line_stroke_width)
	}
}

impl Default for ToSVGOptions {
	fn default() -> Self {
		ToSVGOptions {
			curve_stroke_color: String::from("black"),
			curve_stroke_width: 2.,
			anchor_stroke_color: String::from("black"),
			anchor_stroke_width: 2.,
			anchor_radius: 4.,
			anchor_fill: String::from("white"),
			handle_line_stroke_color: String::from("gray"),
			handle_line_stroke_width: 1.,
			handle_point_stroke_color: String::from("gray"),
			handle_point_stroke_width: 1.5,
			handle_point_radius: 3.,
			handle_point_fill: String::from("white"),
		}
	}
}
