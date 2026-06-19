//! A generic, rotary dial that edits a discrete `u32` node parameter (e.g. a polygon's side count).
//!
//! Like [`GenericSliderGizmo`](super::generic_slider_gizmo::GenericSliderGizmo), this is fully
//! data-driven from the [gizmo registry]: it is anchored at the layer's origin and converts the
//! angle the user drags around that origin into integer steps.
//!
//! [gizmo registry]: crate::messages::tool::common_functionality::gizmos::gizmo_registry

use crate::consts::{GIZMO_HIDE_THRESHOLD, NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH};
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

/// How many degrees of rotation correspond to one integer step.
const DIAL_DEGREES_PER_STEP: f64 = 30.;
/// Viewport radius of the drawn dial indicator.
const DIAL_INDICATOR_RADIUS: f64 = NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH;

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
	pub fn handle_state(&mut self, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		if self.state == GenericDialState::Dragging {
			return;
		}

		if self.current_value(document).is_none() {
			self.state = GenericDialState::Inactive;
			return;
		}

		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		// Hide the dial when the shape is too small on screen.
		let extent = viewport.transform_point2(DVec2::new(1., 0.)).distance(center);
		if extent < f64::EPSILON {
			self.state = GenericDialState::Inactive;
			return;
		}

		if mouse_position.distance(center) <= DIAL_INDICATOR_RADIUS {
			if self.state != GenericDialState::Hover {
				self.state = GenericDialState::Hover;
				// Capture the reference value now, since `handle_click` (which starts the drag) has no
				// access to the document.
				self.initial_value = self.current_value(document).unwrap_or(0);
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
			}
		} else if self.state == GenericDialState::Hover {
			self.state = GenericDialState::Inactive;
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
		}
	}

	/// Convert the angle swept around the layer origin (relative to the drag start) into integer
	/// steps, clamped to the registry's bounds.
	pub fn handle_update(&self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		let start_vector = drag_start - center;
		let current_vector = input.mouse.position - center;
		let (Some(start_dir), Some(current_dir)) = (start_vector.try_normalize(), current_vector.try_normalize()) else {
			return;
		};

		// Signed angle (radians) swept from the drag-start direction to the current direction.
		let swept_degrees = start_dir.angle_to(current_dir).to_degrees();
		let steps = (swept_degrees / DIAL_DEGREES_PER_STEP).round() as i64;

		let min = self.info.min.map(|m| m as i64).unwrap_or(0);
		let max = self.info.max.map(|m| m as i64).unwrap_or(i64::MAX);
		let new_value = (self.initial_value as i64 + steps).clamp(min, max) as u32;

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(self.node_id, self.info.parameter_index),
			input: NodeInput::value(TaggedValue::U32(new_value), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	/// Draw the dial ring and a tick pointing toward the current mouse-derived angle.
	pub fn overlays(&self, document: &DocumentMessageHandler, mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		if self.state == GenericDialState::Inactive {
			return;
		}

		let viewport = document.metadata().transform_to_viewport(self.layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		let extent = viewport.transform_point2(DVec2::new(1., 0.)).distance(center);
		if extent < GIZMO_HIDE_THRESHOLD / DIAL_INDICATOR_RADIUS {
			// Shape is extremely small; skip to avoid a cluttered overlay.
			return;
		}

		overlay_context.circle(center, DIAL_INDICATOR_RADIUS, None, None);

		// Tick line pointing from the center toward the mouse, giving the dial a sense of direction.
		if let Some(direction) = (mouse_position - center).try_normalize() {
			overlay_context.line(center, center + direction * DIAL_INDICATOR_RADIUS, None, None);
		}
	}

	pub fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		match self.state {
			GenericDialState::Hover | GenericDialState::Dragging => Some(MouseCursorIcon::EWResize),
			GenericDialState::Inactive => None,
		}
	}
}
