use super::utility_functions::overlay_canvas_context;
use crate::consts::{COLOR_OVERLAY_BLUE, COLOR_OVERLAY_WHITE, COLOR_OVERLAY_YELLOW, MANIPULATOR_GROUP_MARKER_SIZE, PIVOT_CROSSHAIR_LENGTH, PIVOT_CROSSHAIR_THICKNESS, PIVOT_DIAMETER};
use crate::messages::prelude::Message;

use bezier_rs::{Bezier, Subpath};
use graphene_core::renderer::Quad;
use graphene_std::vector::{PointId, VectorData};

use core::borrow::Borrow;
use core::f64::consts::TAU;
use glam::{DAffine2, DVec2};
use wasm_bindgen::JsValue;

pub type OverlayProvider = fn(OverlayContext) -> Message;

pub fn empty_provider() -> OverlayProvider {
	|_| Message::NoOp
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OverlayContext {
	// Serde functionality isn't used but is required by the message system macros
	#[serde(skip, default = "overlay_canvas_context")]
	#[specta(skip)]
	pub render_context: web_sys::CanvasRenderingContext2d,
	pub size: DVec2,
	pub device_pixel_ratio: f64,
}
// Message hashing isn't used but is required by the message system macros
impl core::hash::Hash for OverlayContext {
	fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

impl OverlayContext {
	pub fn dpr(&self) -> f64 {
		self.device_pixel_ratio
	}

	fn offset(&self) -> DVec2 {
		DVec2::splat(0.5 / self.dpr())
	}

	pub fn align_to_pixel(&self, coord: DVec2) -> DVec2 {
		(coord * self.dpr()).round() / self.dpr() - self.offset()
	}

	pub fn quad(&mut self, quad: Quad, color_fill: Option<&str>) {
		self.render_context.begin_path();

		let start = self.align_to_pixel(quad.0[3]);
		self.render_context.move_to(start.x, start.y);

		for i in 0..4 {
			let point = self.align_to_pixel(quad.0[i]);
			self.render_context.line_to(point.x, point.y);
		}
		if let Some(color_fill) = color_fill {
			self.render_context.set_fill_style_str(color_fill);
			self.render_context.fill();
		}
		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();
	}

	pub fn line(&mut self, start: DVec2, end: DVec2, color: Option<&str>) {
		self.dashed_line(start, end, color, None)
	}

	pub fn dashed_line(&mut self, start: DVec2, end: DVec2, color: Option<&str>, dash_width: Option<f64>) {
		let start = self.align_to_pixel(start);
		let end = self.align_to_pixel(end);

		if let Some(dash_width) = dash_width {
			let scaled_dash_width = dash_width * self.dpr();
			let array = js_sys::Array::new();
			array.push(&JsValue::from(self.dpr()));
			array.push(&JsValue::from(scaled_dash_width - self.dpr()));
			self.render_context
				.set_line_dash(&JsValue::from(array))
				.map_err(|error| log::warn!("Error drawing dashed line: {:?}", error))
				.ok();
		} else {
			let array = js_sys::Array::new();
			self.render_context
				.set_line_dash(&JsValue::from(array))
				.map_err(|error| log::warn!("Error drawing dashed line: {:?}", error))
				.ok();
		}
		self.render_context.begin_path();
		self.render_context.move_to(start.x, start.y);
		self.render_context.line_to(end.x, end.y);
		self.render_context.set_stroke_style_str(color.unwrap_or(COLOR_OVERLAY_BLUE));
		self.render_context.stroke();
	}

	pub fn manipulator_handle(&mut self, position: DVec2, selected: bool) {
		let position = self.align_to_pixel(position);
		let radius = (MANIPULATOR_GROUP_MARKER_SIZE / 2.) * self.dpr();

		self.render_context.begin_path();
		self.render_context.arc(position.x, position.y, radius, 0., TAU).expect("Failed to draw the circle");

		let fill = if selected { COLOR_OVERLAY_BLUE } else { COLOR_OVERLAY_WHITE };
		self.render_context.set_fill_style_str(fill);
		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.fill();
		self.render_context.stroke();
	}

	pub fn manipulator_anchor(&mut self, position: DVec2, selected: bool, color: Option<&str>) {
		let color_stroke = color.unwrap_or(COLOR_OVERLAY_BLUE);
		let color_fill = if selected { color_stroke } else { COLOR_OVERLAY_WHITE };
		self.square(position, None, Some(color_fill), Some(color_stroke));
	}

	pub fn square(&mut self, position: DVec2, size: Option<f64>, color_fill: Option<&str>, color_stroke: Option<&str>) {
		let size = size.unwrap_or(MANIPULATOR_GROUP_MARKER_SIZE) * self.dpr();
		let color_fill = color_fill.unwrap_or(COLOR_OVERLAY_WHITE);
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);

		let position = self.align_to_pixel(position);
		let corner = position - DVec2::splat(size) / 2.;

		self.render_context.begin_path();
		self.render_context.rect(corner.x, corner.y, size, size);
		self.render_context.set_fill_style_str(color_fill);
		self.render_context.set_stroke_style_str(color_stroke);
		self.render_context.fill();
		self.render_context.stroke();
	}

	pub fn pixel(&mut self, position: DVec2, color: Option<&str>) {
		let size = self.dpr();
		let color_fill = color.unwrap_or(COLOR_OVERLAY_WHITE);

		let position = self.align_to_pixel(position);
		let corner = position - DVec2::splat(size) / 2.;

		self.render_context.begin_path();
		self.render_context.rect(corner.x, corner.y, size, size);
		self.render_context.set_fill_style_str(color_fill);
		self.render_context.fill();
	}

	pub fn circle(&mut self, position: DVec2, radius: f64, color_fill: Option<&str>, color_stroke: Option<&str>) {
		let color_fill = color_fill.unwrap_or(COLOR_OVERLAY_WHITE);
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);
		let position = self.align_to_pixel(position);
		let radius = radius * self.dpr();
		self.render_context.begin_path();
		self.render_context.arc(position.x, position.y, radius, 0., TAU).expect("Failed to draw the circle");
		self.render_context.set_fill_style_str(color_fill);
		self.render_context.set_stroke_style_str(color_stroke);
		self.render_context.fill();
		self.render_context.stroke();
	}
	pub fn pivot(&mut self, position: DVec2) {
		let position = self.align_to_pixel(position);
		let (x, y) = position.into();

		// Circle

		let radius = (PIVOT_DIAMETER / 2.) * self.dpr();
		self.render_context.begin_path();
		self.render_context.arc(x, y, radius, 0., TAU).expect("Failed to draw the circle");
		self.render_context.set_fill_style_str(COLOR_OVERLAY_YELLOW);
		self.render_context.fill();

		// Crosshair

		// Round line caps add half the stroke width to the length on each end, so we subtract that here before halving to get the radius
		let crosshair_radius = ((PIVOT_CROSSHAIR_LENGTH - PIVOT_CROSSHAIR_THICKNESS) / 2.) * self.dpr();

		self.render_context.set_stroke_style_str(COLOR_OVERLAY_YELLOW);
		self.render_context.set_line_cap("round");

		self.render_context.begin_path();
		self.render_context.move_to(x - crosshair_radius, y);
		self.render_context.line_to(x + crosshair_radius, y);
		self.render_context.stroke();

		self.render_context.begin_path();
		self.render_context.move_to(x, y - crosshair_radius);
		self.render_context.line_to(x, y + crosshair_radius);
		self.render_context.stroke();
	}

	pub fn outline_vector(&mut self, vector_data: &VectorData, transform: DAffine2) {
		self.render_context.begin_path();
		let mut last_point = None;
		for (_, bezier, start_id, end_id) in vector_data.segment_bezier_iter() {
			let move_to = last_point != Some(start_id);
			last_point = Some(end_id);

			self.bezier_command(bezier, transform, move_to);
		}

		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();
	}

	pub fn outline_bezier(&mut self, bezier: Bezier, transform: DAffine2) {
		self.render_context.begin_path();
		self.bezier_command(bezier, transform, true);
		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();
	}

	fn bezier_command(&self, bezier: Bezier, transform: DAffine2, move_to: bool) {
		let Bezier { start, end, handles } = bezier.apply_transformation(|point| transform.transform_point2(point));
		if move_to {
			self.render_context.move_to(start.x, start.y);
		}

		match handles {
			bezier_rs::BezierHandles::Linear => self.render_context.line_to(end.x, end.y),
			bezier_rs::BezierHandles::Quadratic { handle } => self.render_context.quadratic_curve_to(handle.x, handle.y, end.x, end.y),
			bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => self.render_context.bezier_curve_to(handle_start.x, handle_start.y, handle_end.x, handle_end.y, end.x, end.y),
		}
	}

	pub fn outline(&mut self, subpaths: impl Iterator<Item = impl Borrow<Subpath<PointId>>>, transform: DAffine2) {
		self.render_context.begin_path();
		for subpath in subpaths {
			let subpath = subpath.borrow();
			let mut curves = subpath.iter().peekable();

			let Some(first) = curves.peek() else {
				continue;
			};

			let start = self.align_to_pixel(transform.transform_point2(first.start()));
			self.render_context.move_to(start.x, start.y);
			for curve in curves {
				match curve.handles {
					bezier_rs::BezierHandles::Linear => {
						let end = self.align_to_pixel(transform.transform_point2(curve.end()));
						self.render_context.line_to(end.x, end.y)
					}
					bezier_rs::BezierHandles::Quadratic { handle } => {
						let a = self.align_to_pixel(transform.transform_point2(handle));
						let b = self.align_to_pixel(transform.transform_point2(curve.end()));
						self.render_context.quadratic_curve_to(a.x, a.y, b.x, b.y)
					}
					bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => {
						let a = self.align_to_pixel(transform.transform_point2(handle_start));
						let b = self.align_to_pixel(transform.transform_point2(handle_end));
						let c = self.align_to_pixel(transform.transform_point2(curve.end()));
						self.render_context.bezier_curve_to(a.x, a.y, b.x, b.y, c.x, c.y)
					}
				}
			}

			if subpath.closed() {
				self.render_context.close_path();
			}
		}

		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();
	}

	pub fn text(&self, text: &str, font_color: &str, background_color: Option<&str>, transform: DAffine2, padding: f64, pivot: [Pivot; 2]) {
		let font_size = 12.0;

		self.render_context.set_font(&format!("{}px Source Sans Pro, Arial, sans-serif", font_size));
		let metrics = self.render_context.measure_text(text).expect("Failed to measure the text dimensions");

		let x = match pivot[0] {
			Pivot::Start => padding,
			Pivot::Middle => -(metrics.actual_bounding_box_right() + metrics.actual_bounding_box_left()) / 2.,
			Pivot::End => -padding - metrics.actual_bounding_box_right() + metrics.actual_bounding_box_left(),
		};
		let y = match pivot[1] {
			Pivot::Start => padding + metrics.font_bounding_box_ascent() - metrics.font_bounding_box_descent(),
			Pivot::Middle => (metrics.font_bounding_box_ascent() + metrics.font_bounding_box_descent()) / 2.,
			Pivot::End => -padding,
		};

		let position = self.align_to_pixel(DVec2::new(x, y));
		let [a, b, c, d, e, f] = (transform * DAffine2::from_translation(position)).to_cols_array();
		self.render_context.set_transform(a, b, c, d, e, f).expect("Failed to rotate the render context to the specified angle");

		if let Some(background) = background_color {
			self.render_context.set_fill_style_str(background);
			self.render_context.fill_rect(
				-padding,
				padding,
				metrics.actual_bounding_box_right() - metrics.actual_bounding_box_left() + padding * 2.,
				metrics.font_bounding_box_descent() - metrics.font_bounding_box_ascent() - padding * 2.,
			);
		}

		self.render_context.set_fill_style_str(font_color);
		self.render_context.fill_text(text, 0., 0.).expect("Failed to draw the text at the calculated position");
		self.render_context.reset_transform().expect("Failed to reset the render context transform");
	}
}

pub enum Pivot {
	Start,
	Middle,
	End,
}
