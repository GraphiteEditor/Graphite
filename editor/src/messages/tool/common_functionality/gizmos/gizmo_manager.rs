use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::DocumentMessageHandler;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::operations::circular_repeat::CircularRepeatGizmoHandler;
use crate::messages::tool::common_functionality::shapes::arc_shape::ArcGizmoHandler;
use crate::messages::tool::common_functionality::shapes::circle_shape::CircleGizmoHandler;
use crate::messages::tool::common_functionality::shapes::grid_shape::GridGizmoHandler;
use crate::messages::tool::common_functionality::shapes::polygon_shape::PolygonGizmoHandler;
use crate::messages::tool::common_functionality::shapes::shape_utility::{GizmoContext, ShapeGizmoHandler};
use crate::messages::tool::common_functionality::shapes::star_shape::StarGizmoHandler;
use glam::DVec2;
use std::collections::HashMap;

/// A unified enum wrapper around all available shape-specific gizmo handlers.
///
/// This abstraction allows `GizmoManager` to interact with different shape gizmos (like Star or Polygon)
/// using a common interface without needing to know the specific shape type at compile time.
///
/// Each variant stores a concrete handler (e.g., `StarGizmoHandler`, `PolygonGizmoHandler`) that implements
/// the shape-specific logic for rendering overlays, responding to input, and modifying shape parameters.
#[derive(Clone, Debug, Default)]
pub enum ShapeGizmoHandlers {
	#[default]
	None,
	Star(StarGizmoHandler),
	Polygon(PolygonGizmoHandler),
	Arc(ArcGizmoHandler),
	Circle(CircleGizmoHandler),
	CircularRepeat(CircularRepeatGizmoHandler),
	Grid(GridGizmoHandler),
}

impl ShapeGizmoHandlers {
	/// Returns the kind of shape the handler is managing, such as `"star"` or `"polygon"`.
	/// Used for grouping logic and distinguishing between handler types at runtime.
	pub fn kind(&self) -> &'static str {
		match self {
			Self::Star(_) => "star",
			Self::Polygon(_) => "polygon",
			Self::Arc(_) => "arc",
			Self::Circle(_) => "circle",
			Self::CircularRepeat(_) => "circular_repeat",
			Self::Grid(_) => "grid",
			Self::None => "none",
		}
	}

	/// Dispatches interaction state updates to the corresponding shape-specific handler.
	pub fn handle_state(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, ctx: &mut GizmoContext) {
		match self {
			Self::Star(h) => h.handle_state(layer, mouse_position, ctx),
			Self::Polygon(h) => h.handle_state(layer, mouse_position, ctx),
			Self::Arc(h) => h.handle_state(layer, mouse_position, ctx),
			Self::Circle(h) => h.handle_state(layer, mouse_position, ctx),
			Self::CircularRepeat(h) => h.handle_state(layer, mouse_position, ctx),
			Self::Grid(h) => h.handle_state(layer, mouse_position, ctx),
			Self::None => {}
		}
	}

	/// Checks if any interactive part of the gizmo is currently hovered.
	pub fn is_any_gizmo_hovered(&self) -> bool {
		match self {
			Self::Star(h) => h.is_any_gizmo_hovered(),
			Self::Polygon(h) => h.is_any_gizmo_hovered(),
			Self::Arc(h) => h.is_any_gizmo_hovered(),
			Self::Circle(h) => h.is_any_gizmo_hovered(),
			Self::CircularRepeat(h) => h.is_any_gizmo_hovered(),
			Self::Grid(h) => h.is_any_gizmo_hovered(),
			Self::None => false,
		}
	}

	/// Passes the click interaction to the appropriate gizmo handler if one is hovered.
	pub fn handle_click(&mut self) {
		match self {
			Self::Star(h) => h.handle_click(),
			Self::Polygon(h) => h.handle_click(),
			Self::Arc(h) => h.handle_click(),
			Self::Circle(h) => h.handle_click(),
			Self::CircularRepeat(h) => h.handle_click(),
			Self::Grid(h) => h.handle_click(),
			Self::None => {}
		}
	}

	/// Updates the gizmo state while the user is dragging a handle (e.g., adjusting radius).
	pub fn handle_update(&mut self, drag_start: DVec2, ctx: &mut GizmoContext) {
		match self {
			Self::Star(h) => h.handle_update(drag_start, ctx),
			Self::Polygon(h) => h.handle_update(drag_start, ctx),
			Self::Arc(h) => h.handle_update(drag_start, ctx),
			Self::Circle(h) => h.handle_update(drag_start, ctx),
			Self::CircularRepeat(h) => h.handle_update(drag_start, ctx),
			Self::Grid(h) => h.handle_update(drag_start, ctx),
			Self::None => {}
		}
	}

	/// Cleans up any state used by the gizmo handler.
	pub fn cleanup(&mut self) {
		match self {
			Self::Star(h) => h.cleanup(),
			Self::Polygon(h) => h.cleanup(),
			Self::Arc(h) => h.cleanup(),
			Self::Circle(h) => h.cleanup(),
			Self::Grid(h) => h.cleanup(),
			Self::CircularRepeat(h) => h.cleanup(),
			Self::None => {}
		}
	}

	/// Draws overlays like control points or outlines for the shape handled by this gizmo.
	pub fn overlays(&self, layer: Option<LayerNodeIdentifier>, mouse_position: DVec2, ctx: &mut GizmoContext, overlay_context: &mut OverlayContext) {
		match self {
			Self::Star(h) => h.overlays(layer, mouse_position, ctx, overlay_context),
			Self::Polygon(h) => h.overlays(layer, mouse_position, ctx, overlay_context),
			Self::Arc(h) => h.overlays(layer, mouse_position, ctx, overlay_context),
			Self::Circle(h) => h.overlays(layer, mouse_position, ctx, overlay_context),
			Self::CircularRepeat(h) => h.overlays(layer, mouse_position, ctx, overlay_context),
			Self::Grid(h) => h.overlays(layer, mouse_position, ctx, overlay_context),
			Self::None => {}
		}
	}

	/// Draws live-updating overlays during drag interactions for the shape handled by this gizmo.
	pub fn dragging_overlays(&self, mouse_position: DVec2, ctx: &mut GizmoContext, overlay_context: &mut OverlayContext) {
		match self {
			Self::Star(h) => h.dragging_overlays(mouse_position, ctx, overlay_context),
			Self::Polygon(h) => h.dragging_overlays(mouse_position, ctx, overlay_context),
			Self::Arc(h) => h.dragging_overlays(mouse_position, ctx, overlay_context),
			Self::Circle(h) => h.dragging_overlays(mouse_position, ctx, overlay_context),
			Self::CircularRepeat(h) => h.dragging_overlays(mouse_position, ctx, overlay_context),
			Self::Grid(h) => h.dragging_overlays(mouse_position, ctx, overlay_context),
			Self::None => {}
		}
	}

	pub fn gizmo_cursor_icon(&self) -> Option<MouseCursorIcon> {
		match self {
			Self::Star(h) => h.mouse_cursor_icon(),
			Self::Polygon(h) => h.mouse_cursor_icon(),
			Self::Arc(h) => h.mouse_cursor_icon(),
			Self::Circle(h) => h.mouse_cursor_icon(),
			Self::CircularRepeat(h) => h.mouse_cursor_icon(),
			Self::Grid(h) => h.mouse_cursor_icon(),
			Self::None => None,
		}
	}
}

/// Central manager that coordinates shape gizmo handlers for interactive editing on the canvas.
///
/// The `GizmoManager` is responsible for detecting which shapes are selected, activating the appropriate
/// shape-specific gizmo, and routing user interactions (hover, click, drag) to the correct handler.
/// It allows editing multiple shapes of the same type or focusing on a single active shape when a gizmo is hovered.
///
/// ## Responsibilities:
/// - Detect which selected layers support shape gizmos (e.g., stars, polygons)
/// - Activate the correct handler and manage state between frames
/// - Route click, hover, and drag events to the proper shape gizmo
/// - Render overlays and dragging visuals
#[derive(Clone, Debug, Default)]
pub struct GizmoManager {
	active_shape_handler: Option<ShapeGizmoHandlers>,
	layers_handlers: Vec<(ShapeGizmoHandlers, Vec<LayerNodeIdentifier>)>,
}

impl GizmoManager {
	/// Detects and returns a shape gizmo handler based on the layer type (e.g., star, polygon).
	///
	/// Returns `None` if the given layer does not represent a shape with a registered gizmo.
	pub fn detect_shape_handler(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Option<ShapeGizmoHandlers> {
		// Star
		if graph_modification_utils::get_star_id(layer, &document.network_interface).is_some() {
			return Some(ShapeGizmoHandlers::Star(StarGizmoHandler::default()));
		}
		// Polygon
		if graph_modification_utils::get_polygon_id(layer, &document.network_interface).is_some() {
			return Some(ShapeGizmoHandlers::Polygon(PolygonGizmoHandler::default()));
		}
		// Arc
		if graph_modification_utils::get_arc_id(layer, &document.network_interface).is_some() {
			return Some(ShapeGizmoHandlers::Arc(ArcGizmoHandler::new()));
		}
		// Circle
		if graph_modification_utils::get_circle_id(layer, &document.network_interface).is_some() {
			return Some(ShapeGizmoHandlers::Circle(CircleGizmoHandler::default()));
		}
		// Grid
		if graph_modification_utils::get_grid_id(layer, &document.network_interface).is_some() {
			return Some(ShapeGizmoHandlers::Grid(GridGizmoHandler::default()));
		}

		None
	}

	/// Detects and returns a operation gizmo handler based on the layer type (e.g., circular_repeat, repeat).
	pub fn detect_operation_gizmo_handler(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Option<ShapeGizmoHandlers> {
		// Circular Repeat
		if graph_modification_utils::get_circular_repeat(layer, &document.network_interface).is_some() {
			return Some(ShapeGizmoHandlers::CircularRepeat(CircularRepeatGizmoHandler::default()));
		}

		None
	}
	/// Returns `true` if a gizmo is currently active (hovered or being interacted with).
	pub fn hovering_over_gizmo(&self) -> bool {
		self.active_shape_handler.is_some()
	}

	/// Called every frame to check selected layers and update the active shape gizmo, if hovered.
	///
	/// Also groups all shape layers with the same kind of gizmo to support overlays for multi-shape editing.
	pub fn handle_actions(&mut self, mouse_position: DVec2, ctx: &mut GizmoContext) {
		let mut handlers_layer: Vec<(ShapeGizmoHandlers, Vec<LayerNodeIdentifier>)> = Vec::new();

		for layer in ctx.document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&ctx.document.network_interface) {
			if let Some(mut handler) = Self::detect_shape_handler(layer, ctx.document) {
				handler.handle_state(layer, mouse_position, ctx);
				let is_hovered = handler.is_any_gizmo_hovered();

				if is_hovered {
					self.layers_handlers.clear();
					self.active_shape_handler = Some(handler);
					return;
				}

				// Group same-kind handlers together
				if let Some((_, layers)) = handlers_layer.iter_mut().find(|(existing_handler, _)| existing_handler.kind() == handler.kind()) {
					layers.push(layer);
				} else {
					handlers_layer.push((handler, vec![layer]));
				}
			}
		}

		self.layers_handlers = handlers_layer;
		self.active_shape_handler = None;
	}

	pub fn handle_operation_actions(&mut self, mouse_position: DVec2, ctx: &mut GizmoContext) {
		self.active_shape_handler = None;

		let mut handlers_map: HashMap<&'static str, ShapeGizmoHandlers> = HashMap::new();
		let mut maybe_active_kind: Option<&'static str> = None;

		for layer in ctx.document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&ctx.document.network_interface) {
			if let Some(mut handler) = Self::detect_operation_gizmo_handler(layer, ctx.document) {
				let kind = handler.kind();

				// Reuse existing handler to accumulate layers
				if let Some(existing_handler) = handlers_map.remove(kind) {
					handler = existing_handler;
				}

				handler.handle_state(layer, mouse_position, ctx);

				if handler.is_any_gizmo_hovered() {
					maybe_active_kind = Some(kind);
				}

				handlers_map.insert(kind, handler);
			}
		}

		if let Some(kind) = maybe_active_kind {
			if let Some(handler) = handlers_map.remove(kind) {
				self.active_shape_handler = Some(handler);
			}
		}
	}

	/// Handles click interactions if a gizmo is active. Returns `true` if a gizmo handled the click.
	pub fn handle_click(&mut self) -> bool {
		if let Some(handle) = &mut self.active_shape_handler {
			handle.handle_click();
			return true;
		}
		false
	}

	pub fn handle_cleanup(&mut self) {
		if let Some(handle) = &mut self.active_shape_handler {
			handle.cleanup();
		}
	}

	/// Passes drag update data to the active gizmo to update shape parameters live.
	pub fn handle_update(&mut self, drag_start: DVec2, ctx: &mut GizmoContext) {
		if let Some(handle) = &mut self.active_shape_handler {
			handle.handle_update(drag_start, ctx);
		}
	}

	/// Draws overlays for the currently active shape gizmo during a drag interaction.
	pub fn dragging_overlays(&self, mouse_position: DVec2, ctx: &mut GizmoContext, overlay_context: &mut OverlayContext) {
		if let Some(handle) = &self.active_shape_handler {
			handle.dragging_overlays(mouse_position, ctx, overlay_context);
		}
	}

	/// Draws overlays for either the active gizmo (if hovered) or all grouped selected gizmos.
	///
	/// If no single gizmo is active, it renders overlays for all grouped layers with associated handlers.
	pub fn overlays(&self, mouse_position: DVec2, ctx: &mut GizmoContext, overlay_context: &mut OverlayContext) {
		if let Some(handler) = &self.active_shape_handler {
			handler.overlays(None, mouse_position, ctx, overlay_context);
			return;
		}

		for (handler, selected_layers) in &self.layers_handlers {
			for layer in selected_layers {
				handler.overlays(Some(*layer), mouse_position, ctx, overlay_context);
			}
		}
	}

	/// Returns the cursor icon to display when hovering or dragging a gizmo.
	///
	/// If a gizmo is active (hovered or being manipulated), it returns the cursor icon associated with that gizmo;
	/// otherwise, returns `None` to indicate the default crosshair cursor should be used.
	pub fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		self.active_shape_handler.as_ref().and_then(|h| h.gizmo_cursor_icon())
	}
}
