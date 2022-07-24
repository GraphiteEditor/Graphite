use super::Subpath;

use glam::DVec2;

/// Structure used to represent a single anchor with up to two optional associated handles along a `Subpath`
pub struct ManipulatorGroup {
	pub anchor: DVec2,
	pub in_handle: Option<DVec2>,
	pub out_handle: Option<DVec2>,
}

/// Iteration structure for iterating across each curve of a `Subpath`, using an intermediate `Bezier` representation.
pub struct SubpathIter<'a> {
	pub(super) index: usize,
	pub(super) sub_path: &'a Subpath,
}

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
	/// Color of the line segments connecting anchors to handle points. Defaulted to `grey`.
	pub handle_line_stroke_color: String,
	/// Width of the line segments connecting anchors to handle points. Defaulted to `1.`.
	pub handle_line_stroke_width: f64,
	/// Stroke color outlining circles marking the handles of `Subpath`. Defaulted to `grey`.
	pub handle_point_stroke_color: String,
	/// Stroke color outlining circles marking the handles of `Subpath`. Defaulted to `1.5`.
	pub handle_point_stroke_width: f64,
	/// Radius of the circles marking the handles of `Subpath`. Defaulted to `3.`.
	pub handle_point_radius: f64,
	/// Fill color of the circles marking the handles of `Subpath`. Defaulted to `white`.
	pub handle_point_fill: String,
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
			handle_line_stroke_color: String::from("grey"),
			handle_line_stroke_width: 1.,
			handle_point_stroke_color: String::from("grey"),
			handle_point_stroke_width: 1.5,
			handle_point_radius: 3.,
			handle_point_fill: String::from("white"),
		}
	}
}
