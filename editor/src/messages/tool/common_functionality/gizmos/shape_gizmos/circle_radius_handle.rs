use crate::consts::GIZMO_HIDE_THRESHOLD;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::{overlays::utility_types::OverlayContext, utility_types::network_interface::InputConnector};
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::prelude::{FrontendMessage, Responses};
use crate::messages::tool::common_functionality::graph_modification_utils::{self};
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_circle_radius;
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RadiusHandleState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RadiusHandle {
	pub layer: Option<LayerNodeIdentifier>,
	initial_radius: f64,
	handle_state: RadiusHandleState,
	angle: f64,
	previous_mouse_position: DVec2,
}

impl RadiusHandle {
	pub fn cleanup(&mut self) {
		self.handle_state = RadiusHandleState::Inactive;
		self.layer = None;
	}

	pub fn hovered(&self) -> bool {
		self.handle_state == RadiusHandleState::Hover
	}

	pub fn is_dragging_or_snapped(&self) -> bool {
		self.handle_state == RadiusHandleState::Dragging
	}

	pub fn update_state(&mut self, state: RadiusHandleState) {
		self.handle_state = state;
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		match &self.handle_state {
			RadiusHandleState::Inactive => {
				let Some(radius) = extract_circle_radius(layer, document) else { return };
				let viewport = document.metadata().transform_to_viewport(layer);

				let angle = viewport.inverse().transform_point2(mouse_position).angle_to(DVec2::X);

				let point_position = viewport.transform_point2(get_circle_point_position(angle, radius.abs()));
				let center = viewport.transform_point2(DVec2::ZERO);

				log::info!("reaching here");
				if point_position.distance(center) < GIZMO_HIDE_THRESHOLD {
					return;
				}

				if mouse_position.distance(center) <= point_position.distance(center) {
					self.layer = Some(layer);
					self.initial_radius = radius;
					self.previous_mouse_position = mouse_position;
					self.angle = angle;
					self.update_state(RadiusHandleState::Hover);
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
				}
			}

			RadiusHandleState::Dragging | RadiusHandleState::Hover => {}
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
		match &self.handle_state {
			RadiusHandleState::Inactive => {}

			RadiusHandleState::Dragging | RadiusHandleState::Hover => {
				let Some(layer) = self.layer else { return };
				let Some(radius) = extract_circle_radius(layer, document) else { return };
				let viewport = document.metadata().transform_to_viewport(layer);

				overlay_context.dashed_circle(DVec2::ZERO, radius.abs(), None, None, Some(20.), Some(4.), Some(0.5), Some(viewport));
				// overlay_context.dashed_line(center, point_position, None, None, Some(4.), Some(4.), Some(0.5));
			}
		}
	}

	pub fn update_inner_radius(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let Some(layer) = self.layer else { return };

		let Some(node_id) = graph_modification_utils::get_circle_id(layer, &document.network_interface) else {
			return;
		};

		let Some(current_radius) = extract_circle_radius(layer, document) else { return };

		let viewport_transform = document.network_interface.document_metadata().transform_to_viewport(layer);
		let center = viewport_transform.transform_point2(DVec2::ZERO);

		let delta_vector = viewport_transform.inverse().transform_point2(input.mouse.position) - viewport_transform.inverse().transform_point2(self.previous_mouse_position);
		let radius = document.metadata().document_to_viewport.transform_point2(drag_start) - center;
		let sign = radius.dot(delta_vector).signum();

		let net_delta = delta_vector.length() * sign * self.initial_radius.signum();
		self.previous_mouse_position = input.mouse.position;

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::F64(current_radius + net_delta), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}

fn get_circle_point_position(theta: f64, radius: f64) -> DVec2 {
	DVec2::new(radius * theta.cos(), -radius * theta.sin())
}
