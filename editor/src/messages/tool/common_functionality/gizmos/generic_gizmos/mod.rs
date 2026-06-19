//! # Generic Gizmos
//!
//! Data-driven, reusable gizmo components that any node can opt into via the
//! [gizmo registry](super::gizmo_registry). Where the legacy `shape_gizmos` each hand-code a
//! shape's interaction, the generic gizmos here are parameterized purely by `(node_id,
//! parameter_index, GizmoInfo)` and therefore work for any node that registers them.
//!
//! - [`GenericSliderGizmo`](generic_slider_gizmo::GenericSliderGizmo) edits an `f64` parameter.
//! - [`GenericDialGizmo`](generic_dial_gizmo::GenericDialGizmo) edits a `u32` parameter.
//!
//! [`GenericGizmoHandler`] ties them together behind the existing
//! [`ShapeGizmoHandler`](crate::messages::tool::common_functionality::shapes::shape_utility::ShapeGizmoHandler)
//! trait, so the [`GizmoManager`](super::gizmo_manager::GizmoManager) can drive them with no
//! knowledge of the underlying node.

pub mod generic_dial_gizmo;
pub mod generic_slider_gizmo;

use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler};
use crate::messages::tool::common_functionality::gizmos::gizmo_registry::{GizmoType, registered_gizmo_nodes};
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeGizmoHandler;
use generic_dial_gizmo::GenericDialGizmo;
use generic_slider_gizmo::GenericSliderGizmo;
use glam::DVec2;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

/// Read an `f64` node input value by node identifier and parameter index.
pub fn read_f64_input(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, identifier: &ProtoNodeIdentifier, index: usize) -> Option<f64> {
	let inputs = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(identifier.clone()))?;
	match inputs.get(index)?.as_value()? {
		TaggedValue::F64(value) => Some(*value),
		_ => None,
	}
}

/// Read a `u32` node input value by node identifier and parameter index.
pub fn read_u32_input(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, identifier: &ProtoNodeIdentifier, index: usize) -> Option<u32> {
	let inputs = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(identifier.clone()))?;
	match inputs.get(index)?.as_value()? {
		TaggedValue::U32(value) => Some(*value),
		_ => None,
	}
}

/// A single generic gizmo instance, dispatching over the supported control types.
#[derive(Clone, Debug)]
enum GenericGizmo {
	Slider(GenericSliderGizmo),
	Dial(GenericDialGizmo),
}

impl GenericGizmo {
	fn is_hovered(&self) -> bool {
		match self {
			Self::Slider(g) => g.is_hovered(),
			Self::Dial(g) => g.is_hovered(),
		}
	}

	fn is_dragging(&self) -> bool {
		match self {
			Self::Slider(g) => g.is_dragging(),
			Self::Dial(g) => g.is_dragging(),
		}
	}

	/// Distance from the mouse to this gizmo's handle when it is a hover candidate, else `None`.
	fn hover_distance(&self, mouse_position: DVec2, document: &DocumentMessageHandler) -> Option<f64> {
		match self {
			Self::Slider(g) => g.hover_distance(mouse_position, document),
			Self::Dial(g) => g.hover_distance(mouse_position, document),
		}
	}

	fn enter_hover(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		match self {
			Self::Slider(g) => g.enter_hover(document, responses),
			Self::Dial(g) => g.enter_hover(document, responses),
		}
	}

	fn exit_hover(&mut self, responses: &mut VecDeque<Message>) {
		match self {
			Self::Slider(g) => g.exit_hover(responses),
			Self::Dial(g) => g.exit_hover(responses),
		}
	}

	fn handle_click(&mut self) {
		match self {
			Self::Slider(g) => g.handle_click(),
			Self::Dial(g) => g.handle_click(),
		}
	}

	fn handle_update(&self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		match self {
			Self::Slider(g) => g.handle_update(document, input, responses),
			Self::Dial(g) => g.handle_update(drag_start, document, input, responses),
		}
	}

	fn overlays(&self, document: &DocumentMessageHandler, mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		match self {
			Self::Slider(g) => g.overlays(document, overlay_context),
			Self::Dial(g) => g.overlays(document, mouse_position, overlay_context),
		}
	}

	fn cleanup(&mut self) {
		match self {
			Self::Slider(g) => g.cleanup(),
			Self::Dial(g) => g.cleanup(),
		}
	}

	fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		match self {
			Self::Slider(g) => g.mouse_cursor_icon(),
			Self::Dial(g) => g.mouse_cursor_icon(),
		}
	}
}

/// A registry-driven gizmo manager. On construction it looks up the selected layer's generator
/// node in the [gizmo registry](super::gizmo_registry) and instantiates the appropriate generic
/// gizmos, so it can stand in for a hand-written `ShapeGizmoHandler` with no node-specific code.
///
/// It owns a `Vec<GenericGizmo>` and routes all interaction events to them, resolving priority
/// when multiple handles overlap (the handle closest to the cursor wins the hover).
#[derive(Clone, Debug, Default)]
pub struct GenericGizmoManager {
	gizmos: Vec<GenericGizmo>,
}

impl GenericGizmoManager {
	/// Query the registry for `layer`'s node and instantiate its gizmos. Returns `None` when the
	/// layer has no registry entry (so callers can fall through to legacy shape-specific handlers)
	/// or when none of its registered parameters use a currently-supported gizmo type.
	pub fn detect_gizmos(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Option<Self> {
		let node_graph_layer = NodeGraphLayer::new(layer, &document.network_interface);

		for (identifier, infos) in registered_gizmo_nodes() {
			let Some(node_id) = node_graph_layer.upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(identifier.clone())) else {
				continue;
			};

			let mut gizmos = Vec::new();
			for info in infos {
				match info.gizmo_type {
					GizmoType::Slider => gizmos.push(GenericGizmo::Slider(GenericSliderGizmo::new(layer, node_id, identifier.clone(), *info))),
					GizmoType::Dial => gizmos.push(GenericGizmo::Dial(GenericDialGizmo::new(layer, node_id, identifier.clone(), *info))),
					// Position and Angle gizmos are not yet implemented; they are skipped so a
					// partially-migrated node still gets its slider/dial controls.
					GizmoType::Position | GizmoType::Angle => {}
				}
			}

			if !gizmos.is_empty() {
				return Some(Self { gizmos });
			}
		}

		None
	}

	/// Index of the gizmo whose handle is closest to the cursor among all hover candidates.
	/// This is the priority rule for overlapping handles: nearest wins, ties broken by the
	/// registry declaration order (earlier entries win).
	fn closest_hover_candidate(&self, mouse_position: DVec2, document: &DocumentMessageHandler) -> Option<usize> {
		self.gizmos
			.iter()
			.enumerate()
			.filter_map(|(index, gizmo)| gizmo.hover_distance(mouse_position, document).map(|distance| (index, distance)))
			.min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
			.map(|(index, _)| index)
	}
}

impl ShapeGizmoHandler for GenericGizmoManager {
	fn is_any_gizmo_hovered(&self) -> bool {
		self.gizmos.iter().any(GenericGizmo::is_hovered)
	}

	fn handle_state(&mut self, _selected_shape_layers: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		// Don't recompute hover while a drag is in progress: the dragging gizmo keeps ownership.
		if self.gizmos.iter().any(GenericGizmo::is_dragging) {
			return;
		}

		// Resolve priority centrally so two overlapping handles never highlight at once: only the
		// closest candidate enters the hover state; every other gizmo leaves it.
		let winner = self.closest_hover_candidate(mouse_position, document);
		for (index, gizmo) in self.gizmos.iter_mut().enumerate() {
			if Some(index) == winner {
				gizmo.enter_hover(document, responses);
			} else {
				gizmo.exit_hover(responses);
			}
		}
	}

	fn handle_click(&mut self) {
		if let Some(gizmo) = self.gizmos.iter_mut().find(|gizmo| gizmo.is_hovered()) {
			gizmo.handle_click();
		}
	}

	fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		for gizmo in &self.gizmos {
			if gizmo.is_dragging() {
				gizmo.handle_update(drag_start, document, input, responses);
			}
		}
	}

	fn overlays(
		&self,
		document: &DocumentMessageHandler,
		_selected_shape_layers: Option<LayerNodeIdentifier>,
		_input: &InputPreprocessorMessageHandler,
		_shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		for gizmo in &self.gizmos {
			gizmo.overlays(document, mouse_position, overlay_context);
		}
	}

	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		_input: &InputPreprocessorMessageHandler,
		_shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		for gizmo in &self.gizmos {
			if gizmo.is_dragging() {
				gizmo.overlays(document, mouse_position, overlay_context);
			}
		}
	}

	fn cleanup(&mut self) {
		for gizmo in &mut self.gizmos {
			gizmo.cleanup();
		}
	}

	fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		self.gizmos.iter().find_map(GenericGizmo::mouse_cursor_icon)
	}
}
