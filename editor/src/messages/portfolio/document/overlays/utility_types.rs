use super::utility_functions::overlay_canvas_context;
use crate::consts::{
	COLOR_OVERLAY_BLUE, COLOR_OVERLAY_TRANSPARENT, COLOR_OVERLAY_WHITE, COLOR_OVERLAY_YELLOW, MANIPULATOR_GROUP_MARKER_SIZE, PIVOT_CROSSHAIR_LENGTH, PIVOT_CROSSHAIR_THICKNESS, PIVOT_DIAMETER,
};
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
	// The device pixel ratio is a property provided by the browser window and is the CSS pixel size divided by the physical monitor's pixel size.
	// It allows better pixel density of visualizations on high-DPI displays where the OS display scaling is not 100%, or where the browser is zoomed.
	pub device_pixel_ratio: f64,
}
// Message hashing isn't used but is required by the message system macros
impl core::hash::Hash for OverlayContext {
	fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

impl OverlayContext {
	pub fn quad(&mut self, quad: Quad, color_fill: Option<&str>) {
		self.dashed_quad(quad, color_fill, None, None, None);
	}

	pub fn dashed_quad(&mut self, quad: Quad, color_fill: Option<&str>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
		self.start_dpi_aware_transform();

		// Set the dash pattern
		if let Some(dash_width) = dash_width {
			let dash_gap_width = dash_gap_width.unwrap_or(1.);
			let array = js_sys::Array::new();
			array.push(&JsValue::from(dash_width));
			array.push(&JsValue::from(dash_gap_width));

			if let Some(dash_offset) = dash_offset {
				if dash_offset != 0. {
					self.render_context.set_line_dash_offset(dash_offset);
				}
			}

			self.render_context
				.set_line_dash(&JsValue::from(array))
				.map_err(|error| log::warn!("Error drawing dashed line: {:?}", error))
				.ok();
		}

		self.render_context.begin_path();
		self.render_context.move_to(quad.0[3].x.round() - 0.5, quad.0[3].y.round() - 0.5);

		for i in 0..4 {
			self.render_context.line_to(quad.0[i].x.round() - 0.5, quad.0[i].y.round() - 0.5);
		}

		if let Some(color_fill) = color_fill {
			self.render_context.set_fill_style_str(color_fill);
			self.render_context.fill();
		}

		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();

		// Reset the dash pattern back to solid
		if dash_width.is_some() {
			self.render_context
				.set_line_dash(&JsValue::from(js_sys::Array::new()))
				.map_err(|error| log::warn!("Error drawing dashed line: {:?}", error))
				.ok();
		}
		if dash_offset.is_some() && dash_offset != Some(0.) {
			self.render_context.set_line_dash_offset(0.);
		}

		self.end_dpi_aware_transform();
	}

	pub fn line(&mut self, start: DVec2, end: DVec2, color: Option<&str>) {
		self.dashed_line(start, end, color, None, None, None)
	}

	pub fn dashed_line(&mut self, start: DVec2, end: DVec2, color: Option<&str>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
		self.start_dpi_aware_transform();

		// Set the dash pattern
		if let Some(dash_width) = dash_width {
			let dash_gap_width = dash_gap_width.unwrap_or(1.);
			let array = js_sys::Array::new();
			array.push(&JsValue::from(dash_width));
			array.push(&JsValue::from(dash_gap_width));

			if let Some(dash_offset) = dash_offset {
				if dash_offset != 0. {
					self.render_context.set_line_dash_offset(dash_offset);
				}
			}

			self.render_context
				.set_line_dash(&JsValue::from(array))
				.map_err(|error| log::warn!("Error drawing dashed line: {:?}", error))
				.ok();
		}

		let start = start.round() - DVec2::splat(0.5);
		let end = end.round() - DVec2::splat(0.5);

		self.render_context.begin_path();
		self.render_context.move_to(start.x, start.y);
		self.render_context.line_to(end.x, end.y);
		self.render_context.set_stroke_style_str(color.unwrap_or(COLOR_OVERLAY_BLUE));
		self.render_context.stroke();

		// Reset the dash pattern back to solid
		if dash_width.is_some() {
			self.render_context
				.set_line_dash(&JsValue::from(js_sys::Array::new()))
				.map_err(|error| log::warn!("Error drawing dashed line: {:?}", error))
				.ok();
		}
		if dash_offset.is_some() && dash_offset != Some(0.) {
			self.render_context.set_line_dash_offset(0.);
		}

		self.end_dpi_aware_transform();
	}

	pub fn manipulator_handle(&mut self, position: DVec2, selected: bool, color: Option<&str>) {
		self.start_dpi_aware_transform();

		let position = position.round() - DVec2::splat(0.5);

		self.render_context.begin_path();
		self.render_context
			.arc(position.x, position.y, MANIPULATOR_GROUP_MARKER_SIZE / 2., 0., TAU)
			.expect("Failed to draw the circle");

		let fill = if selected { COLOR_OVERLAY_BLUE } else { COLOR_OVERLAY_WHITE };
		self.render_context.set_fill_style_str(fill);
		self.render_context.set_stroke_style_str(color.unwrap_or(COLOR_OVERLAY_BLUE));
		self.render_context.fill();
		self.render_context.stroke();

		self.end_dpi_aware_transform();
	}

	pub fn manipulator_anchor(&mut self, position: DVec2, selected: bool, color: Option<&str>) {
		let color_stroke = color.unwrap_or(COLOR_OVERLAY_BLUE);
		let color_fill = if selected { color_stroke } else { COLOR_OVERLAY_WHITE };
		self.square(position, None, Some(color_fill), Some(color_stroke));
	}

	/// Transforms the canvas context to adjust for DPI scaling
	///
	/// Overwrites all existing tranforms. This operation can be reversed with [`Self::reset_transform`].
	fn start_dpi_aware_transform(&self) {
		let [a, b, c, d, e, f] = DAffine2::from_scale(DVec2::splat(self.device_pixel_ratio)).to_cols_array();
		self.render_context
			.set_transform(a, b, c, d, e, f)
			.expect("transform should be able to be set to be able to account for DPI");
	}

	/// Un-transforms the Canvas context to adjust for DPI scaling
	///
	/// Warning: this function doesn't only reset the DPI scaling adjustment, it resets the entire transform.
	fn end_dpi_aware_transform(&self) {
		self.render_context.reset_transform().expect("transform should be able to be reset to be able to account for DPI");
	}

	pub fn square(&mut self, position: DVec2, size: Option<f64>, color_fill: Option<&str>, color_stroke: Option<&str>) {
		let size = size.unwrap_or(MANIPULATOR_GROUP_MARKER_SIZE);
		let color_fill = color_fill.unwrap_or(COLOR_OVERLAY_WHITE);
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);

		let position = position.round() - DVec2::splat(0.5);
		let corner = position - DVec2::splat(size) / 2.;

		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		self.render_context.rect(corner.x, corner.y, size, size);
		self.render_context.set_fill_style_str(color_fill);
		self.render_context.set_stroke_style_str(color_stroke);
		self.render_context.fill();
		self.render_context.stroke();

		self.end_dpi_aware_transform();
	}

	pub fn pixel(&mut self, position: DVec2, color: Option<&str>) {
		let size = 1.;
		let color_fill = color.unwrap_or(COLOR_OVERLAY_WHITE);

		let position = position.round() - DVec2::splat(0.5);
		let corner = position - DVec2::splat(size) / 2.;

		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		self.render_context.rect(corner.x, corner.y, size, size);
		self.render_context.set_fill_style_str(color_fill);
		self.render_context.fill();

		self.end_dpi_aware_transform();
	}

	pub fn circle(&mut self, position: DVec2, radius: f64, color_fill: Option<&str>, color_stroke: Option<&str>) {
		let color_fill = color_fill.unwrap_or(COLOR_OVERLAY_WHITE);
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);
		let position = position.round();

		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		self.render_context.arc(position.x, position.y, radius, 0., TAU).expect("Failed to draw the circle");
		self.render_context.set_fill_style_str(color_fill);
		self.render_context.set_stroke_style_str(color_stroke);
		self.render_context.fill();
		self.render_context.stroke();

		self.end_dpi_aware_transform();
	}

	pub fn draw_arc(&mut self, center: DVec2, radius: f64, start_from: f64, end_at: f64) {
		let segments = ((end_at - start_from).abs() / (std::f64::consts::PI / 4.)).ceil() as usize;
		let step = (end_at - start_from) / segments as f64;
		let half_step = step / 2.;
		let factor = 4. / 3. * half_step.sin() / (1. + half_step.cos());

		self.render_context.begin_path();

		for i in 0..segments {
			let start_angle = start_from + step * i as f64;
			let end_angle = start_angle + step;
			let start_vec = DVec2::from_angle(start_angle);
			let end_vec = DVec2::from_angle(end_angle);

			let start = center + radius * start_vec;
			let end = center + radius * end_vec;

			let handle_start = start + start_vec.perp() * radius * factor;
			let handle_end = end - end_vec.perp() * radius * factor;

			let bezier = Bezier {
				start,
				end,
				handles: bezier_rs::BezierHandles::Cubic { handle_start, handle_end },
			};

			self.bezier_command(bezier, DAffine2::IDENTITY, i == 0);
		}

		self.render_context.stroke();
	}

	pub fn draw_angle(&mut self, pivot: DVec2, radius: f64, arc_radius: f64, offset_angle: f64, angle: f64) {
		let color_line = COLOR_OVERLAY_BLUE;

		let end_point1 = pivot + radius * DVec2::from_angle(angle + offset_angle);
		let end_point2 = pivot + radius * DVec2::from_angle(offset_angle);
		self.line(pivot, end_point1, Some(color_line));
		self.line(pivot, end_point2, Some(color_line));

		self.draw_arc(pivot, arc_radius, offset_angle, (angle) % TAU + offset_angle);
	}

	pub fn draw_scale(&mut self, start: DVec2, scale: f64, radius: f64, text: &str) {
		let sign = scale.signum();
		self.line(start + DVec2::X * radius * sign, start + DVec2::X * (radius * scale), None);
		self.circle(start, radius, Some(COLOR_OVERLAY_TRANSPARENT), None);
		self.circle(start, radius * scale.abs(), Some(COLOR_OVERLAY_TRANSPARENT), None);
		self.text(
			text,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation(start + sign * DVec2::X * radius * (1. + scale.abs()) / 2.),
			2.,
			[Pivot::Middle, Pivot::End],
		)
	}

	pub fn pivot(&mut self, position: DVec2) {
		let (x, y) = (position.round() - DVec2::splat(0.5)).into();

		self.start_dpi_aware_transform();

		// Circle

		self.render_context.begin_path();
		self.render_context.arc(x, y, PIVOT_DIAMETER / 2., 0., TAU).expect("Failed to draw the circle");
		self.render_context.set_fill_style_str(COLOR_OVERLAY_YELLOW);
		self.render_context.fill();

		// Crosshair

		// Round line caps add half the stroke width to the length on each end, so we subtract that here before halving to get the radius
		let crosshair_radius = (PIVOT_CROSSHAIR_LENGTH - PIVOT_CROSSHAIR_THICKNESS) / 2.;

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

		self.render_context.set_line_cap("butt");

		self.end_dpi_aware_transform();
	}

	pub fn outline_vector(&mut self, vector_data: &VectorData, transform: DAffine2) {
		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		let mut last_point = None;
		for (_, bezier, start_id, end_id) in vector_data.segment_bezier_iter() {
			let move_to = last_point != Some(start_id);
			last_point = Some(end_id);

			self.bezier_command(bezier, transform, move_to);
		}

		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();

		self.end_dpi_aware_transform();
	}

	pub fn outline_bezier(&mut self, bezier: Bezier, transform: DAffine2) {
		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		self.bezier_command(bezier, transform, true);
		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();

		self.end_dpi_aware_transform();
	}

	fn bezier_command(&self, bezier: Bezier, transform: DAffine2, move_to: bool) {
		self.start_dpi_aware_transform();

		let Bezier { start, end, handles } = bezier.apply_transformation(|point| transform.transform_point2(point));
		if move_to {
			self.render_context.move_to(start.x, start.y);
		}

		match handles {
			bezier_rs::BezierHandles::Linear => self.render_context.line_to(end.x, end.y),
			bezier_rs::BezierHandles::Quadratic { handle } => self.render_context.quadratic_curve_to(handle.x, handle.y, end.x, end.y),
			bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => self.render_context.bezier_curve_to(handle_start.x, handle_start.y, handle_end.x, handle_end.y, end.x, end.y),
		}

		self.end_dpi_aware_transform();
	}

	pub fn outline(&mut self, subpaths: impl Iterator<Item = impl Borrow<Subpath<PointId>>>, transform: DAffine2) {
		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		for subpath in subpaths {
			let subpath = subpath.borrow();
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

		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();

		self.end_dpi_aware_transform();
	}

	pub fn get_width(&self, text: &str) -> f64 {
		self.render_context.measure_text(text).expect("Failed to measure text dimensions").width()
	}

	pub fn text(&self, text: &str, font_color: &str, background_color: Option<&str>, transform: DAffine2, padding: f64, pivot: [Pivot; 2]) {
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

		let [a, b, c, d, e, f] = (DAffine2::from_scale(DVec2::splat(self.device_pixel_ratio)) * transform * DAffine2::from_translation(DVec2::new(x, y))).to_cols_array();
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

		self.render_context.set_font("12px Source Sans Pro, Arial, sans-serif");
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
