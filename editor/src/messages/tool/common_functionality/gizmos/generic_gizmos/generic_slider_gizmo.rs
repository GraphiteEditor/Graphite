//! A generic, draggable handle that edits a continuous `f64` node parameter (e.g. a radius).
//!
//! Unlike the hand-written shape gizmos in `shape_gizmos`, this gizmo is fully driven by data
//! from the [gizmo registry](crate::messages::tool::common_functionality::gizmos::gizmo_registry):
//! it knows nothing about the specific node it edits beyond the node id, the parameter index, and
//! the registry's [`GizmoInfo`]. This is what lets any node opt into a slider with zero custom code.

use crate::consts::{GIZMO_HIDE_THRESHOLD, POINT_RADIUS_HANDLE_SNAP_THRESHOLD};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::gizmos::generic_gizmos::read_f64_input;
use crate::messages::tool::common_functionality::gizmos::gizmo_registry::{GizmoContext, GizmoInfo, GizmoState, PositionHint};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use glam::DVec2;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::NodeId;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::ParameterRef;
use std::collections::VecDeque;

/// Pixel radius within which the mouse is considered to be hovering the handle.
const SLIDER_HANDLE_HOVER_THRESHOLD: f64 = 8.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GenericSliderState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

/// A draggable slider handle bound to one `f64` parameter of one node.
#[derive(Clone, Debug)]
pub struct GenericSliderGizmo {
	layer: LayerNodeIdentifier,
	node_id: NodeId,
	identifier: ProtoNodeIdentifier,
	info: GizmoInfo,
	state: GenericSliderState,
	/// The parameter value captured when the drag began, used as the clamping/anchor reference.
	initial_value: f64,
	/// Values this drag should snap to, resolved once when the gizmo is first hovered. They depend on the
	/// layer's *other* parameters, so they are captured alongside `initial_value` rather than recomputed
	/// per frame while the value being dragged is already in flux.
	snap_targets: Vec<f64>,
}

impl GenericSliderGizmo {
	pub fn new(layer: LayerNodeIdentifier, node_id: NodeId, identifier: ProtoNodeIdentifier, info: GizmoInfo) -> Self {
		Self {
			layer,
			node_id,
			identifier,
			info,
			state: GenericSliderState::Inactive,
			initial_value: 0.,
			snap_targets: Vec::new(),
		}
	}

	pub fn is_hovered(&self) -> bool {
		self.state == GenericSliderState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.state == GenericSliderState::Dragging
	}

	pub fn cleanup(&mut self) {
		self.state = GenericSliderState::Inactive;
		self.snap_targets.clear();
	}

	/// Begin a drag if currently hovered.
	pub fn handle_click(&mut self) {
		if self.state == GenericSliderState::Hover {
			self.state = GenericSliderState::Dragging;
		}
	}

	/// The registry entry's parameter, re-paired with the node it was declared for. `ParameterRef` is the
	/// runtime form of a parameter symbol: the generic gizmos choose their parameter from the registry at
	/// runtime, so they cannot name a symbol at the call site, but the identifier and index still travel together.
	fn parameter(&self) -> ParameterRef {
		ParameterRef {
			node_identifier: self.identifier.clone(),
			input_index: self.info.parameter_index,
		}
	}

	fn context<'a>(&self, document: &'a DocumentMessageHandler, mouse_position: DVec2, shape_editor: Option<&'a ShapeState>) -> GizmoContext<'a> {
		GizmoContext {
			layer: self.layer,
			document,
			parameter: self.parameter(),
			state: match self.state {
				GenericSliderState::Inactive => GizmoState::Inactive,
				GenericSliderState::Hover => GizmoState::Hover,
				GenericSliderState::Dragging => GizmoState::Dragging,
			},
			mouse_position,
			shape_editor,
		}
	}

	fn current_value(&self, document: &DocumentMessageHandler) -> Option<f64> {
		read_f64_input(self.layer, document, &self.identifier, self.info.parameter_index)
	}

	/// The handle's anchor point, in the layer's local coordinate space, derived from the current
	/// parameter value and the registry's position hint.
	fn handle_position_local(&self, value: f64) -> DVec2 {
		match self.info.position_hint {
			// A length-like parameter: place the handle that far out along the local +X axis.
			PositionHint::ParameterDerived => DVec2::new(value.abs(), 0.),
			// Generic fall-backs map the value onto the local +X axis as well; bounding-box-aware
			// hints are refined as more node types adopt the slider.
			_ => DVec2::new(value.abs(), 0.),
		}
	}

	/// Pure hover test: returns the mouse's distance to the handle when it is a hover candidate, or
	/// `None` otherwise. The manager uses this distance to resolve priority when several gizmos
	/// overlap (the closest handle wins). This performs no state mutation.
	pub fn hover_distance(&self, mouse_position: DVec2, document: &DocumentMessageHandler) -> Option<f64> {
		let value = self.current_value(document)?;

		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);
		let handle = viewport.transform_point2(self.handle_position_local(value));

		// Hide the gizmo when the shape is too small on screen to interact with reliably.
		if handle.distance(center) < GIZMO_HIDE_THRESHOLD {
			return None;
		}

		let distance = mouse_position.distance(handle);
		(distance <= SLIDER_HANDLE_HOVER_THRESHOLD).then_some(distance)
	}

	/// Transition into the hovered state (no-op if already hovered or dragging). Capturing the
	/// reference value here is necessary because `handle_click` (which starts the drag) has no
	/// access to the document.
	pub fn enter_hover(&mut self, document: &DocumentMessageHandler, mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		if self.state != GenericSliderState::Inactive {
			return;
		}
		let Some(value) = self.current_value(document) else { return };

		self.state = GenericSliderState::Hover;
		self.initial_value = value;
		self.snap_targets = match self.info.behavior.snap_targets {
			Some(targets) => targets(&self.context(document, mouse_position, None)),
			None => Vec::new(),
		};
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
	}

	/// Transition out of the hovered state. Leaves an in-progress drag untouched.
	pub fn exit_hover(&mut self, responses: &mut VecDeque<Message>) {
		if self.state == GenericSliderState::Hover {
			self.state = GenericSliderState::Inactive;
			self.snap_targets.clear();
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
		}
	}

	/// Update the parameter live while dragging. The new value is the mouse's position projected
	/// onto the local +X axis, clamped to the registry's min/max bounds.
	pub fn handle_update(&self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let viewport = document.metadata().transform_to_viewport(self.layer);
		let local_mouse = viewport.inverse().transform_point2(input.mouse.position);

		let mut value = local_mouse.x;

		// Preserve the sign of the original value for parameters (like radius) that can be negative.
		if self.initial_value.is_sign_negative() {
			value = -value;
		}

		value = self.snap(self.clamp(value));

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(self.node_id, self.parameter()),
			input: NodeInput::value(TaggedValue::F64(value), false),
		});

		// Parameters that are only meaningful in combination are written in the same batch, so the graph
		// never evaluates a half-updated shape.
		if let Some(coupled_writes) = self.info.behavior.coupled_writes {
			for (parameter, coupled_value) in coupled_writes(&self.context(document, input.mouse.position, None), value) {
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(self.node_id, parameter),
					input: NodeInput::value(coupled_value, false),
				});
			}
		}

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	/// Pull the value onto the nearest snap target within the threshold. Returns the value unchanged when
	/// nothing is in range.
	fn snap(&self, value: f64) -> f64 {
		nearest_snap_target(value, &self.snap_targets, POINT_RADIUS_HANDLE_SNAP_THRESHOLD).unwrap_or(value)
	}

	fn clamp(&self, value: f64) -> f64 {
		let mut value = value;
		if let Some(min) = self.info.min {
			value = value.max(min);
		}
		if let Some(max) = self.info.max {
			value = value.min(max);
		}
		value
	}

	/// Draw the handle dot, plus a guide line from the layer origin while hovered or dragging.
	pub fn overlays(&self, document: &DocumentMessageHandler, mouse_position: DVec2, shape_editor: Option<&ShapeState>, overlay_context: &mut OverlayContext) {
		if self.state == GenericSliderState::Inactive {
			return;
		}

		let Some(value) = self.current_value(document) else { return };
		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);
		let handle = viewport.transform_point2(self.handle_position_local(value));

		if handle.distance(center) < GIZMO_HIDE_THRESHOLD {
			return;
		}

		overlay_context.line(center, handle, None, None);
		overlay_context.manipulator_handle(handle, self.state == GenericSliderState::Dragging, None);

		if let Some(overlay) = self.info.behavior.overlay {
			overlay(&self.context(document, mouse_position, shape_editor), overlay_context);
		}
	}

	pub fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		match self.state {
			GenericSliderState::Hover | GenericSliderState::Dragging => Some(MouseCursorIcon::EWResize),
			GenericSliderState::Inactive => None,
		}
	}
}

/// The snap target closest to `value`, if any lies within `threshold`. Ties go to the earlier target, so a
/// shape can express priority through the order it returns them in.
fn nearest_snap_target(value: f64, targets: &[f64], threshold: f64) -> Option<f64> {
	targets
		.iter()
		.copied()
		.map(|target| (target, (target - value).abs()))
		.filter(|(_, distance)| *distance < threshold)
		.min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
		.map(|(target, _)| target)
}

#[cfg(test)]
mod tests {
	use super::nearest_snap_target;

	#[test]
	fn no_targets_never_snaps() {
		assert_eq!(nearest_snap_target(42., &[], 8.), None);
	}

	#[test]
	fn snaps_to_a_target_inside_the_threshold() {
		assert_eq!(nearest_snap_target(48., &[50., 100.], 8.), Some(50.));
	}

	#[test]
	fn ignores_targets_outside_the_threshold() {
		assert_eq!(nearest_snap_target(40., &[50., 100.], 8.), None);
	}

	#[test]
	fn picks_the_nearest_of_several_candidates() {
		assert_eq!(nearest_snap_target(52., &[50., 55., 100.], 8.), Some(50.));
		assert_eq!(nearest_snap_target(54., &[50., 55., 100.], 8.), Some(55.));
	}

	#[test]
	fn ties_go_to_the_earlier_target() {
		// A shape returns its most important snap targets first, so an exact tie must not reorder them.
		assert_eq!(nearest_snap_target(50., &[45., 55.], 8.), Some(45.));
	}

	#[test]
	fn handles_negative_values() {
		// Radii can be negative, and a negative radius snaps against negative targets.
		assert_eq!(nearest_snap_target(-48., &[-50., 50.], 8.), Some(-50.));
	}
}
