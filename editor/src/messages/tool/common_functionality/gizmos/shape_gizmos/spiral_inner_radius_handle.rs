use crate::consts::{COLOR_OVERLAY_RED, SPIRAL_INNER_RADIUS_INDEX, SPIRAL_OUTER_RADIUS_INDEX};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::{overlays::utility_types::OverlayContext, utility_types::network_interface::InputConnector};
use crate::messages::prelude::FrontendMessage;
use crate::messages::prelude::Responses;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, NodeGraphLayer};
use crate::messages::tool::common_functionality::shapes::shape_utility::{calculate_b, get_spiral_type};
use crate::messages::tool::common_functionality::shapes::shape_utility::{extract_arc_or_log_spiral_parameters, spiral_point};
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::misc::SpiralType;
use std::collections::VecDeque;
use std::f64::consts::TAU;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RadiusGizmoState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default)]
pub struct RadiusGizmo {
	pub layer: Option<LayerNodeIdentifier>,
	pub handle_state: RadiusGizmoState,
	pub spiral_type: SpiralType,
	radius_index: usize,
	previous_mouse_position: DVec2,
	initial_radius: f64,
}

impl RadiusGizmo {
	pub fn cleanup(&mut self) {
		self.layer = None;
		self.handle_state = RadiusGizmoState::Inactive;
		self.initial_radius = 0.;
	}

	pub fn hovered(&self) -> bool {
		self.handle_state == RadiusGizmoState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.handle_state == RadiusGizmoState::Dragging
	}

	pub fn update_state(&mut self, state: RadiusGizmoState) {
		self.handle_state = state;
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		match &self.handle_state {
			RadiusGizmoState::Inactive => {
				if let Some(((inner_radius, outer_radius, _, _), spiral_type)) = extract_arc_or_log_spiral_parameters(layer, document).zip(get_spiral_type(layer, document)) {
					let smaller_radius = (inner_radius.min(outer_radius)).max(5.);
					let viewport = document.metadata().transform_to_viewport(layer);
					let layer_mouse = viewport.inverse().transform_point2(mouse_position);

					if DVec2::ZERO.distance(layer_mouse) < smaller_radius.max(5.) {
						self.layer = Some(layer);
						self.initial_radius = inner_radius;
						self.spiral_type = spiral_type;
						self.previous_mouse_position = mouse_position;
						self.radius_index = if inner_radius > outer_radius { SPIRAL_OUTER_RADIUS_INDEX } else { SPIRAL_INNER_RADIUS_INDEX };
						self.update_state(RadiusGizmoState::Hover);
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
					}
				}
			}
			RadiusGizmoState::Hover | RadiusGizmoState::Dragging => {}
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, selected_spiral_layer: Option<LayerNodeIdentifier>, overlay_context: &mut OverlayContext) {
		match &self.handle_state {
			RadiusGizmoState::Hover | RadiusGizmoState::Dragging => {
				let Some(layer) = selected_spiral_layer.or(self.layer) else { return };

				let viewport = document.metadata().transform_to_viewport(layer);
				if let Some(((inner_radius, outer_radius, turns, _), spiral_type)) = extract_arc_or_log_spiral_parameters(layer, document).zip(get_spiral_type(layer, document)) {
					let b = calculate_b(inner_radius, turns, outer_radius, spiral_type);
					let (radius, endpoint) = if self.radius_index == SPIRAL_INNER_RADIUS_INDEX {
						(inner_radius, spiral_point(0., inner_radius, b, spiral_type))
					} else {
						(outer_radius, spiral_point(turns * TAU, inner_radius, b, spiral_type))
					};

					overlay_context.manipulator_handle(viewport.transform_point2(endpoint), true, Some(COLOR_OVERLAY_RED));
					overlay_context.dashed_circle(DVec2::ZERO, radius.max(5.), None, None, Some(4.), Some(4.), Some(0.5), Some(viewport));
				}
			}
			_ => {}
		}
	}

	pub fn update_inner_radius(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(layer) = self.layer else { return };

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		let node_inputs = NodeGraphLayer::new(layer, &document.network_interface)
			.find_node_inputs("Spiral")
			.expect("Failed to find inputs of Spiral");

		let viewport_transform = document.network_interface.document_metadata().transform_to_viewport(layer);

		let center = DVec2::ZERO;
		let layer_drag_start = viewport_transform.inverse().transform_point2(drag_start);

		let current_mouse_layer = viewport_transform.inverse().transform_point2(input.mouse.position);
		let previous_mouse_layer = viewport_transform.inverse().transform_point2(self.previous_mouse_position);

		let sign = (current_mouse_layer - previous_mouse_layer).dot(layer_drag_start).signum();

		let delta = current_mouse_layer.distance(previous_mouse_layer) * sign;

		let net_radius = current_mouse_layer.distance(DVec2::ZERO);

		let Some(&TaggedValue::F64(radius)) = node_inputs.get(self.radius_index).expect("Failed to get radius of Spiral").as_value() else {
			return;
		};

		let net_radius = match self.spiral_type {
			SpiralType::Archimedean => (radius + delta).max(0.),
			SpiralType::Logarithmic => (radius + delta).max(0.001),
		};

		self.previous_mouse_position = input.mouse.position;

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, self.radius_index),
			input: NodeInput::value(TaggedValue::F64(net_radius), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}
