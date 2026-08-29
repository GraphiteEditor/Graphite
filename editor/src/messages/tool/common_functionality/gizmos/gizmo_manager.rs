use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler};
use crate::messages::tool::common_functionality::gizmos::generic_gizmos::GenericGizmoManager;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeGizmoHandler;
use glam::DVec2;
use std::collections::VecDeque;

/// Coordinates the gizmo handlers of every selected layer.
///
/// While a gizmo is hovered it becomes the active handler and receives every click and drag. Otherwise the
/// manager keeps one handler per selected layer, so all of them draw their overlays.
#[derive(Clone, Debug, Default)]
pub struct GizmoManager {
	active_shape_handler: Option<GenericGizmoManager>,
	layer_handlers: Vec<GenericGizmoManager>,
}

impl GizmoManager {
	/// Returns `true` if a gizmo is currently active (hovered or being interacted with).
	pub fn hovering_over_gizmo(&self) -> bool {
		self.active_shape_handler.is_some()
	}

	/// Called every frame to refresh the handler of each selected layer and pick out the hovered one.
	pub fn handle_actions(&mut self, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let mut layer_handlers = Vec::new();

		for layer in document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface) {
			let Some(mut handler) = GenericGizmoManager::detect_gizmos(layer, document) else { continue };
			handler.handle_state(layer, mouse_position, document, responses);

			// A hovered gizmo takes the whole interaction, so the other layers stop drawing overlays entirely.
			if handler.is_any_gizmo_hovered() {
				self.layer_handlers.clear();
				self.active_shape_handler = Some(handler);
				return;
			}

			layer_handlers.push(handler);
		}

		self.layer_handlers = layer_handlers;
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

	/// Draws overlays for the hovered gizmo, or for every selected layer when none is hovered.
	pub fn overlays(
		&self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		let handlers = match &self.active_shape_handler {
			Some(handler) => std::slice::from_ref(handler),
			None => self.layer_handlers.as_slice(),
		};

		for handler in handlers {
			handler.overlays(document, input, shape_editor, mouse_position, overlay_context);
		}
	}

	/// The cursor icon of the active gizmo, or `None` for the tool's default crosshair.
	pub fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		self.active_shape_handler.as_ref().and_then(|h| h.mouse_cursor_icon())
	}
}
