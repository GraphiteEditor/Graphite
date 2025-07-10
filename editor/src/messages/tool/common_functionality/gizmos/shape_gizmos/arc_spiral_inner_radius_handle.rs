use crate::consts::ARCHIMEDEAN_INNER_RADIUS_INDEX;
use crate::consts::COLOR_OVERLAY_RED;
use crate::consts::GIZMO_HIDE_THRESHOLD;
use crate::consts::LOGARITHMIC_START_RADIUS_INDEX;
use crate::consts::NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH;
use crate::consts::SPIRAL_INNER_RADIUS_GIZMO_THRESHOLD;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::{overlays::utility_types::OverlayContext, utility_types::network_interface::InputConnector};
use crate::messages::prelude::FrontendMessage;
use crate::messages::prelude::Responses;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, NodeGraphLayer};
use crate::messages::tool::common_functionality::shapes::shape_utility::archimedean_spiral_point;
use crate::messages::tool::common_functionality::shapes::shape_utility::calculate_b;
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_arc_spiral_parameters;
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_log_spiral_parameters;
use crate::messages::tool::common_functionality::shapes::shape_utility::get_arc_spiral_end_point;
use crate::messages::tool::common_functionality::shapes::shape_utility::get_log_spiral_end_point;
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::num_traits::sign;
use graphene_std::vector::misc::SpiralType;
use graphene_std::vector::misc::dvec2_to_point;
use kurbo::BezPath;
use kurbo::Circle;
use kurbo::Line;
use kurbo::ParamCurveNearest;
use kurbo::Point;
use std::collections::VecDeque;

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
	initial_radius: f64,
	previous_mouse: DVec2,
}

impl RadiusGizmo {
	pub fn cleanup(&mut self) {
		self.layer = None;
		self.handle_state = RadiusGizmoState::Inactive;
		self.initial_radius = 0.;
		self.previous_mouse = DVec2::ZERO;
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
				// Archimedean
				if let Some((a, outer_radius, turns)) = extract_arc_spiral_parameters(layer, document) {
					let viewport = document.metadata().transform_to_viewport(layer);
					let layer_mouse = viewport.inverse().transform_point2(mouse_position);

					let center = viewport.transform_point2(DVec2::ZERO);

					if (DVec2::ZERO.distance(layer_mouse) - a).abs() < 5. {
						self.layer = Some(layer);
						self.initial_radius = a;
						self.spiral_type = SpiralType::Archimedean;
						self.update_state(RadiusGizmoState::Hover);
						self.previous_mouse = mouse_position;
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
					}
				}

				// Logarithmic
				if let Some((a, outer_radius, turns)) = extract_log_spiral_parameters(layer, document) {
					let viewport = document.metadata().transform_to_viewport(layer);
					let layer_mouse = viewport.inverse().transform_point2(mouse_position);

					if (DVec2::ZERO.distance(layer_mouse) - a).abs() < 5. {
						self.layer = Some(layer);
						self.initial_radius = a;
						self.spiral_type = SpiralType::Logarithmic;
						self.update_state(RadiusGizmoState::Hover);
						self.previous_mouse = mouse_position;
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
					}
				}
			}
			RadiusGizmoState::Hover | RadiusGizmoState::Dragging => {}
		}
	}

	pub fn overlays(
		&self,
		document: &DocumentMessageHandler,
		selected_spiral_layer: Option<LayerNodeIdentifier>,
		input: &InputPreprocessorMessageHandler,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		match &self.handle_state {
			_ => {
				let Some(layer) = selected_spiral_layer.or(self.layer) else { return };

				let viewport = document.metadata().transform_to_viewport(layer);
				if let Some((radius, _, _)) = extract_arc_spiral_parameters(layer, document).or(extract_log_spiral_parameters(layer, document)) {
					overlay_context.dashed_circle(DVec2::ZERO, radius.max(5.), None, None, Some(4.), Some(4.), Some(0.5), Some(viewport));
				}
			}
		}
	}

	pub fn update_inner_radius(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let Some(layer) = self.layer else { return };

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface).or(graph_modification_utils::get_polygon_id(layer, &document.network_interface)) else {
			return;
		};

		let viewport_transform = document.network_interface.document_metadata().transform_to_viewport(layer);
		let center = viewport_transform.transform_point2(DVec2::ZERO);
		let current_mouse_layer = viewport_transform.inverse().transform_point2(input.mouse.position);
		let previous_mouse_layer = viewport_transform.inverse().transform_point2(self.previous_mouse);
		let drag_start = viewport_transform.inverse().transform_point2(drag_start);
		let center_layer = DVec2::ZERO;

		let delta_vector = current_mouse_layer - previous_mouse_layer;
		let sign = (current_mouse_layer - previous_mouse_layer).dot(drag_start - center_layer).signum();
		let delta = delta_vector.length() * sign;

		self.previous_mouse = input.mouse.position;

		let (net_radius, index) = match self.spiral_type {
			SpiralType::Archimedean => {
				let current_radius = extract_arc_spiral_parameters(layer, document)
					.map(|(a, _, _)| a)
					.expect("Failed to get archimedean spiral inner radius");
				((current_radius + delta).max(0.), ARCHIMEDEAN_INNER_RADIUS_INDEX)
			}
			SpiralType::Logarithmic => {
				let current_radius = extract_log_spiral_parameters(layer, document)
					.map(|(a, _, _)| a)
					.expect("Failed to get logarithmic spiral inner radius");
				((current_radius + delta).max(0.001), LOGARITHMIC_START_RADIUS_INDEX)
			}
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, index),
			input: NodeInput::value(TaggedValue::F64(net_radius), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}
