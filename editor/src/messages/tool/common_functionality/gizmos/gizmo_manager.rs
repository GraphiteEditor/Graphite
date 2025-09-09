use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::arc_shape::ArcGizmoHandler;
use crate::messages::tool::common_functionality::shapes::circle_shape::CircleGizmoHandler;
use crate::messages::tool::common_functionality::shapes::grid_shape::GridGizmoHandler;
use crate::messages::tool::common_functionality::shapes::polygon_shape::PolygonGizmoHandler;
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeGizmoHandler;
use crate::messages::tool::common_functionality::shapes::star_shape::StarGizmoHandler;
use glam::DVec2;
use std::collections::VecDeque;

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
			Self::Grid(_) => "grid",
			Self::None => "none",
		}
	}

	/// Dispatches interaction state updates to the corresponding shape-specific handler.
	pub fn handle_state(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		match self {
			Self::Star(h) => h.handle_state(layer, mouse_position, document, responses),
			Self::Polygon(h) => h.handle_state(layer, mouse_position, document, responses),
			Self::Arc(h) => h.handle_state(layer, mouse_position, document, responses),
			Self::Circle(h) => h.handle_state(layer, mouse_position, document, responses),
			Self::Grid(h) => h.handle_state(layer, mouse_position, document, responses),
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
			Self::Grid(h) => h.handle_click(),
			Self::None => {}
		}
	}

	/// Updates the gizmo state while the user is dragging a handle (e.g., adjusting radius).
	pub fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		match self {
			Self::Star(h) => h.handle_update(drag_start, document, input, responses),
			Self::Polygon(h) => h.handle_update(drag_start, document, input, responses),
			Self::Arc(h) => h.handle_update(drag_start, document, input, responses),
			Self::Circle(h) => h.handle_update(drag_start, document, input, responses),
			Self::Grid(h) => h.handle_update(drag_start, document, input, responses),
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
			Self::None => {}
		}
	}

	/// Draws overlays like control points or outlines for the shape handled by this gizmo.
	pub fn overlays(
		&self,
		document: &DocumentMessageHandler,
		layer: Option<LayerNodeIdentifier>,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		match self {
			Self::Star(h) => h.overlays(document, layer, input, shape_editor, mouse_position, overlay_context),
			Self::Polygon(h) => h.overlays(document, layer, input, shape_editor, mouse_position, overlay_context),
			Self::Arc(h) => h.overlays(document, layer, input, shape_editor, mouse_position, overlay_context),
			Self::Circle(h) => h.overlays(document, layer, input, shape_editor, mouse_position, overlay_context),
			Self::Grid(h) => h.overlays(document, layer, input, shape_editor, mouse_position, overlay_context),
			Self::None => {}
		}
	}

	/// Draws live-updating overlays during drag interactions for the shape handled by this gizmo.
	pub fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		match self {
			Self::Star(h) => h.dragging_overlays(document, input, shape_editor, mouse_position, overlay_context),
			Self::Polygon(h) => h.dragging_overlays(document, input, shape_editor, mouse_position, overlay_context),
			Self::Arc(h) => h.dragging_overlays(document, input, shape_editor, mouse_position, overlay_context),
			Self::Circle(h) => h.dragging_overlays(document, input, shape_editor, mouse_position, overlay_context),
			Self::Grid(h) => h.dragging_overlays(document, input, shape_editor, mouse_position, overlay_context),
			Self::None => {}
		}
	}

	pub fn gizmo_cursor_icon(&self) -> Option<MouseCursorIcon> {
		match self {
			Self::Star(h) => h.mouse_cursor_icon(),
			Self::Polygon(h) => h.mouse_cursor_icon(),
			Self::Arc(h) => h.mouse_cursor_icon(),
			Self::Circle(h) => h.mouse_cursor_icon(),
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

	/// Returns `true` if a gizmo is currently active (hovered or being interacted with).
	pub fn hovering_over_gizmo(&self) -> bool {
		self.active_shape_handler.is_some()
	}

	/// Called every frame to check selected layers and update the active shape gizmo, if hovered.
	///
	/// Also groups all shape layers with the same kind of gizmo to support overlays for multi-shape editing.
	pub fn handle_actions(&mut self, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let mut handlers_layer: Vec<(ShapeGizmoHandlers, Vec<LayerNodeIdentifier>)> = Vec::new();

		for layer in document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface) {
			if let Some(mut handler) = Self::detect_shape_handler(layer, document) {
				handler.handle_state(layer, mouse_position, document, responses);
				let is_hovered = handler.is_any_gizmo_hovered();

				if is_hovered {
					self.layers_handlers.clear();
					self.active_shape_handler = Some(handler);
					return;
				}

				// Try to group this handler with others of the same type
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
	pub fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		if let Some(handle) = &mut self.active_shape_handler {
			handle.handle_update(drag_start, document, input, responses);
		}
	}

	/// Draws overlays for the currently active shape gizmo during a drag interaction.
	pub fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if let Some(handle) = &self.active_shape_handler {
			handle.dragging_overlays(document, input, shape_editor, mouse_position, overlay_context);
		}
	}

	/// Draws overlays for either the active gizmo (if hovered) or all grouped selected gizmos.
	///
	/// If no single gizmo is active, it renders overlays for all grouped layers with associated handlers.
	pub fn overlays(
		&self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if let Some(handler) = &self.active_shape_handler {
			handler.overlays(document, None, input, shape_editor, mouse_position, overlay_context);
			return;
		}

		for (handler, selected_layers) in &self.layers_handlers {
			for layer in selected_layers {
				handler.overlays(document, Some(*layer), input, shape_editor, mouse_position, overlay_context);
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
