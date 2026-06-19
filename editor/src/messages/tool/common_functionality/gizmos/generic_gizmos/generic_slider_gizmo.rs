//! A generic, draggable handle that edits a continuous `f64` node parameter (e.g. a radius).
//!
//! Unlike the hand-written shape gizmos in `shape_gizmos`, this gizmo is fully driven by data
//! from the [gizmo registry](crate::messages::tool::common_functionality::gizmos::gizmo_registry):
//! it knows nothing about the specific node it edits beyond the node id, the parameter index, and
//! the registry's [`GizmoInfo`]. This is what lets any node opt into a slider with zero custom code.

use crate::consts::GIZMO_HIDE_THRESHOLD;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::gizmos::generic_gizmos::read_f64_input;
use crate::messages::tool::common_functionality::gizmos::gizmo_registry::{GizmoInfo, PositionHint};
use glam::DVec2;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::NodeId;
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
	}

	/// Begin a drag if currently hovered.
	pub fn handle_click(&mut self) {
		if self.state == GenericSliderState::Hover {
			self.state = GenericSliderState::Dragging;
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

	/// Detect hover by measuring the mouse's distance to the handle in viewport space.
	pub fn handle_state(&mut self, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		// Never override an in-progress drag.
		if self.state == GenericSliderState::Dragging {
			return;
		}

		let Some(value) = self.current_value(document) else {
			self.state = GenericSliderState::Inactive;
			return;
		};

		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);
		let handle = viewport.transform_point2(self.handle_position_local(value));

		// Hide the gizmo when the shape is too small on screen to interact with reliably.
		if handle.distance(center) < GIZMO_HIDE_THRESHOLD {
			self.state = GenericSliderState::Inactive;
			return;
		}

		if mouse_position.distance(handle) <= SLIDER_HANDLE_HOVER_THRESHOLD {
			if self.state != GenericSliderState::Hover {
				self.state = GenericSliderState::Hover;
				// Capture the reference value now, since `handle_click` (which starts the drag) has no
				// access to the document.
				self.initial_value = value;
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
			}
		} else if self.state == GenericSliderState::Hover {
			self.state = GenericSliderState::Inactive;
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

		value = self.clamp(value);

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(self.node_id, self.info.parameter_index),
			input: NodeInput::value(TaggedValue::F64(value), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
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
	pub fn overlays(&self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
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
	}

	pub fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		match self.state {
			GenericSliderState::Hover | GenericSliderState::Dragging => Some(MouseCursorIcon::EWResize),
			GenericSliderState::Inactive => None,
		}
	}
}
