//! A generic dial that edits a discrete `u32` node parameter (e.g. a polygon's side count).
//!
//! Like [`GenericSliderGizmo`](super::generic_slider_gizmo::GenericSliderGizmo), this is fully
//! data-driven from the [gizmo registry]: it is anchored at the layer's origin and converts a
//! horizontal drag into integer steps (drag right to increase, left to decrease).
//!
//! [gizmo registry]: crate::messages::tool::common_functionality::gizmos::gizmo_registry

use crate::consts::NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::gizmos::generic_gizmos::read_u32_input;
use crate::messages::tool::common_functionality::gizmos::gizmo_registry::GizmoInfo;
use glam::DVec2;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::NodeId;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

/// Horizontal drag distance (viewport px) that corresponds to one integer step.
const DIAL_PIXELS_PER_STEP: f64 = 20.;
/// Viewport radius of the drawn dial indicator.
const DIAL_INDICATOR_RADIUS: f64 = NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH;
/// Viewport radius of the clickable hit area. Deliberately larger than the drawn indicator so the
/// handle is easy to grab and the press doesn't fall through to the layer-move behavior.
const DIAL_HOVER_RADIUS: f64 = NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH + 8.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GenericDialState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

/// A rotary dial bound to one `u32` parameter of one node.
#[derive(Clone, Debug)]
pub struct GenericDialGizmo {
	layer: LayerNodeIdentifier,
	node_id: NodeId,
	identifier: ProtoNodeIdentifier,
	info: GizmoInfo,
	state: GenericDialState,
	/// Parameter value captured when the drag began.
	initial_value: u32,
}

impl GenericDialGizmo {
	pub fn new(layer: LayerNodeIdentifier, node_id: NodeId, identifier: ProtoNodeIdentifier, info: GizmoInfo) -> Self {
		Self {
			layer,
			node_id,
			identifier,
			info,
			state: GenericDialState::Inactive,
			initial_value: 0,
		}
	}

	pub fn is_hovered(&self) -> bool {
		self.state == GenericDialState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.state == GenericDialState::Dragging
	}

	pub fn cleanup(&mut self) {
		self.state = GenericDialState::Inactive;
	}

	pub fn handle_click(&mut self) {
		if self.state == GenericDialState::Hover {
			self.state = GenericDialState::Dragging;
		}
	}

	fn current_value(&self, document: &DocumentMessageHandler) -> Option<u32> {
		read_u32_input(self.layer, document, &self.identifier, self.info.parameter_index)
	}

	/// Hover detection: the dial occupies a disc of `DIAL_INDICATOR_RADIUS` around the layer origin.
	/// Pure hover test: returns the mouse's distance to the dial center when it is a hover
	/// candidate, or `None` otherwise. Used by the manager to resolve overlap priority. Performs
	/// no state mutation.
	pub fn hover_distance(&self, mouse_position: DVec2, document: &DocumentMessageHandler) -> Option<f64> {
		self.current_value(document)?;

		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		// Hide the dial when the shape is degenerate on screen.
		let extent = viewport.transform_point2(DVec2::new(1., 0.)).distance(center);
		if extent < f64::EPSILON {
			return None;
		}

		let distance = mouse_position.distance(center);
		(distance <= DIAL_HOVER_RADIUS).then_some(distance)
	}

	/// Transition into the hovered state (no-op if already hovered or dragging), capturing the
	/// reference value because `handle_click` has no document access.
	pub fn enter_hover(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		if self.state != GenericDialState::Inactive {
			return;
		}
		let Some(value) = self.current_value(document) else { return };

		self.state = GenericDialState::Hover;
		self.initial_value = value;
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
	}

	/// Transition out of the hovered state. Leaves an in-progress drag untouched.
	pub fn exit_hover(&mut self, responses: &mut VecDeque<Message>) {
		if self.state == GenericDialState::Hover {
			self.state = GenericDialState::Inactive;
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
		}
	}

	/// Convert the drag into integer steps. The magnitude comes from the total drag distance (so the
	/// dial responds to motion in any direction, not just horizontal), while the horizontal direction
	/// decides the sign: drag right to increase, left to decrease. Clamped to the registry's bounds.
	pub fn handle_update(&self, drag_start: DVec2, _document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let drag = input.mouse.position - drag_start;
		let direction = (input.mouse.position.x - drag_start.x).signum();
		let steps = ((drag.length() / DIAL_PIXELS_PER_STEP).round() * direction) as i64;

		let min = self.info.min.map(|m| m as i64).unwrap_or(0);
		let max = self.info.max.map(|m| m as i64).unwrap_or(i64::MAX);
		let new_value = (self.initial_value as i64 + steps).clamp(min, max) as u32;

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(self.node_id, self.info.parameter_index),
			input: NodeInput::value(TaggedValue::U32(new_value), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	/// Draw the dial as a grabbable handle at the layer origin: an outer ring (the hit target) plus
	/// a filled center dot so it reads as draggable.
	pub fn overlays(&self, document: &DocumentMessageHandler, _mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		if self.state == GenericDialState::Inactive {
			return;
		}

		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		overlay_context.circle(center, DIAL_INDICATOR_RADIUS, None, None);
		overlay_context.manipulator_handle(center, self.state == GenericDialState::Dragging, None);
	}

	pub fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		match self.state {
			GenericDialState::Hover | GenericDialState::Dragging => Some(MouseCursorIcon::EWResize),
			GenericDialState::Inactive => None,
		}
	}
}
