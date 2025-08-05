use super::utility_functions::overlay_canvas_context;
use crate::consts::{
	ARC_SWEEP_GIZMO_RADIUS, COLOR_OVERLAY_BLUE, COLOR_OVERLAY_BLUE_50, COLOR_OVERLAY_GREEN, COLOR_OVERLAY_RED, COLOR_OVERLAY_WHITE, COLOR_OVERLAY_YELLOW, COLOR_OVERLAY_YELLOW_DULL,
	COMPASS_ROSE_ARROW_SIZE, COMPASS_ROSE_HOVER_RING_DIAMETER, COMPASS_ROSE_MAIN_RING_DIAMETER, COMPASS_ROSE_RING_INNER_DIAMETER, DOWEL_PIN_RADIUS, MANIPULATOR_GROUP_MARKER_SIZE,
	PIVOT_CROSSHAIR_LENGTH, PIVOT_CROSSHAIR_THICKNESS, PIVOT_DIAMETER, SEGMENT_SELECTED_THICKNESS,
};
use crate::messages::prelude::Message;
use bezier_rs::{Bezier, Subpath};
use core::borrow::Borrow;
use core::f64::consts::{FRAC_PI_2, PI, TAU};
use glam::{DAffine2, DVec2};
use graphene_std::Color;
use graphene_std::math::quad::Quad;
use graphene_std::vector::click_target::ClickTargetType;
use graphene_std::vector::{PointId, SegmentId, Vector};
use std::collections::HashMap;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{OffscreenCanvas, OffscreenCanvasRenderingContext2d};

pub type OverlayProvider = fn(OverlayContext) -> Message;

pub fn empty_provider() -> OverlayProvider {
	|_| Message::NoOp
}

// Types of overlays used by DocumentMessage to enable/disable select group of overlays in the frontend
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum OverlaysType {
	ArtboardName,
	CompassRose,
	QuickMeasurement,
	TransformMeasurement,
	TransformCage,
	HoverOutline,
	SelectionOutline,
	Pivot,
	Origin,
	Path,
	Anchors,
	Handles,
}

#[derive(PartialEq, Copy, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(default)]
pub struct OverlaysVisibilitySettings {
	pub all: bool,
	pub artboard_name: bool,
	pub compass_rose: bool,
	pub quick_measurement: bool,
	pub transform_measurement: bool,
	pub transform_cage: bool,
	pub hover_outline: bool,
	pub selection_outline: bool,
	pub pivot: bool,
	pub origin: bool,
	pub path: bool,
	pub anchors: bool,
	pub handles: bool,
}

impl Default for OverlaysVisibilitySettings {
	fn default() -> Self {
		Self {
			all: true,
			artboard_name: true,
			compass_rose: true,
			quick_measurement: true,
			transform_measurement: true,
			transform_cage: true,
			hover_outline: true,
			selection_outline: true,
			pivot: true,
			origin: true,
			path: true,
			anchors: true,
			handles: true,
		}
	}
}

impl OverlaysVisibilitySettings {
	pub fn all(&self) -> bool {
		self.all
	}

	pub fn artboard_name(&self) -> bool {
		self.all && self.artboard_name
	}

	pub fn compass_rose(&self) -> bool {
		self.all && self.compass_rose
	}

	pub fn quick_measurement(&self) -> bool {
		self.all && self.quick_measurement
	}

	pub fn transform_measurement(&self) -> bool {
		self.all && self.transform_measurement
	}

	pub fn transform_cage(&self) -> bool {
		self.all && self.transform_cage
	}

	pub fn hover_outline(&self) -> bool {
		self.all && self.hover_outline
	}

	pub fn selection_outline(&self) -> bool {
		self.all && self.selection_outline
	}

	pub fn pivot(&self) -> bool {
		self.all && self.pivot
	}

	pub fn origin(&self) -> bool {
		self.all && self.origin
	}

	pub fn path(&self) -> bool {
		self.all && self.path
	}

	pub fn anchors(&self) -> bool {
		self.all && self.anchors
	}

	pub fn handles(&self) -> bool {
		self.all && self.anchors && self.handles
	}
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
	pub visibility_settings: OverlaysVisibilitySettings,
}
// Message hashing isn't used but is required by the message system macros
impl core::hash::Hash for OverlayContext {
	fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

impl OverlayContext {
	pub fn quad(&mut self, quad: Quad, stroke_color: Option<&str>, color_fill: Option<&str>) {
		self.dashed_polygon(&quad.0, stroke_color, color_fill, None, None, None);
	}

	pub fn draw_triangle(&mut self, base: DVec2, direction: DVec2, size: f64, color_fill: Option<&str>, color_stroke: Option<&str>) {
		let color_fill = color_fill.unwrap_or(COLOR_OVERLAY_WHITE);
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);
		let normal = direction.perp();
		let top = base + direction * size;
		let edge1 = base + normal * size / 2.;
		let edge2 = base - normal * size / 2.;

		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		self.render_context.move_to(top.x, top.y);
		self.render_context.line_to(edge1.x, edge1.y);
		self.render_context.line_to(edge2.x, edge2.y);
		self.render_context.close_path();

		self.render_context.set_fill_style_str(color_fill);
		self.render_context.set_stroke_style_str(color_stroke);
		self.render_context.fill();
		self.render_context.stroke();

		self.end_dpi_aware_transform();
	}

	pub fn dashed_quad(&mut self, quad: Quad, stroke_color: Option<&str>, color_fill: Option<&str>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
		self.dashed_polygon(&quad.0, stroke_color, color_fill, dash_width, dash_gap_width, dash_offset);
	}

	pub fn polygon(&mut self, polygon: &[DVec2], stroke_color: Option<&str>, color_fill: Option<&str>) {
		self.dashed_polygon(polygon, stroke_color, color_fill, None, None, None);
	}

	pub fn dashed_polygon(&mut self, polygon: &[DVec2], stroke_color: Option<&str>, color_fill: Option<&str>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
		if polygon.len() < 2 {
			return;
		}

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
		self.render_context.move_to(polygon.last().unwrap().x.round() - 0.5, polygon.last().unwrap().y.round() - 0.5);

		for point in polygon {
			self.render_context.line_to(point.x.round() - 0.5, point.y.round() - 0.5);
		}

		if let Some(color_fill) = color_fill {
			self.render_context.set_fill_style_str(color_fill);
			self.render_context.fill();
		}

		let stroke_color = stroke_color.unwrap_or(COLOR_OVERLAY_BLUE);
		self.render_context.set_stroke_style_str(stroke_color);
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

	pub fn line(&mut self, start: DVec2, end: DVec2, color: Option<&str>, thickness: Option<f64>) {
		self.dashed_line(start, end, color, thickness, None, None, None)
	}

	#[allow(clippy::too_many_arguments)]
	pub fn dashed_line(&mut self, start: DVec2, end: DVec2, color: Option<&str>, thickness: Option<f64>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
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
		self.render_context.set_line_width(thickness.unwrap_or(1.));
		self.render_context.set_stroke_style_str(color.unwrap_or(COLOR_OVERLAY_BLUE));
		self.render_context.stroke();
		self.render_context.set_line_width(1.);

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

	#[allow(clippy::too_many_arguments)]
	pub fn dashed_ellipse(
		&mut self,
		center: DVec2,
		radius_x: f64,
		radius_y: f64,
		rotation: Option<f64>,
		start_angle: Option<f64>,
		end_angle: Option<f64>,
		counterclockwise: Option<bool>,
		color_fill: Option<&str>,
		color_stroke: Option<&str>,
		dash_width: Option<f64>,
		dash_gap_width: Option<f64>,
		dash_offset: Option<f64>,
	) {
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);
		let center = center.round();

		self.start_dpi_aware_transform();

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
		self.render_context
			.ellipse_with_anticlockwise(
				center.x,
				center.y,
				radius_x,
				radius_y,
				rotation.unwrap_or_default(),
				start_angle.unwrap_or_default(),
				end_angle.unwrap_or(TAU),
				counterclockwise.unwrap_or_default(),
			)
			.expect("Failed to draw ellipse");
		self.render_context.set_stroke_style_str(color_stroke);

		if let Some(fill_color) = color_fill {
			self.render_context.set_fill_style_str(fill_color);
			self.render_context.fill();
		}
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

	pub fn dashed_circle(
		&mut self,
		position: DVec2,
		radius: f64,
		color_fill: Option<&str>,
		color_stroke: Option<&str>,
		dash_width: Option<f64>,
		dash_gap_width: Option<f64>,
		dash_offset: Option<f64>,
		transform: Option<DAffine2>,
	) {
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);
		let position = position.round();

		self.start_dpi_aware_transform();

		if let Some(transform) = transform {
			let [a, b, c, d, e, f] = transform.to_cols_array();
			self.render_context.transform(a, b, c, d, e, f).expect("Failed to transform circle");
		}

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
		self.render_context.arc(position.x, position.y, radius, 0., TAU).expect("Failed to draw the circle");
		self.render_context.set_stroke_style_str(color_stroke);

		if let Some(fill_color) = color_fill {
			self.render_context.set_fill_style_str(fill_color);
			self.render_context.fill();
		}
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

	pub fn circle(&mut self, position: DVec2, radius: f64, color_fill: Option<&str>, color_stroke: Option<&str>) {
		self.dashed_circle(position, radius, color_fill, color_stroke, None, None, None, None);
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

	pub fn hover_manipulator_handle(&mut self, position: DVec2, selected: bool) {
		self.start_dpi_aware_transform();

		let position = position.round() - DVec2::splat(0.5);

		self.render_context.begin_path();
		self.render_context
			.arc(position.x, position.y, (MANIPULATOR_GROUP_MARKER_SIZE + 2.) / 2., 0., TAU)
			.expect("Failed to draw the circle");

		self.render_context.set_fill_style_str(COLOR_OVERLAY_BLUE_50);
		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE_50);
		self.render_context.fill();
		self.render_context.stroke();

		self.render_context.begin_path();
		self.render_context
			.arc(position.x, position.y, MANIPULATOR_GROUP_MARKER_SIZE / 2., 0., TAU)
			.expect("Failed to draw the circle");

		let color_fill = if selected { COLOR_OVERLAY_BLUE } else { COLOR_OVERLAY_WHITE };

		self.render_context.set_fill_style_str(color_fill);
		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.fill();
		self.render_context.stroke();

		self.end_dpi_aware_transform();
	}

	pub fn hover_manipulator_anchor(&mut self, position: DVec2, selected: bool) {
		self.square(position, Some(MANIPULATOR_GROUP_MARKER_SIZE + 2.), Some(COLOR_OVERLAY_BLUE_50), Some(COLOR_OVERLAY_BLUE_50));
		let color_fill = if selected { COLOR_OVERLAY_BLUE } else { COLOR_OVERLAY_WHITE };
		self.square(position, None, Some(color_fill), Some(COLOR_OVERLAY_BLUE));
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
		self.render_context.set_line_width(1.);
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

	pub fn draw_arc_gizmo_angle(&mut self, pivot: DVec2, bold_radius: f64, arc_radius: f64, offset_angle: f64, angle: f64) {
		let end_point1 = pivot + bold_radius * DVec2::from_angle(angle + offset_angle);
		self.line(pivot, end_point1, None, None);
		self.draw_arc(pivot, arc_radius, offset_angle, (angle) % TAU + offset_angle);
	}

	pub fn draw_angle(&mut self, pivot: DVec2, radius: f64, arc_radius: f64, offset_angle: f64, angle: f64) {
		let end_point1 = pivot + radius * DVec2::from_angle(angle + offset_angle);
		let end_point2 = pivot + radius * DVec2::from_angle(offset_angle);
		self.line(pivot, end_point1, None, None);
		self.dashed_line(pivot, end_point2, None, None, Some(2.), Some(2.), Some(0.5));
		self.draw_arc(pivot, arc_radius, offset_angle, (angle) % TAU + offset_angle);
	}

	pub fn draw_scale(&mut self, start: DVec2, scale: f64, radius: f64, text: &str) {
		let sign = scale.signum();
		let mut fill_color = Color::from_rgb_str(COLOR_OVERLAY_WHITE.strip_prefix('#').unwrap()).unwrap().with_alpha(0.05).to_rgba_hex_srgb();
		fill_color.insert(0, '#');
		let fill_color = Some(fill_color.as_str());
		self.line(start + DVec2::X * radius * sign, start + DVec2::X * (radius * scale), None, None);
		self.circle(start, radius, fill_color, None);
		self.circle(start, radius * scale.abs(), fill_color, None);
		self.text(
			text,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation(start + sign * DVec2::X * radius * (1. + scale.abs()) / 2.),
			2.,
			[Pivot::Middle, Pivot::End],
		)
	}

	pub fn compass_rose(&mut self, compass_center: DVec2, angle: f64, show_compass_with_hover_ring: Option<bool>) {
		const HOVER_RING_OUTER_RADIUS: f64 = COMPASS_ROSE_HOVER_RING_DIAMETER / 2.;
		const MAIN_RING_OUTER_RADIUS: f64 = COMPASS_ROSE_MAIN_RING_DIAMETER / 2.;
		const MAIN_RING_INNER_RADIUS: f64 = COMPASS_ROSE_RING_INNER_DIAMETER / 2.;
		const ARROW_RADIUS: f64 = COMPASS_ROSE_ARROW_SIZE / 2.;
		const HOVER_RING_STROKE_WIDTH: f64 = HOVER_RING_OUTER_RADIUS - MAIN_RING_INNER_RADIUS;
		const HOVER_RING_CENTERLINE_RADIUS: f64 = (HOVER_RING_OUTER_RADIUS + MAIN_RING_INNER_RADIUS) / 2.;
		const MAIN_RING_STROKE_WIDTH: f64 = MAIN_RING_OUTER_RADIUS - MAIN_RING_INNER_RADIUS;
		const MAIN_RING_CENTERLINE_RADIUS: f64 = (MAIN_RING_OUTER_RADIUS + MAIN_RING_INNER_RADIUS) / 2.;

		let Some(show_hover_ring) = show_compass_with_hover_ring else { return };

		self.start_dpi_aware_transform();

		let center = compass_center.round() - DVec2::splat(0.5);

		// Save the old line width to restore it later
		let old_line_width = self.render_context.line_width();

		// Hover ring
		if show_hover_ring {
			let mut fill_color = Color::from_rgb_str(COLOR_OVERLAY_BLUE.strip_prefix('#').unwrap()).unwrap().with_alpha(0.5).to_rgba_hex_srgb();
			fill_color.insert(0, '#');

			self.render_context.set_line_width(HOVER_RING_STROKE_WIDTH);
			self.render_context.begin_path();
			self.render_context.arc(center.x, center.y, HOVER_RING_CENTERLINE_RADIUS, 0., TAU).expect("Failed to draw hover ring");
			self.render_context.set_stroke_style_str(&fill_color);
			self.render_context.stroke();
		}

		// Arrows
		self.render_context.set_line_width(0.01);
		for i in 0..4 {
			let direction = DVec2::from_angle(i as f64 * FRAC_PI_2 + angle);
			let color = if i % 2 == 0 { COLOR_OVERLAY_RED } else { COLOR_OVERLAY_GREEN };

			let tip = center + direction * HOVER_RING_OUTER_RADIUS;
			let base = center + direction * (MAIN_RING_INNER_RADIUS + MAIN_RING_OUTER_RADIUS) / 2.;

			let r = (ARROW_RADIUS.powi(2) + MAIN_RING_INNER_RADIUS.powi(2)).sqrt();
			let (cos, sin) = (MAIN_RING_INNER_RADIUS / r, ARROW_RADIUS / r);
			let side1 = center + r * DVec2::new(cos * direction.x - sin * direction.y, sin * direction.x + direction.y * cos);
			let side2 = center + r * DVec2::new(cos * direction.x + sin * direction.y, -sin * direction.x + direction.y * cos);

			self.render_context.begin_path();
			self.render_context.move_to(tip.x, tip.y);
			self.render_context.line_to(side1.x, side1.y);
			self.render_context.line_to(base.x, base.y);
			self.render_context.line_to(side2.x, side2.y);
			self.render_context.close_path();

			self.render_context.set_fill_style_str(color);
			self.render_context.fill();
			self.render_context.set_stroke_style_str(color);
			self.render_context.stroke();
		}

		// Main ring
		self.render_context.set_line_width(MAIN_RING_STROKE_WIDTH);
		self.render_context.begin_path();
		self.render_context.arc(center.x, center.y, MAIN_RING_CENTERLINE_RADIUS, 0., TAU).expect("Failed to draw main ring");
		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();

		// Restore the old line width
		self.render_context.set_line_width(old_line_width);
	}

	pub fn pivot(&mut self, position: DVec2, angle: f64) {
		let uv = DVec2::from_angle(angle);
		let (x, y) = (position.round() - DVec2::splat(0.5)).into();

		self.start_dpi_aware_transform();

		// Circle

		self.render_context.begin_path();
		self.render_context.arc(x, y, PIVOT_DIAMETER / 2., 0., TAU).expect("Failed to draw the circle");
		self.render_context.set_fill_style_str(COLOR_OVERLAY_YELLOW);
		self.render_context.fill();

		// Crosshair

		// Round line caps add half the stroke width to the length on each end, so we subtract that here before halving to get the radius
		const CROSSHAIR_RADIUS: f64 = (PIVOT_CROSSHAIR_LENGTH - PIVOT_CROSSHAIR_THICKNESS) / 2.;

		self.render_context.set_stroke_style_str(COLOR_OVERLAY_YELLOW);
		self.render_context.set_line_cap("round");

		self.render_context.begin_path();
		self.render_context.move_to(x + CROSSHAIR_RADIUS * uv.x, y + CROSSHAIR_RADIUS * uv.y);
		self.render_context.line_to(x - CROSSHAIR_RADIUS * uv.x, y - CROSSHAIR_RADIUS * uv.y);
		self.render_context.stroke();

		self.render_context.begin_path();
		self.render_context.move_to(x - CROSSHAIR_RADIUS * uv.y, y + CROSSHAIR_RADIUS * uv.x);
		self.render_context.line_to(x + CROSSHAIR_RADIUS * uv.y, y - CROSSHAIR_RADIUS * uv.x);
		self.render_context.stroke();

		self.render_context.set_line_cap("butt");

		self.end_dpi_aware_transform();
	}

	pub fn dowel_pin(&mut self, position: DVec2, angle: f64, color: Option<&str>) {
		let (x, y) = (position.round() - DVec2::splat(0.5)).into();
		let color = color.unwrap_or(COLOR_OVERLAY_YELLOW_DULL);

		self.start_dpi_aware_transform();

		// Draw the background circle with a white fill and blue outline
		self.render_context.begin_path();
		self.render_context.arc(x, y, DOWEL_PIN_RADIUS, 0., TAU).expect("Failed to draw the circle");
		self.render_context.set_fill_style_str(COLOR_OVERLAY_WHITE);
		self.render_context.fill();
		self.render_context.set_stroke_style_str(color);
		self.render_context.stroke();

		// Draw the two blue filled sectors
		self.render_context.begin_path();
		// Top-left sector
		self.render_context.move_to(x, y);
		self.render_context.arc(x, y, DOWEL_PIN_RADIUS, FRAC_PI_2 + angle, PI + angle).expect("Failed to draw arc");
		self.render_context.close_path();
		// Bottom-right sector
		self.render_context.move_to(x, y);
		self.render_context.arc(x, y, DOWEL_PIN_RADIUS, PI + FRAC_PI_2 + angle, TAU + angle).expect("Failed to draw arc");
		self.render_context.close_path();
		self.render_context.set_fill_style_str(color);
		self.render_context.fill();

		self.end_dpi_aware_transform();
	}

	pub fn arc_sweep_angle(&mut self, offset_angle: f64, angle: f64, end_point_position: DVec2, bold_radius: f64, pivot: DVec2, text: &str, transform: DAffine2) {
		self.manipulator_handle(end_point_position, true, None);
		self.draw_arc_gizmo_angle(pivot, bold_radius, ARC_SWEEP_GIZMO_RADIUS, offset_angle, angle.to_radians());
		self.text(&text, COLOR_OVERLAY_BLUE, None, transform, 16., [Pivot::Middle, Pivot::Middle]);
	}

	/// Used by the Pen and Path tools to outline the path of the shape.
	pub fn outline_vector(&mut self, vector: &Vector, transform: DAffine2) {
		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		let mut last_point = None;
		for (_, bezier, start_id, end_id) in vector.segment_bezier_iter() {
			let move_to = last_point != Some(start_id);
			last_point = Some(end_id);

			self.bezier_command(bezier, transform, move_to);
		}

		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();

		self.end_dpi_aware_transform();
	}

	/// Used by the Pen tool in order to show how the bezier curve would look like.
	pub fn outline_bezier(&mut self, bezier: Bezier, transform: DAffine2) {
		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		self.bezier_command(bezier, transform, true);
		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.stroke();

		self.end_dpi_aware_transform();
	}

	/// Used by the path tool segment mode in order to show the selected segments.
	pub fn outline_select_bezier(&mut self, bezier: Bezier, transform: DAffine2) {
		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		self.bezier_command(bezier, transform, true);
		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE);
		self.render_context.set_line_width(SEGMENT_SELECTED_THICKNESS);
		self.render_context.stroke();

		self.render_context.set_line_width(1.);

		self.end_dpi_aware_transform();
	}

	pub fn outline_overlay_bezier(&mut self, bezier: Bezier, transform: DAffine2) {
		self.start_dpi_aware_transform();

		self.render_context.begin_path();
		self.bezier_command(bezier, transform, true);
		self.render_context.set_stroke_style_str(COLOR_OVERLAY_BLUE_50);
		self.render_context.set_line_width(SEGMENT_SELECTED_THICKNESS);
		self.render_context.stroke();

		self.render_context.set_line_width(1.);

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

	fn push_path(&mut self, subpaths: impl Iterator<Item = impl Borrow<Subpath<PointId>>>, transform: DAffine2) {
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

		self.end_dpi_aware_transform();
	}

	/// Used by the Select tool to outline a path or a free point when selected or hovered.
	pub fn outline(&mut self, target_types: impl Iterator<Item = impl Borrow<ClickTargetType>>, transform: DAffine2, color: Option<&str>) {
		let mut subpaths: Vec<bezier_rs::Subpath<PointId>> = vec![];

		target_types.for_each(|target_type| match target_type.borrow() {
			ClickTargetType::FreePoint(point) => {
				self.manipulator_anchor(transform.transform_point2(point.position), false, None);
			}
			ClickTargetType::Subpath(subpath) => subpaths.push(subpath.clone()),
		});

		if !subpaths.is_empty() {
			self.push_path(subpaths.iter(), transform);

			let color = color.unwrap_or(COLOR_OVERLAY_BLUE);
			self.render_context.set_stroke_style_str(color);
			self.render_context.set_line_width(1.);
			self.render_context.stroke();
		}
	}

	/// Fills the area inside the path. Assumes `color` is in gamma space.
	/// Used by the Pen tool to show the path being closed.
	pub fn fill_path(&mut self, subpaths: impl Iterator<Item = impl Borrow<Subpath<PointId>>>, transform: DAffine2, color: &str) {
		self.push_path(subpaths, transform);

		self.render_context.set_fill_style_str(color);
		self.render_context.fill();
	}

	/// Fills the area inside the path with a pattern. Assumes `color` is in gamma space.
	/// Used by the fill tool to show the area to be filled.
	pub fn fill_path_pattern(&mut self, subpaths: impl Iterator<Item = impl Borrow<Subpath<PointId>>>, transform: DAffine2, color: &Color) {
		const PATTERN_WIDTH: usize = 4;
		const PATTERN_HEIGHT: usize = 4;

		let pattern_canvas = OffscreenCanvas::new(PATTERN_WIDTH as u32, PATTERN_HEIGHT as u32).unwrap();
		let pattern_context: OffscreenCanvasRenderingContext2d = pattern_canvas
			.get_context("2d")
			.ok()
			.flatten()
			.expect("Failed to get canvas context")
			.dyn_into()
			.expect("Context should be a canvas 2d context");

		// 4x4 pixels, 4 components (RGBA) per pixel
		let mut data = [0_u8; 4 * PATTERN_WIDTH * PATTERN_HEIGHT];

		// ┌▄▄┬──┬──┬──┐
		// ├▀▀┼──┼──┼──┤
		// ├──┼──┼▄▄┼──┤
		// ├──┼──┼▀▀┼──┤
		// └──┴──┴──┴──┘
		let pixels = [(0, 0), (2, 2)];
		for &(x, y) in &pixels {
			let index = (x + y * PATTERN_WIDTH) * 4;
			data[index..index + 4].copy_from_slice(&color.to_rgba8_srgb());
		}

		let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(wasm_bindgen::Clamped(&data), PATTERN_WIDTH as u32, PATTERN_HEIGHT as u32).unwrap();
		pattern_context.put_image_data(&image_data, 0., 0.).unwrap();
		let pattern = self.render_context.create_pattern_with_offscreen_canvas(&pattern_canvas, "repeat").unwrap().unwrap();

		self.push_path(subpaths, transform);

		self.render_context.set_fill_style_canvas_pattern(&pattern);
		self.render_context.fill();
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

		self.render_context.set_font(r#"12px "Source Sans Pro", Arial, sans-serif"#);
		self.render_context.set_fill_style_str(font_color);
		self.render_context.fill_text(text, 0., 0.).expect("Failed to draw the text at the calculated position");
		self.render_context.reset_transform().expect("Failed to reset the render context transform");
	}

	pub fn translation_box(&mut self, translation: DVec2, quad: Quad, typed_string: Option<String>) {
		if translation.x.abs() > 1e-3 {
			self.dashed_line(quad.top_left(), quad.top_right(), None, None, Some(2.), Some(2.), Some(0.5));

			let width = match typed_string {
				Some(ref typed_string) => typed_string,
				None => &format!("{:.2}", translation.x).trim_end_matches('0').trim_end_matches('.').to_string(),
			};
			let x_transform = DAffine2::from_translation((quad.top_left() + quad.top_right()) / 2.);
			self.text(width, COLOR_OVERLAY_BLUE, None, x_transform, 4., [Pivot::Middle, Pivot::End]);
		}

		if translation.y.abs() > 1e-3 {
			self.dashed_line(quad.top_left(), quad.bottom_left(), None, None, Some(2.), Some(2.), Some(0.5));

			let height = match typed_string {
				Some(ref typed_string) => typed_string,
				None => &format!("{:.2}", translation.y).trim_end_matches('0').trim_end_matches('.').to_string(),
			};
			let y_transform = DAffine2::from_translation((quad.top_left() + quad.bottom_left()) / 2.);
			let height_pivot = if translation.x > -1e-3 { Pivot::Start } else { Pivot::End };
			self.text(height, COLOR_OVERLAY_BLUE, None, y_transform, 3., [height_pivot, Pivot::Middle]);
		}

		if translation.x.abs() > 1e-3 && translation.y.abs() > 1e-3 {
			self.line(quad.top_right(), quad.bottom_right(), None, None);
			self.line(quad.bottom_left(), quad.bottom_right(), None, None);
		}
	}
}

pub enum Pivot {
	Start,
	Middle,
	End,
}

pub enum DrawHandles {
	All,
	SelectedAnchors(Vec<SegmentId>),
	FrontierHandles(HashMap<SegmentId, Vec<PointId>>),
	None,
}
