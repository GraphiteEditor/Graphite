//! # Generic Gizmos
//!
//! Data-driven, reusable gizmo components that any node can opt into via the
//! [gizmo registry](super::gizmo_registry). Where a hand-written handler used to hand-code a
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

/// Read a node input as a number, whichever numeric type it is stored as.
///
/// The generic gizmos use this for hit-testing and overlays, where all that matters is how large the value
/// is. Writing still goes through the parameter's own type, so a count stays a count.
pub fn read_number_input(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, identifier: &ProtoNodeIdentifier, index: usize) -> Option<f64> {
	let inputs = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(identifier.clone()))?;
	match inputs.get(index)?.as_value()? {
		TaggedValue::F64(value) => Some(*value),
		TaggedValue::U32(value) => Some(*value as f64),
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

	/// Whether this gizmo is grabbed along a region rather than at a point.
	fn is_extended_target(&self) -> bool {
		match self {
			Self::Slider(g) => g.is_extended_target(),
			Self::Dial(g) => g.is_extended_target(),
		}
	}

	fn enter_hover(&mut self, document: &DocumentMessageHandler, mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		match self {
			Self::Slider(g) => g.enter_hover(document, mouse_position, responses),
			Self::Dial(g) => g.enter_hover(document, mouse_position, responses),
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

	fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		match self {
			Self::Slider(g) => g.handle_update(drag_start, document, input, responses),
			Self::Dial(g) => g.handle_update(drag_start, document, input, responses),
		}
	}

	fn overlays(&self, document: &DocumentMessageHandler, mouse_position: DVec2, shape_editor: Option<&ShapeState>, overlay_context: &mut OverlayContext) {
		match self {
			Self::Slider(g) => g.overlays(document, mouse_position, shape_editor, overlay_context),
			Self::Dial(g) => g.overlays(document, mouse_position, shape_editor, overlay_context),
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
				// `GenericSliderGizmo` is the general hook-driven handle: it carries the grab points, the
				// hover test, and the drag, any of which a shape can replace. `GenericDialGizmo` is the
				// narrower one, a count stepped by horizontal drag with no hooks of its own. So a
				// declaration that brings its own drag is hosted by the general one whatever it declares,
				// and only a dial relying on the default gets the dial.
				let brings_own_drag = info.behavior.drag.is_some();

				match info.gizmo_type {
					GizmoType::Dial if !brings_own_drag => gizmos.push(GenericGizmo::Dial(GenericDialGizmo::new(layer, node_id, identifier.clone(), *info))),
					// An angle runs on the same handle machinery as a length; what differs is the drag.
					GizmoType::Slider | GizmoType::Angle | GizmoType::Dial => gizmos.push(GenericGizmo::Slider(GenericSliderGizmo::new(layer, node_id, identifier.clone(), *info))),
					// Position gizmos are not yet implemented; they are skipped so a partially-migrated node
					// still gets its other controls.
					GizmoType::Position => {}
				}
			}

			if !gizmos.is_empty() {
				return Some(Self { gizmos });
			}
		}

		None
	}

	/// Index of the gizmo winning the hover among all candidates.
	///
	/// This is the priority rule for overlapping handles: a point target beats a region target, and
	/// between two of the same kind the nearer one wins. Ties go to the earlier registry declaration.
	fn closest_hover_candidate(&self, mouse_position: DVec2, document: &DocumentMessageHandler) -> Option<usize> {
		self.gizmos
			.iter()
			.enumerate()
			.filter_map(|(index, gizmo)| gizmo.hover_distance(mouse_position, document).map(|distance| (index, (gizmo.is_extended_target(), distance))))
			.min_by(|(_, a), (_, b)| rank_candidates(*a, *b))
			.map(|(index, _)| index)
	}
}

/// Orders two hover candidates, each given as `(is_extended_target, distance)`.
///
/// A point target wins outright over a region target, whatever the two distances say, because the numbers
/// are not the same measurement: a region reports distance *to the region*, which along a circle's
/// circumference is near zero everywhere, while a point reports distance to that one point. An arc's sweep
/// endpoints sit on its circumference, so comparing the two directly hands every grab to the radius. Only
/// once the kinds match does distance decide.
fn rank_candidates(a: (bool, f64), b: (bool, f64)) -> std::cmp::Ordering {
	a.0.cmp(&b.0).then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
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
				gizmo.enter_hover(document, mouse_position, responses);
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
		for gizmo in &mut self.gizmos {
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
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		for gizmo in &self.gizmos {
			gizmo.overlays(document, mouse_position, Some(shape_editor), overlay_context);
		}
	}

	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		_input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		for gizmo in &self.gizmos {
			if gizmo.is_dragging() {
				gizmo.overlays(document, mouse_position, Some(shape_editor), overlay_context);
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

#[cfg(test)]
mod tests {
	use super::rank_candidates;
	use std::cmp::Ordering;

	const POINT: bool = false;
	const REGION: bool = true;

	#[test]
	fn point_target_beats_region_target_however_far_it_is() {
		// The arc case: the radius band reads ~0 all along the circumference while the sweep endpoint
		// sitting on that circumference reads its real distance. The endpoint still has to win.
		assert_eq!(rank_candidates((POINT, 7.9), (REGION, 0.01)), Ordering::Less);
		assert_eq!(rank_candidates((REGION, 0.01), (POINT, 7.9)), Ordering::Greater);
	}

	#[test]
	fn distance_decides_between_targets_of_the_same_kind() {
		assert_eq!(rank_candidates((POINT, 2.), (POINT, 5.)), Ordering::Less);
		assert_eq!(rank_candidates((REGION, 5.), (REGION, 2.)), Ordering::Greater);
	}

	#[test]
	fn equal_candidates_tie_so_declaration_order_wins() {
		// `min_by` keeps the first of equal elements, which is the earlier registry entry.
		assert_eq!(rank_candidates((POINT, 3.), (POINT, 3.)), Ordering::Equal);
	}
}
