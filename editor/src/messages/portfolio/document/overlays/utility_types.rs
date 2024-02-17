use super::utility_functions::overlay_canvas_context;
use crate::consts::{COLOR_OVERLAY_BLUE, COLOR_OVERLAY_WHITE, COLOR_OVERLAY_YELLOW, MANIPULATOR_GROUP_MARKER_SIZE, PIVOT_CROSSHAIR_LENGTH, PIVOT_CROSSHAIR_THICKNESS, PIVOT_DIAMETER};
use crate::messages::prelude::Message;

use bezier_rs::Subpath;
use graphene_core::renderer::Quad;
use graphene_core::uuid::ManipulatorGroupId;

use core::f64::consts::PI;
use glam::{DAffine2, DVec2};

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
}
// Message hashing isn't used but is required by the message system macros
impl core::hash::Hash for OverlayContext {
	fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

impl OverlayContext {
	pub fn quad(&mut self, quad: Quad) {
		self.render_context.begin_path();
		self.render_context.move_to(quad.0[3].x.round() - 0.5, quad.0[3].y.round() - 0.5);
		for i in 0..4 {
			self.render_context.line_to(quad.0[i].x.round() - 0.5, quad.0[i].y.round() - 0.5);
		}
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(COLOR_OVERLAY_BLUE));
		self.render_context.stroke();
	}

	pub fn line(&mut self, start: DVec2, end: DVec2, color: Option<&str>) {
		let start = start.round() - DVec2::splat(0.5);
		let end = end.round() - DVec2::splat(0.5);

		self.render_context.begin_path();
		self.render_context.move_to(start.x, start.y);
		self.render_context.line_to(end.x, end.y);
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(color.unwrap_or(COLOR_OVERLAY_BLUE)));
		self.render_context.stroke();
	}

	pub fn manipulator_handle(&mut self, position: DVec2, selected: bool) {
		let position = position.round() - DVec2::splat(0.5);

		self.render_context.begin_path();
		self.render_context.arc(position.x, position.y, MANIPULATOR_GROUP_MARKER_SIZE / 2., 0., PI * 2.).expect("draw circle");

		let fill = if selected { COLOR_OVERLAY_BLUE } else { COLOR_OVERLAY_WHITE };
		self.render_context.set_fill_style(&wasm_bindgen::JsValue::from_str(fill));
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(COLOR_OVERLAY_BLUE));
		self.render_context.fill();
		self.render_context.stroke();
	}

	pub fn manipulator_anchor(&mut self, position: DVec2, selected: bool, color: Option<&str>) {
		let color_stroke = color.unwrap_or(COLOR_OVERLAY_BLUE);
		let color_fill = if selected { color_stroke } else { COLOR_OVERLAY_WHITE };
		self.square(position, None, Some(color_fill), Some(color_stroke));
	}

	pub fn square(&mut self, position: DVec2, size: Option<f64>, color_fill: Option<&str>, color_stroke: Option<&str>) {
		let size = size.unwrap_or(MANIPULATOR_GROUP_MARKER_SIZE);
		let color_fill = color_fill.unwrap_or(COLOR_OVERLAY_WHITE);
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);

		let position = position.round() - DVec2::splat(0.5);
		let corner = position - DVec2::splat(size) / 2.;

		self.render_context.begin_path();
		self.render_context.rect(corner.x, corner.y, size, size);
		self.render_context.set_fill_style(&wasm_bindgen::JsValue::from_str(color_fill));
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(color_stroke));
		self.render_context.fill();
		self.render_context.stroke();
	}

	pub fn pivot(&mut self, position: DVec2) {
		let (x, y) = (position.round() - DVec2::splat(0.5)).into();

		// Circle

		self.render_context.begin_path();
		self.render_context.arc(x, y, PIVOT_DIAMETER / 2., 0., PI * 2.).expect("draw circle");
		self.render_context.set_fill_style(&wasm_bindgen::JsValue::from_str(COLOR_OVERLAY_YELLOW));
		self.render_context.fill();

		// Crosshair

		// Round line caps add half the stroke width to the length on each end, so we subtract that here before halving to get the radius
		let crosshair_radius = (PIVOT_CROSSHAIR_LENGTH - PIVOT_CROSSHAIR_THICKNESS) / 2.;

		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(COLOR_OVERLAY_YELLOW));
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

	pub fn outline<'a>(&mut self, subpaths: impl Iterator<Item = &'a Subpath<ManipulatorGroupId>>, transform: DAffine2) {
		self.render_context.begin_path();
		for subpath in subpaths {
			let mut curves = subpath.iter().peekable();

			let Some(first) = curves.peek() else {
				continue;
			};

			self.render_context.move_to(transform.transform_point2(first.start()).x, transform.transform_point2(first.start()).y);
			for curve in curves {
				match curve.handles {
					bezier_rs::BezierHandles::Linear => {
						let a = transform.transform_point2(curve.end());
						let a = a.round() - DVec2::splat(0.5);

						self.render_context.line_to(a.x, a.y)
					}
					bezier_rs::BezierHandles::Quadratic { handle } => {
						let a = transform.transform_point2(handle);
						let b = transform.transform_point2(curve.end());
						let a = a.round() - DVec2::splat(0.5);
						let b = b.round() - DVec2::splat(0.5);

						self.render_context.quadratic_curve_to(a.x, a.y, b.x, b.y)
					}
					bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => {
						let a = transform.transform_point2(handle_start);
						let b = transform.transform_point2(handle_end);
						let c = transform.transform_point2(curve.end());
						let a = a.round() - DVec2::splat(0.5);
						let b = b.round() - DVec2::splat(0.5);
						let c = c.round() - DVec2::splat(0.5);

						self.render_context.bezier_curve_to(a.x, a.y, b.x, b.y, c.x, c.y)
					}
				}
			}

			if subpath.closed() {
				self.render_context.close_path();
			}
		}

		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(COLOR_OVERLAY_BLUE));
		self.render_context.stroke();
	}

	pub fn text(&self, text: &str, pos: DVec2, background: &str, padding: f64) {
		let pos = pos.round();
		let metrics = self.render_context.measure_text(text).expect("measure text");
		self.render_context.set_fill_style(&background.into());
		self.render_context.fill_rect(
			pos.x + metrics.actual_bounding_box_left(),
			pos.y - metrics.font_bounding_box_ascent() - metrics.font_bounding_box_descent() - padding * 2.,
			metrics.actual_bounding_box_right() - metrics.actual_bounding_box_left() + padding * 2.,
			metrics.font_bounding_box_ascent() + metrics.font_bounding_box_descent() + padding * 2.,
		);
		self.render_context.set_fill_style(&"white".into());
		self.render_context
			.fill_text(text, pos.x + padding, pos.y - padding - metrics.font_bounding_box_descent())
			.expect("draw text");
	}
}
