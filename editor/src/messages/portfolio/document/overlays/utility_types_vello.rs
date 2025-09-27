use crate::consts::{
	ARC_SWEEP_GIZMO_RADIUS, COLOR_OVERLAY_BLUE, COLOR_OVERLAY_BLUE_50, COLOR_OVERLAY_GREEN, COLOR_OVERLAY_RED, COLOR_OVERLAY_WHITE, COLOR_OVERLAY_YELLOW, COLOR_OVERLAY_YELLOW_DULL,
	COMPASS_ROSE_ARROW_SIZE, COMPASS_ROSE_HOVER_RING_DIAMETER, COMPASS_ROSE_MAIN_RING_DIAMETER, COMPASS_ROSE_RING_INNER_DIAMETER, DOWEL_PIN_RADIUS, MANIPULATOR_GROUP_MARKER_SIZE,
	PIVOT_CROSSHAIR_LENGTH, PIVOT_CROSSHAIR_THICKNESS, PIVOT_DIAMETER,
};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::Message;
use core::borrow::Borrow;
use core::f64::consts::{FRAC_PI_2, PI, TAU};
use glam::{DAffine2, DVec2};
use graphene_std::Color;
use graphene_std::math::quad::Quad;
use graphene_std::subpath::{self, Subpath};
use graphene_std::table::Table;
use graphene_std::text::TextContext;
use graphene_std::text::{Font, FontCache, TextAlign, TypesettingConfig};
use graphene_std::vector::click_target::ClickTargetType;
use graphene_std::vector::misc::point_to_dvec2;
use graphene_std::vector::{PointId, SegmentId, Vector};
use kurbo::{self, BezPath, ParamCurve};
use kurbo::{Affine, PathSeg};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use vello::Scene;
use vello::peniko;

// Global lazy initialized font cache and text context
static GLOBAL_FONT_CACHE: LazyLock<FontCache> = LazyLock::new(|| {
	let mut font_cache = FontCache::default();
	// Initialize with the hardcoded font used by overlay text
	const FONT_DATA: &[u8] = include_bytes!("source-sans-pro-regular.ttf");
	let font = Font::new("Source Sans Pro".to_string(), "Regular".to_string());
	font_cache.insert(font, String::new(), FONT_DATA.to_vec());
	font_cache
});

static GLOBAL_TEXT_CONTEXT: LazyLock<Mutex<TextContext>> = LazyLock::new(|| Mutex::new(TextContext::default()));

pub type OverlayProvider = fn(OverlayContext) -> Message;

pub fn empty_provider() -> OverlayProvider {
	|_| Message::NoOp
}

/// Types of overlays used by DocumentMessage to enable/disable the selected set of viewport overlays.
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

#[derive(serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OverlayContext {
	// Serde functionality isn't used but is required by the message system macros
	#[serde(skip)]
	#[specta(skip)]
	internal: Arc<Mutex<OverlayContextInternal>>,
	pub size: DVec2,
	// The device pixel ratio is a property provided by the browser window and is the CSS pixel size divided by the physical monitor's pixel size.
	// It allows better pixel density of visualizations on high-DPI displays where the OS display scaling is not 100%, or where the browser is zoomed.
	pub device_pixel_ratio: f64,
	pub visibility_settings: OverlaysVisibilitySettings,
}

impl Clone for OverlayContext {
	fn clone(&self) -> Self {
		let internal = self.internal.lock().expect("Failed to lock internal overlay context");
		let size = internal.size;
		let device_pixel_ratio = internal.device_pixel_ratio;
		let visibility_settings = internal.visibility_settings;
		drop(internal); // Explicitly release the lock before cloning the Arc<Mutex<_>>
		Self {
			internal: self.internal.clone(),
			size,
			device_pixel_ratio,
			visibility_settings,
		}
	}
}

// Manual implementations since Scene doesn't implement PartialEq or Debug
impl PartialEq for OverlayContext {
	fn eq(&self, other: &Self) -> bool {
		self.size == other.size && self.device_pixel_ratio == other.device_pixel_ratio && self.visibility_settings == other.visibility_settings
	}
}

impl std::fmt::Debug for OverlayContext {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("OverlayContext")
			.field("scene", &"Scene { ... }")
			.field("size", &self.size)
			.field("device_pixel_ratio", &self.device_pixel_ratio)
			.field("visibility_settings", &self.visibility_settings)
			.finish()
	}
}

// Default implementation for Scene
impl Default for OverlayContext {
	fn default() -> Self {
		Self {
			internal: Mutex::new(OverlayContextInternal::default()).into(),
			size: DVec2::ZERO,
			device_pixel_ratio: 1.0,
			visibility_settings: OverlaysVisibilitySettings::default(),
		}
	}
}

// Message hashing isn't used but is required by the message system macros
impl core::hash::Hash for OverlayContext {
	fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

impl OverlayContext {
	#[allow(dead_code)]
	pub(super) fn new(size: DVec2, device_pixel_ratio: f64, visibility_settings: OverlaysVisibilitySettings) -> Self {
		Self {
			internal: Arc::new(Mutex::new(OverlayContextInternal::new(size, device_pixel_ratio, visibility_settings))),
			size,
			device_pixel_ratio,
			visibility_settings,
		}
	}

	pub fn take_scene(self) -> Scene {
		let mut internal = self.internal.lock().expect("Failed to lock internal overlay context");
		std::mem::take(&mut internal.scene)
	}

	fn internal(&'_ self) -> MutexGuard<'_, OverlayContextInternal> {
		self.internal.lock().expect("Failed to lock internal overlay context")
	}

	pub fn quad(&mut self, quad: Quad, stroke_color: Option<&str>, color_fill: Option<&str>) {
		self.internal().quad(quad, stroke_color, color_fill);
	}

	pub fn draw_triangle(&mut self, base: DVec2, direction: DVec2, size: f64, color_fill: Option<&str>, color_stroke: Option<&str>) {
		self.internal().draw_triangle(base, direction, size, color_fill, color_stroke);
	}

	pub fn dashed_quad(&mut self, quad: Quad, stroke_color: Option<&str>, color_fill: Option<&str>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
		self.internal().dashed_quad(quad, stroke_color, color_fill, dash_width, dash_gap_width, dash_offset);
	}

	pub fn polygon(&mut self, polygon: &[DVec2], stroke_color: Option<&str>, color_fill: Option<&str>) {
		self.internal().polygon(polygon, stroke_color, color_fill);
	}

	pub fn dashed_polygon(&mut self, polygon: &[DVec2], stroke_color: Option<&str>, color_fill: Option<&str>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
		self.internal().dashed_polygon(polygon, stroke_color, color_fill, dash_width, dash_gap_width, dash_offset);
	}

	pub fn line(&mut self, start: DVec2, end: DVec2, color: Option<&str>, thickness: Option<f64>) {
		self.internal().line(start, end, color, thickness);
	}

	#[allow(clippy::too_many_arguments)]
	pub fn dashed_line(&mut self, start: DVec2, end: DVec2, color: Option<&str>, thickness: Option<f64>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
		self.internal().dashed_line(start, end, color, thickness, dash_width, dash_gap_width, dash_offset);
	}

	pub fn hover_manipulator_handle(&mut self, position: DVec2, selected: bool) {
		self.internal().hover_manipulator_handle(position, selected);
	}

	pub fn hover_manipulator_anchor(&mut self, position: DVec2, selected: bool) {
		self.internal().hover_manipulator_anchor(position, selected);
	}

	pub fn manipulator_handle(&mut self, position: DVec2, selected: bool, color: Option<&str>) {
		self.internal().manipulator_handle(position, selected, color);
	}

	pub fn manipulator_anchor(&mut self, position: DVec2, selected: bool, color: Option<&str>) {
		self.internal().manipulator_anchor(position, selected, color);
	}

	pub fn square(&mut self, position: DVec2, size: Option<f64>, color_fill: Option<&str>, color_stroke: Option<&str>) {
		self.internal().square(position, size, color_fill, color_stroke);
	}

	pub fn pixel(&mut self, position: DVec2, color: Option<&str>) {
		self.internal().pixel(position, color);
	}

	pub fn circle(&mut self, position: DVec2, radius: f64, color_fill: Option<&str>, color_stroke: Option<&str>) {
		self.internal().circle(position, radius, color_fill, color_stroke);
	}

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
		self.internal().dashed_ellipse(
			center,
			radius_x,
			radius_y,
			rotation,
			start_angle,
			end_angle,
			counterclockwise,
			color_fill,
			color_stroke,
			dash_width,
			dash_gap_width,
			dash_offset,
		);
	}

	pub fn draw_arc(&mut self, center: DVec2, radius: f64, start_from: f64, end_at: f64) {
		self.internal().draw_arc(center, radius, start_from, end_at);
	}

	pub fn draw_arc_gizmo_angle(&mut self, pivot: DVec2, bold_radius: f64, arc_radius: f64, offset_angle: f64, angle: f64) {
		self.internal().draw_arc_gizmo_angle(pivot, bold_radius, arc_radius, offset_angle, angle);
	}

	pub fn draw_angle(&mut self, pivot: DVec2, radius: f64, arc_radius: f64, offset_angle: f64, angle: f64) {
		self.internal().draw_angle(pivot, radius, arc_radius, offset_angle, angle);
	}

	pub fn draw_scale(&mut self, start: DVec2, scale: f64, radius: f64, text: &str) {
		self.internal().draw_scale(start, scale, radius, text);
	}

	pub fn compass_rose(&mut self, compass_center: DVec2, angle: f64, show_compass_with_hover_ring: Option<bool>) {
		self.internal().compass_rose(compass_center, angle, show_compass_with_hover_ring);
	}

	pub fn pivot(&mut self, position: DVec2, angle: f64) {
		self.internal().pivot(position, angle);
	}

	pub fn dowel_pin(&mut self, position: DVec2, angle: f64, color: Option<&str>) {
		self.internal().dowel_pin(position, angle, color);
	}

	#[allow(clippy::too_many_arguments)]
	pub fn arc_sweep_angle(&mut self, offset_angle: f64, angle: f64, end_point_position: DVec2, bold_radius: f64, pivot: DVec2, text: &str, transform: DAffine2) {
		self.internal().arc_sweep_angle(offset_angle, angle, end_point_position, bold_radius, pivot, text, transform);
	}

	/// Used by the Pen and Path tools to outline the path of the shape.
	pub fn outline_vector(&mut self, vector: &Vector, transform: DAffine2) {
		self.internal().outline_vector(vector, transform);
	}

	/// Used by the Pen tool in order to show how the bezier curve would look like.
	pub fn outline_bezier(&mut self, bezier: PathSeg, transform: DAffine2) {
		self.internal().outline_bezier(bezier, transform);
	}

	/// Used by the path tool segment mode in order to show the selected segments.
	pub fn outline_select_bezier(&mut self, bezier: PathSeg, transform: DAffine2) {
		self.internal().outline_select_bezier(bezier, transform);
	}

	pub fn outline_overlay_bezier(&mut self, bezier: PathSeg, transform: DAffine2) {
		self.internal().outline_overlay_bezier(bezier, transform);
	}

	/// Used by the Select tool to outline a path or a free point when selected or hovered.
	pub fn outline(&mut self, target_types: impl Iterator<Item = impl Borrow<ClickTargetType>>, transform: DAffine2, color: Option<&str>) {
		self.internal().outline(target_types, transform, color);
	}

	/// Fills the area inside the path. Assumes `color` is in gamma space.
	/// Used by the Pen tool to show the path being closed.
	pub fn fill_path(&mut self, subpaths: impl Iterator<Item = impl Borrow<Subpath<PointId>>>, transform: DAffine2, color: &str) {
		self.internal().fill_path(subpaths, transform, color);
	}

	/// Fills the area inside the path with a pattern. Assumes `color` is in gamma space.
	/// Used by the fill tool to show the area to be filled.
	pub fn fill_path_pattern(&mut self, subpaths: impl Iterator<Item = impl Borrow<Subpath<PointId>>>, transform: DAffine2, color: &Color) {
		self.internal().fill_path_pattern(subpaths, transform, color);
	}

	pub fn get_width(&self, text: &str) -> f64 {
		self.internal().get_width(text)
	}

	pub fn text(&self, text: &str, font_color: &str, background_color: Option<&str>, transform: DAffine2, padding: f64, pivot: [Pivot; 2]) {
		let mut internal = self.internal();
		internal.text(text, font_color, background_color, transform, padding, pivot);
	}

	pub fn translation_box(&mut self, translation: DVec2, quad: Quad, typed_string: Option<String>) {
		self.internal().translation_box(translation, quad, typed_string);
	}
}

pub enum Pivot {
	Start,
	Middle,
	End,
}

pub enum DrawHandles {
	All,
	SelectedAnchors(HashMap<LayerNodeIdentifier, Vec<SegmentId>>),
	FrontierHandles(HashMap<LayerNodeIdentifier, HashMap<SegmentId, Vec<PointId>>>),
	None,
}

pub(super) struct OverlayContextInternal {
	scene: Scene,
	size: DVec2,
	device_pixel_ratio: f64,
	visibility_settings: OverlaysVisibilitySettings,
}

impl Default for OverlayContextInternal {
	fn default() -> Self {
		Self::new(DVec2::new(100., 100.), 1., OverlaysVisibilitySettings::default())
	}
}

impl OverlayContextInternal {
	pub(super) fn new(size: DVec2, device_pixel_ratio: f64, visibility_settings: OverlaysVisibilitySettings) -> Self {
		Self {
			scene: Scene::new(),
			size,
			device_pixel_ratio,
			visibility_settings,
		}
	}

	fn parse_color(color: &str) -> peniko::Color {
		let hex = color.trim_start_matches('#');
		let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
		let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
		let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
		let a = if hex.len() >= 8 { u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) } else { 255 };
		peniko::Color::from_rgba8(r, g, b, a)
	}

	fn quad(&mut self, quad: Quad, stroke_color: Option<&str>, color_fill: Option<&str>) {
		self.dashed_polygon(&quad.0, stroke_color, color_fill, None, None, None);
	}

	fn draw_triangle(&mut self, base: DVec2, direction: DVec2, size: f64, color_fill: Option<&str>, color_stroke: Option<&str>) {
		let color_fill = color_fill.unwrap_or(COLOR_OVERLAY_WHITE);
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);
		let normal = direction.perp();
		let top = base + direction * size;
		let edge1 = base + normal * size / 2.;
		let edge2 = base - normal * size / 2.;

		let transform = self.get_transform();

		let mut path = BezPath::new();
		path.move_to(kurbo::Point::new(top.x, top.y));
		path.line_to(kurbo::Point::new(edge1.x, edge1.y));
		path.line_to(kurbo::Point::new(edge2.x, edge2.y));
		path.close_path();

		self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(color_fill), None, &path);

		self.scene.stroke(&kurbo::Stroke::new(1.0), transform, Self::parse_color(color_stroke), None, &path);
	}

	fn dashed_quad(&mut self, quad: Quad, stroke_color: Option<&str>, color_fill: Option<&str>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
		self.dashed_polygon(&quad.0, stroke_color, color_fill, dash_width, dash_gap_width, dash_offset);
	}

	fn polygon(&mut self, polygon: &[DVec2], stroke_color: Option<&str>, color_fill: Option<&str>) {
		self.dashed_polygon(polygon, stroke_color, color_fill, None, None, None);
	}

	fn dashed_polygon(&mut self, polygon: &[DVec2], stroke_color: Option<&str>, color_fill: Option<&str>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
		if polygon.len() < 2 {
			return;
		}

		let transform = self.get_transform();

		let mut path = BezPath::new();
		if let Some(first) = polygon.last() {
			path.move_to(kurbo::Point::new(first.x.round() - 0.5, first.y.round() - 0.5));
		}

		for point in polygon {
			path.line_to(kurbo::Point::new(point.x.round() - 0.5, point.y.round() - 0.5));
		}
		path.close_path();

		if let Some(color_fill) = color_fill {
			self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(color_fill), None, &path);
		}

		let stroke_color = stroke_color.unwrap_or(COLOR_OVERLAY_BLUE);
		let mut stroke = kurbo::Stroke::new(1.0);

		if let Some(dash_width) = dash_width {
			let dash_gap = dash_gap_width.unwrap_or(1.);
			stroke = stroke.with_dashes(dash_offset.unwrap_or(0.), [dash_width, dash_gap]);
		}

		self.scene.stroke(&stroke, transform, Self::parse_color(stroke_color), None, &path);
	}

	fn line(&mut self, start: DVec2, end: DVec2, color: Option<&str>, thickness: Option<f64>) {
		self.dashed_line(start, end, color, thickness, None, None, None)
	}

	#[allow(clippy::too_many_arguments)]
	fn dashed_line(&mut self, start: DVec2, end: DVec2, color: Option<&str>, thickness: Option<f64>, dash_width: Option<f64>, dash_gap_width: Option<f64>, dash_offset: Option<f64>) {
		let transform = self.get_transform();

		let start = start.round() - DVec2::splat(0.5);
		let end = end.round() - DVec2::splat(0.5);

		let mut path = BezPath::new();
		path.move_to(kurbo::Point::new(start.x, start.y));
		path.line_to(kurbo::Point::new(end.x, end.y));

		let mut stroke = kurbo::Stroke::new(thickness.unwrap_or(1.));

		if let Some(dash_width) = dash_width {
			let dash_gap = dash_gap_width.unwrap_or(1.);
			stroke = stroke.with_dashes(dash_offset.unwrap_or(0.), [dash_width, dash_gap]);
		}

		self.scene.stroke(&stroke, transform, Self::parse_color(color.unwrap_or(COLOR_OVERLAY_BLUE)), None, &path);
	}

	fn manipulator_handle(&mut self, position: DVec2, selected: bool, color: Option<&str>) {
		let transform = self.get_transform();
		let position = position.round() - DVec2::splat(0.5);

		let circle = kurbo::Circle::new((position.x, position.y), MANIPULATOR_GROUP_MARKER_SIZE / 2.);

		let fill = if selected { COLOR_OVERLAY_BLUE } else { COLOR_OVERLAY_WHITE };
		self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(fill), None, &circle);

		self.scene
			.stroke(&kurbo::Stroke::new(1.0), transform, Self::parse_color(color.unwrap_or(COLOR_OVERLAY_BLUE)), None, &circle);
	}

	fn hover_manipulator_handle(&mut self, position: DVec2, selected: bool) {
		let transform = self.get_transform();

		let position = position.round() - DVec2::splat(0.5);

		let circle = kurbo::Circle::new((position.x, position.y), (MANIPULATOR_GROUP_MARKER_SIZE + 2.) / 2.);

		let fill = COLOR_OVERLAY_BLUE_50;
		self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(fill), None, &circle);
		self.scene.stroke(&kurbo::Stroke::new(1.0), transform, Self::parse_color(COLOR_OVERLAY_BLUE_50), None, &circle);

		let inner_circle = kurbo::Circle::new((position.x, position.y), MANIPULATOR_GROUP_MARKER_SIZE / 2.);

		let color_fill = if selected { COLOR_OVERLAY_BLUE } else { COLOR_OVERLAY_WHITE };
		self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(color_fill), None, &circle);
		self.scene.stroke(&kurbo::Stroke::new(1.0), transform, Self::parse_color(COLOR_OVERLAY_BLUE), None, &inner_circle);
	}

	fn manipulator_anchor(&mut self, position: DVec2, selected: bool, color: Option<&str>) {
		let color_stroke = color.unwrap_or(COLOR_OVERLAY_BLUE);
		let color_fill = if selected { color_stroke } else { COLOR_OVERLAY_WHITE };
		self.square(position, None, Some(color_fill), Some(color_stroke));
	}

	fn hover_manipulator_anchor(&mut self, position: DVec2, selected: bool) {
		self.square(position, Some(MANIPULATOR_GROUP_MARKER_SIZE + 2.), Some(COLOR_OVERLAY_BLUE_50), Some(COLOR_OVERLAY_BLUE_50));
		let color_fill = if selected { COLOR_OVERLAY_BLUE } else { COLOR_OVERLAY_WHITE };
		self.square(position, None, Some(color_fill), Some(COLOR_OVERLAY_BLUE));
	}

	fn get_transform(&self) -> kurbo::Affine {
		kurbo::Affine::scale(self.device_pixel_ratio)
	}

	fn square(&mut self, position: DVec2, size: Option<f64>, color_fill: Option<&str>, color_stroke: Option<&str>) {
		let size = size.unwrap_or(MANIPULATOR_GROUP_MARKER_SIZE);
		let color_fill = color_fill.unwrap_or(COLOR_OVERLAY_WHITE);
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);

		let position = position.round() - DVec2::splat(0.5);
		let corner = position - DVec2::splat(size) / 2.;

		let transform = self.get_transform();
		let rect = kurbo::Rect::new(corner.x, corner.y, corner.x + size, corner.y + size);

		self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(color_fill), None, &rect);

		self.scene.stroke(&kurbo::Stroke::new(1.0), transform, Self::parse_color(color_stroke), None, &rect);
	}

	fn pixel(&mut self, position: DVec2, color: Option<&str>) {
		let size = 1.;
		let color_fill = color.unwrap_or(COLOR_OVERLAY_WHITE);

		let position = position.round() - DVec2::splat(0.5);
		let corner = position - DVec2::splat(size) / 2.;

		let transform = self.get_transform();
		let rect = kurbo::Rect::new(corner.x, corner.y, corner.x + size, corner.y + size);

		self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(color_fill), None, &rect);
	}

	fn circle(&mut self, position: DVec2, radius: f64, color_fill: Option<&str>, color_stroke: Option<&str>) {
		let color_fill = color_fill.unwrap_or(COLOR_OVERLAY_WHITE);
		let color_stroke = color_stroke.unwrap_or(COLOR_OVERLAY_BLUE);
		let position = position.round();

		let transform = self.get_transform();
		let circle = kurbo::Circle::new((position.x, position.y), radius);

		self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(color_fill), None, &circle);

		self.scene.stroke(&kurbo::Stroke::new(1.0), transform, Self::parse_color(color_stroke), None, &circle);
	}

	fn dashed_ellipse(
		&mut self,
		_center: DVec2,
		_radius_x: f64,
		_radius_y: f64,
		_rotation: Option<f64>,
		_start_angle: Option<f64>,
		_end_angle: Option<f64>,
		_counterclockwise: Option<bool>,
		_color_fill: Option<&str>,
		_color_stroke: Option<&str>,
		_dash_width: Option<f64>,
		_dash_gap_width: Option<f64>,
		_dash_offset: Option<f64>,
	) {
	}

	fn draw_arc(&mut self, center: DVec2, radius: f64, start_from: f64, end_at: f64) {
		let segments = ((end_at - start_from).abs() / (std::f64::consts::PI / 4.)).ceil() as usize;
		let step = (end_at - start_from) / segments as f64;
		let half_step = step / 2.;
		let factor = 4. / 3. * half_step.sin() / (1. + half_step.cos());

		let mut path = BezPath::new();

		for i in 0..segments {
			let start_angle = start_from + step * i as f64;
			let end_angle = start_angle + step;
			let start_vec = DVec2::from_angle(start_angle);
			let end_vec = DVec2::from_angle(end_angle);

			let start = center + radius * start_vec;
			let end = center + radius * end_vec;

			let handle_start = start + start_vec.perp() * radius * factor;
			let handle_end = end - end_vec.perp() * radius * factor;

			if i == 0 {
				path.move_to(kurbo::Point::new(start.x, start.y));
			}

			path.curve_to(
				kurbo::Point::new(handle_start.x, handle_start.y),
				kurbo::Point::new(handle_end.x, handle_end.y),
				kurbo::Point::new(end.x, end.y),
			);
		}

		self.scene.stroke(&kurbo::Stroke::new(1.0), self.get_transform(), Self::parse_color(COLOR_OVERLAY_BLUE), None, &path);
	}

	fn draw_arc_gizmo_angle(&mut self, pivot: DVec2, bold_radius: f64, arc_radius: f64, offset_angle: f64, angle: f64) {
		let end_point1 = pivot + bold_radius * DVec2::from_angle(angle + offset_angle);
		self.line(pivot, end_point1, None, None);
		self.draw_arc(pivot, arc_radius, offset_angle, (angle) % TAU + offset_angle);
	}

	fn draw_angle(&mut self, pivot: DVec2, radius: f64, arc_radius: f64, offset_angle: f64, angle: f64) {
		let end_point1 = pivot + radius * DVec2::from_angle(angle + offset_angle);
		let end_point2 = pivot + radius * DVec2::from_angle(offset_angle);
		self.line(pivot, end_point1, None, None);
		self.dashed_line(pivot, end_point2, None, None, Some(2.), Some(2.), Some(0.5));
		self.draw_arc(pivot, arc_radius, offset_angle, (angle) % TAU + offset_angle);
	}

	fn draw_scale(&mut self, start: DVec2, scale: f64, radius: f64, text: &str) {
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

	fn compass_rose(&mut self, compass_center: DVec2, angle: f64, show_compass_with_hover_ring: Option<bool>) {
		const HOVER_RING_OUTER_RADIUS: f64 = COMPASS_ROSE_HOVER_RING_DIAMETER / 2.;
		const MAIN_RING_OUTER_RADIUS: f64 = COMPASS_ROSE_MAIN_RING_DIAMETER / 2.;
		const MAIN_RING_INNER_RADIUS: f64 = COMPASS_ROSE_RING_INNER_DIAMETER / 2.;
		const ARROW_RADIUS: f64 = COMPASS_ROSE_ARROW_SIZE / 2.;
		const HOVER_RING_STROKE_WIDTH: f64 = HOVER_RING_OUTER_RADIUS - MAIN_RING_INNER_RADIUS;
		const HOVER_RING_CENTERLINE_RADIUS: f64 = (HOVER_RING_OUTER_RADIUS + MAIN_RING_INNER_RADIUS) / 2.;
		const MAIN_RING_STROKE_WIDTH: f64 = MAIN_RING_OUTER_RADIUS - MAIN_RING_INNER_RADIUS;
		const MAIN_RING_CENTERLINE_RADIUS: f64 = (MAIN_RING_OUTER_RADIUS + MAIN_RING_INNER_RADIUS) / 2.;

		let Some(show_hover_ring) = show_compass_with_hover_ring else { return };

		let transform = self.get_transform();
		let center = compass_center.round() - DVec2::splat(0.5);

		// Hover ring
		if show_hover_ring {
			let mut fill_color = Color::from_rgb_str(COLOR_OVERLAY_BLUE.strip_prefix('#').unwrap()).unwrap().with_alpha(0.5).to_rgba_hex_srgb();
			fill_color.insert(0, '#');

			let circle = kurbo::Circle::new((center.x, center.y), HOVER_RING_CENTERLINE_RADIUS);
			self.scene
				.stroke(&kurbo::Stroke::new(HOVER_RING_STROKE_WIDTH), transform, Self::parse_color(&fill_color), None, &circle);
		}

		// Arrows
		for i in 0..4 {
			let direction = DVec2::from_angle(i as f64 * FRAC_PI_2 + angle);
			let color = if i % 2 == 0 { COLOR_OVERLAY_RED } else { COLOR_OVERLAY_GREEN };

			let tip = center + direction * HOVER_RING_OUTER_RADIUS;
			let base = center + direction * (MAIN_RING_INNER_RADIUS + MAIN_RING_OUTER_RADIUS) / 2.;

			let r = (ARROW_RADIUS.powi(2) + MAIN_RING_INNER_RADIUS.powi(2)).sqrt();
			let (cos, sin) = (MAIN_RING_INNER_RADIUS / r, ARROW_RADIUS / r);
			let side1 = center + r * DVec2::new(cos * direction.x - sin * direction.y, sin * direction.x + direction.y * cos);
			let side2 = center + r * DVec2::new(cos * direction.x + sin * direction.y, -sin * direction.x + direction.y * cos);

			let mut path = BezPath::new();
			path.move_to(kurbo::Point::new(tip.x, tip.y));
			path.line_to(kurbo::Point::new(side1.x, side1.y));
			path.line_to(kurbo::Point::new(base.x, base.y));
			path.line_to(kurbo::Point::new(side2.x, side2.y));
			path.close_path();

			let color_parsed = Self::parse_color(color);
			self.scene.fill(peniko::Fill::NonZero, transform, color_parsed, None, &path);
			self.scene.stroke(&kurbo::Stroke::new(0.01), transform, color_parsed, None, &path);
		}

		// Main ring
		let circle = kurbo::Circle::new((center.x, center.y), MAIN_RING_CENTERLINE_RADIUS);
		self.scene
			.stroke(&kurbo::Stroke::new(MAIN_RING_STROKE_WIDTH), transform, Self::parse_color(COLOR_OVERLAY_BLUE), None, &circle);
	}

	fn pivot(&mut self, position: DVec2, angle: f64) {
		let uv = DVec2::from_angle(angle);
		let (x, y) = (position.round() - DVec2::splat(0.5)).into();

		let transform = self.get_transform();

		// Circle
		let circle = kurbo::Circle::new((x, y), PIVOT_DIAMETER / 2.);
		self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(COLOR_OVERLAY_YELLOW), None, &circle);

		// Crosshair
		const CROSSHAIR_RADIUS: f64 = (PIVOT_CROSSHAIR_LENGTH - PIVOT_CROSSHAIR_THICKNESS) / 2.;

		let mut stroke = kurbo::Stroke::new(PIVOT_CROSSHAIR_THICKNESS);
		stroke = stroke.with_caps(kurbo::Cap::Round);

		// Horizontal line
		let mut path = BezPath::new();
		path.move_to(kurbo::Point::new(x + CROSSHAIR_RADIUS * uv.x, y + CROSSHAIR_RADIUS * uv.y));
		path.line_to(kurbo::Point::new(x - CROSSHAIR_RADIUS * uv.x, y - CROSSHAIR_RADIUS * uv.y));

		self.scene.stroke(&stroke, transform, Self::parse_color(COLOR_OVERLAY_YELLOW), None, &path);

		// Vertical line
		let mut path = BezPath::new();
		path.move_to(kurbo::Point::new(x - CROSSHAIR_RADIUS * uv.y, y + CROSSHAIR_RADIUS * uv.x));
		path.line_to(kurbo::Point::new(x + CROSSHAIR_RADIUS * uv.y, y - CROSSHAIR_RADIUS * uv.x));

		self.scene.stroke(&stroke, transform, Self::parse_color(COLOR_OVERLAY_YELLOW), None, &path);
	}

	fn dowel_pin(&mut self, position: DVec2, angle: f64, color: Option<&str>) {
		let (x, y) = (position.round() - DVec2::splat(0.5)).into();
		let color = color.unwrap_or(COLOR_OVERLAY_YELLOW_DULL);

		let transform = self.get_transform();

		// Draw the background circle with a white fill and colored outline
		let circle = kurbo::Circle::new((x, y), DOWEL_PIN_RADIUS);
		self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(COLOR_OVERLAY_WHITE), None, &circle);
		self.scene.stroke(&kurbo::Stroke::new(1.0), transform, Self::parse_color(color), None, &circle);

		// Draw the two filled sectors using paths
		let mut path = BezPath::new();

		// Top-left sector
		path.move_to(kurbo::Point::new(x, y));
		let end_x = x + DOWEL_PIN_RADIUS * (FRAC_PI_2 + angle).cos();
		let end_y = y + DOWEL_PIN_RADIUS * (FRAC_PI_2 + angle).sin();
		path.line_to(kurbo::Point::new(end_x, end_y));
		// Draw arc manually
		let arc = kurbo::Arc::new((x, y), (DOWEL_PIN_RADIUS, DOWEL_PIN_RADIUS), FRAC_PI_2 + angle, FRAC_PI_2, 0.0);
		arc.to_cubic_beziers(0.1, |p1, p2, p| {
			path.curve_to(p1, p2, p);
		});
		path.close_path();

		// Bottom-right sector
		path.move_to(kurbo::Point::new(x, y));
		let end_x = x + DOWEL_PIN_RADIUS * (PI + FRAC_PI_2 + angle).cos();
		let end_y = y + DOWEL_PIN_RADIUS * (PI + FRAC_PI_2 + angle).sin();
		path.line_to(kurbo::Point::new(end_x, end_y));
		// Draw arc manually
		let arc = kurbo::Arc::new((x, y), (DOWEL_PIN_RADIUS, DOWEL_PIN_RADIUS), PI + FRAC_PI_2 + angle, FRAC_PI_2, 0.0);
		arc.to_cubic_beziers(0.1, |p1, p2, p| {
			path.curve_to(p1, p2, p);
		});
		path.close_path();

		self.scene.fill(peniko::Fill::NonZero, transform, Self::parse_color(color), None, &path);
	}

	#[allow(clippy::too_many_arguments)]
	fn arc_sweep_angle(&mut self, offset_angle: f64, angle: f64, end_point_position: DVec2, bold_radius: f64, pivot: DVec2, text: &str, transform: DAffine2) {
		self.manipulator_handle(end_point_position, true, None);
		self.draw_arc_gizmo_angle(pivot, bold_radius, ARC_SWEEP_GIZMO_RADIUS, offset_angle, angle.to_radians());
		self.text(text, COLOR_OVERLAY_BLUE, None, transform, 16., [Pivot::Middle, Pivot::Middle]);
	}

	/// Used by the Pen and Path tools to outline the path of the shape.
	fn outline_vector(&mut self, vector: &Vector, transform: DAffine2) {
		let vello_transform = self.get_transform();
		let mut path = BezPath::new();

		let mut last_point = None;
		for (_, bezier, start_id, end_id) in vector.segment_iter() {
			let move_to = last_point != Some(start_id);
			last_point = Some(end_id);

			self.bezier_to_path(bezier, transform, move_to, &mut path);
		}

		self.scene.stroke(&kurbo::Stroke::new(1.0), vello_transform, Self::parse_color(COLOR_OVERLAY_BLUE), None, &path);
	}

	/// Used by the Pen tool in order to show how the bezier curve would look like.
	fn outline_bezier(&mut self, bezier: PathSeg, transform: DAffine2) {
		let vello_transform = self.get_transform();
		let mut path = BezPath::new();
		self.bezier_to_path(bezier, transform, true, &mut path);

		self.scene.stroke(&kurbo::Stroke::new(1.0), vello_transform, Self::parse_color(COLOR_OVERLAY_BLUE), None, &path);
	}

	/// Used by the path tool segment mode in order to show the selected segments.
	fn outline_select_bezier(&mut self, bezier: PathSeg, transform: DAffine2) {
		let vello_transform = self.get_transform();
		let mut path = BezPath::new();
		self.bezier_to_path(bezier, transform, true, &mut path);

		self.scene.stroke(&kurbo::Stroke::new(4.0), vello_transform, Self::parse_color(COLOR_OVERLAY_BLUE), None, &path);
	}

	fn outline_overlay_bezier(&mut self, bezier: PathSeg, transform: DAffine2) {
		let vello_transform = self.get_transform();
		let mut path = BezPath::new();
		self.bezier_to_path(bezier, transform, true, &mut path);

		self.scene.stroke(&kurbo::Stroke::new(4.0), vello_transform, Self::parse_color(COLOR_OVERLAY_BLUE_50), None, &path);
	}

	fn bezier_to_path(&self, bezier: PathSeg, transform: DAffine2, move_to: bool, path: &mut BezPath) {
		let bezier = Affine::new(transform.to_cols_array()) * bezier;
		if move_to {
			path.move_to(bezier.start());
		}
		path.push(bezier.as_path_el());
	}

	fn push_path(&mut self, subpaths: impl Iterator<Item = impl Borrow<Subpath<PointId>>>, transform: DAffine2) -> BezPath {
		let mut path = BezPath::new();

		for subpath in subpaths {
			let subpath = subpath.borrow();
			let mut curves = subpath.iter().peekable();

			let Some(first) = curves.peek() else {
				continue;
			};

			let start_point = transform.transform_point2(point_to_dvec2(first.start()));
			path.move_to(kurbo::Point::new(start_point.x, start_point.y));

			for curve in curves {
				match curve {
					PathSeg::Line(line) => {
						let a = transform.transform_point2(point_to_dvec2(line.p1));
						let a = a.round() - DVec2::splat(0.5);
						path.line_to(kurbo::Point::new(a.x, a.y));
					}
					PathSeg::Quad(quad_bez) => {
						let a = transform.transform_point2(point_to_dvec2(quad_bez.p1));
						let b = transform.transform_point2(point_to_dvec2(quad_bez.p2));
						let a = a.round() - DVec2::splat(0.5);
						let b = b.round() - DVec2::splat(0.5);
						path.quad_to(kurbo::Point::new(a.x, a.y), kurbo::Point::new(b.x, b.y));
					}
					PathSeg::Cubic(cubic_bez) => {
						let a = transform.transform_point2(point_to_dvec2(cubic_bez.p1));
						let b = transform.transform_point2(point_to_dvec2(cubic_bez.p2));
						let c = transform.transform_point2(point_to_dvec2(cubic_bez.p3));
						let a = a.round() - DVec2::splat(0.5);
						let b = b.round() - DVec2::splat(0.5);
						let c = c.round() - DVec2::splat(0.5);
						path.curve_to(kurbo::Point::new(a.x, a.y), kurbo::Point::new(b.x, b.y), kurbo::Point::new(c.x, c.y));
					}
				}
			}

			if subpath.closed() {
				path.close_path();
			}
		}

		path
	}

	/// Used by the Select tool to outline a path or a free point when selected or hovered.
	fn outline(&mut self, target_types: impl Iterator<Item = impl Borrow<ClickTargetType>>, transform: DAffine2, color: Option<&str>) {
		let mut subpaths: Vec<subpath::Subpath<PointId>> = vec![];

		for target_type in target_types {
			match target_type.borrow() {
				ClickTargetType::FreePoint(point) => {
					self.manipulator_anchor(transform.transform_point2(point.position), false, None);
				}
				ClickTargetType::Subpath(subpath) => subpaths.push(subpath.clone()),
			}
		}

		if !subpaths.is_empty() {
			let path = self.push_path(subpaths.iter(), transform);
			let color = color.unwrap_or(COLOR_OVERLAY_BLUE);

			self.scene.stroke(&kurbo::Stroke::new(1.0), self.get_transform(), Self::parse_color(color), None, &path);
		}
	}

	/// Fills the area inside the path. Assumes `color` is in gamma space.
	/// Used by the Pen tool to show the path being closed.
	fn fill_path(&mut self, subpaths: impl Iterator<Item = impl Borrow<Subpath<PointId>>>, transform: DAffine2, color: &str) {
		let path = self.push_path(subpaths, transform);

		self.scene.fill(peniko::Fill::NonZero, self.get_transform(), Self::parse_color(color), None, &path);
	}

	/// Fills the area inside the path with a pattern. Assumes `color` is in gamma space.
	/// Used by the fill tool to show the area to be filled.
	fn fill_path_pattern(&mut self, subpaths: impl Iterator<Item = impl Borrow<Subpath<PointId>>>, transform: DAffine2, color: &Color) {
		const PATTERN_WIDTH: u32 = 4;
		const PATTERN_HEIGHT: u32 = 4;

		// Create a 4x4 pixel pattern with colored pixels at (0,0) and (2,2)
		// This matches the Canvas2D checkerboard pattern
		let mut data = vec![0u8; (PATTERN_WIDTH * PATTERN_HEIGHT * 4) as usize];
		let rgba = color.to_rgba8_srgb();

		// ┌▄▄┬──┬──┬──┐
		// ├▀▀┼──┼──┼──┤
		// ├──┼──┼▄▄┼──┤
		// ├──┼──┼▀▀┼──┤
		// └──┴──┴──┴──┘
		// Set pixels at (0,0) and (2,2) to the specified color
		let pixels = [(0, 0), (2, 2)];
		for &(x, y) in &pixels {
			let index = ((y * PATTERN_WIDTH + x) * 4) as usize;
			data[index..index + 4].copy_from_slice(&rgba);
		}

		let image = peniko::Image {
			data: data.into(),
			format: peniko::ImageFormat::Rgba8,
			width: PATTERN_WIDTH,
			height: PATTERN_HEIGHT,
			x_extend: peniko::Extend::Repeat,
			y_extend: peniko::Extend::Repeat,
			alpha: 1.0,
			quality: peniko::ImageQuality::default(),
		};

		let path = self.push_path(subpaths, transform);
		let brush = peniko::Brush::Image(image);

		self.scene.fill(peniko::Fill::NonZero, self.get_transform(), &brush, None, &path);
	}

	fn get_width(&mut self, text: &str) -> f64 {
		// Use the actual text-to-path system to get precise text width
		const FONT_SIZE: f64 = 12.0;

		let typesetting = TypesettingConfig {
			font_size: FONT_SIZE,
			line_height_ratio: 1.2,
			character_spacing: 0.0,
			max_width: None,
			max_height: None,
			tilt: 0.0,
			align: TextAlign::Left,
		};

		// Load Source Sans Pro font data
		// TODO: Grab this from the node_modules folder (either with `include_bytes!` or ideally at runtime) instead of checking the font file into the repo.
		// TODO: And maybe use the WOFF2 version (if it's supported) for its smaller, compressed file size.
		let font = Font::new("Source Sans Pro".to_string(), "Regular".to_string());
		let mut text_context = GLOBAL_TEXT_CONTEXT.lock().expect("Failed to lock global text context");
		let bounds = text_context.bounding_box(text, &font, &GLOBAL_FONT_CACHE, typesetting, false);
		bounds.x
	}

	fn text(&mut self, text: &str, font_color: &str, background_color: Option<&str>, transform: DAffine2, padding: f64, pivot: [Pivot; 2]) {
		// Use the proper text-to-path system for accurate text rendering
		const FONT_SIZE: f64 = 12.0;

		// Create typesetting configuration
		let typesetting = TypesettingConfig {
			font_size: FONT_SIZE,
			line_height_ratio: 1.2,
			character_spacing: 0.0,
			max_width: None,
			max_height: None,
			tilt: 0.0,
			align: TextAlign::Left, // We'll handle alignment manually via pivot
		};

		// Load Source Sans Pro font data
		// TODO: Grab this from the node_modules folder (either with `include_bytes!` or ideally at runtime) instead of checking the font file into the repo.
		// TODO: And maybe use the WOFF2 version (if it's supported) for its smaller, compressed file size.
		let font = Font::new("Source Sans Pro".to_string(), "Regular".to_string());

		// Get text dimensions directly from layout
		let mut text_context = GLOBAL_TEXT_CONTEXT.lock().expect("Failed to lock global text context");
		let text_size = text_context.bounding_box(text, &font, &GLOBAL_FONT_CACHE, typesetting, false);
		let text_width = text_size.x;
		let text_height = text_size.y;
		// Create a rect from the size (assuming text starts at origin)
		let text_bounds = kurbo::Rect::new(0.0, 0.0, text_width, text_height);

		// Convert text to vector paths for rendering
		let text_table = text_context.to_path(text, &font, &GLOBAL_FONT_CACHE, typesetting, false);

		// Calculate position based on pivot
		let mut position = DVec2::ZERO;
		match pivot[0] {
			Pivot::Start => position.x = padding,
			Pivot::Middle => position.x = -text_width / 2.0,
			Pivot::End => position.x = -padding - text_width,
		}
		match pivot[1] {
			Pivot::Start => position.y = padding,
			Pivot::Middle => position.y -= text_height * 0.5,
			Pivot::End => position.y = -padding - text_height,
		}

		let text_transform = transform * DAffine2::from_translation(position);
		let device_transform = self.get_transform();
		let combined_transform = kurbo::Affine::new(text_transform.to_cols_array());
		let vello_transform = device_transform * combined_transform;

		// Draw background if specified
		if let Some(bg_color) = background_color {
			let bg_rect = kurbo::Rect::new(
				text_bounds.min_x() - padding,
				text_bounds.min_y() - padding,
				text_bounds.max_x() + padding,
				text_bounds.max_y() + padding,
			);
			self.scene.fill(peniko::Fill::NonZero, vello_transform, Self::parse_color(bg_color), None, &bg_rect);
		}

		// Render the actual text paths
		self.render_text_paths(&text_table, font_color, vello_transform);
	}

	// Render text paths to the vello scene using existing infrastructure
	fn render_text_paths(&mut self, text_table: &Table<Vector>, font_color: &str, base_transform: kurbo::Affine) {
		let color = Self::parse_color(font_color);

		for row in text_table.iter() {
			// Use the existing bezier_to_path infrastructure to convert Vector to BezPath
			let mut path = BezPath::new();
			let mut last_point = None;

			for (_, bezier, start_id, end_id) in row.element.segment_iter() {
				let move_to = last_point != Some(start_id);
				last_point = Some(end_id);

				self.bezier_to_path(bezier, *row.transform, move_to, &mut path);
			}

			// Render the path
			self.scene.fill(peniko::Fill::NonZero, base_transform, color, None, &path);
		}
	}

	fn translation_box(&mut self, translation: DVec2, quad: Quad, typed_string: Option<String>) {
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
