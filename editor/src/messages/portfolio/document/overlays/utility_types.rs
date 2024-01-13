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
		self.render_context.move_to(quad.0[3].x.round(), quad.0[3].y.round());
		for i in 0..4 {
			self.render_context.line_to(quad.0[i].x.round(), quad.0[i].y.round());
		}
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(COLOR_OVERLAY_BLUE));
		self.render_context.stroke();
	}

	pub fn line(&mut self, start: DVec2, end: DVec2, color: Option<&str>) {
		self.render_context.begin_path();
		self.render_context.move_to(start.x, start.y);
		self.render_context.line_to(end.x, end.y);
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(color.unwrap_or(COLOR_OVERLAY_BLUE)));
		self.render_context.stroke();
	}

	pub fn handle(&mut self, position: DVec2, selected: bool) {
		self.render_context.begin_path();
		let position = position.round();
		self.render_context
			.arc(position.x + 0.5, position.y + 0.5, MANIPULATOR_GROUP_MARKER_SIZE / 2., 0., PI * 2.)
			.expect("draw circle");

		let fill = if selected { COLOR_OVERLAY_BLUE } else { COLOR_OVERLAY_WHITE };
		self.render_context.set_fill_style(&wasm_bindgen::JsValue::from_str(&fill));
		self.render_context.fill();
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(COLOR_OVERLAY_BLUE));
		self.render_context.stroke();
	}

	pub fn square(&mut self, position: DVec2, selected: bool) {
		self.render_context.begin_path();
		let corner = position - DVec2::splat(MANIPULATOR_GROUP_MARKER_SIZE) / 2.;
		self.render_context
			.rect(corner.x.round(), corner.y.round(), MANIPULATOR_GROUP_MARKER_SIZE, MANIPULATOR_GROUP_MARKER_SIZE);
		let fill = if selected { COLOR_OVERLAY_BLUE } else { COLOR_OVERLAY_WHITE };
		self.render_context.set_fill_style(&wasm_bindgen::JsValue::from_str(&fill));
		self.render_context.fill();
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(COLOR_OVERLAY_BLUE));
		self.render_context.stroke();
	}

	pub fn pivot(&mut self, pivot: DVec2) {
		let x = pivot.x.round();
		let y = pivot.y.round();

		self.render_context.begin_path();
		self.render_context.arc(x, y, PIVOT_DIAMETER / 2., 0., PI * 2.).expect("draw circle");
		self.render_context.set_fill_style(&wasm_bindgen::JsValue::from_str(COLOR_OVERLAY_YELLOW));
		self.render_context.fill();

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
		let transform = |point| transform.transform_point2(point);
		self.render_context.begin_path();
		for subpath in subpaths {
			let mut curves = subpath.iter().peekable();
			let Some(first) = curves.peek() else {
				continue;
			};
			self.render_context.move_to(transform(first.start()).x, transform(first.start()).y);
			for curve in curves {
				match curve.handles {
					bezier_rs::BezierHandles::Linear => self.render_context.line_to(transform(curve.end()).x, transform(curve.end()).y),
					bezier_rs::BezierHandles::Quadratic { handle } => {
						self.render_context
							.quadratic_curve_to(transform(handle).x, transform(handle).y, transform(curve.end()).x, transform(curve.end()).y)
					}
					bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => self.render_context.bezier_curve_to(
						transform(handle_start).x,
						transform(handle_start).y,
						transform(handle_end).x,
						transform(handle_end).y,
						transform(curve.end()).x,
						transform(curve.end()).y,
					),
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
