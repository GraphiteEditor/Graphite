use crate::consts::{COLOR_OVERLAY_RED, POINT_RADIUS_HANDLE_SNAP_THRESHOLD, SPIRAL_OUTER_RADIUS_INDEX, SPIRAL_START_ANGLE, SPIRAL_TURNS_INDEX};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::Responses;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::{
	calculate_b, extract_arc_or_log_spiral_parameters, get_arc_or_log_spiral_endpoints, get_arc_spiral_end_point, get_log_spiral_end_point, get_spiral_type, spiral_point,
};
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::misc::SpiralType;
use std::collections::VecDeque;
use std::f64::consts::TAU;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum GizmoType {
	#[default]
	None,
	Start,
	End,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SpiralTurnsState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default)]
pub struct SpiralTurns {
	pub layer: Option<LayerNodeIdentifier>,
	pub handle_state: SpiralTurnsState,
	initial_turns: f64,
	initial_outer_radius: f64,
	initial_inner_radius: f64,
	initial_b: f64,
	initial_start_angle: f64,
	previous_mouse_position: DVec2,
	total_angle_delta: f64,
	gizmo_type: GizmoType,
	spiral_type: SpiralType,
}

impl SpiralTurns {
	pub fn cleanup(&mut self) {
		self.handle_state = SpiralTurnsState::Inactive;
		self.total_angle_delta = 0.;
		self.gizmo_type = GizmoType::None;
		self.layer = None;
	}

	pub fn update_state(&mut self, state: SpiralTurnsState) {
		self.handle_state = state;
	}

	pub fn hovered(&self) -> bool {
		self.handle_state == SpiralTurnsState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.handle_state == SpiralTurnsState::Dragging
	}

	pub fn store_initial_parameters(
		&mut self,
		layer: LayerNodeIdentifier,
		a: f64,
		turns: f64,
		outer_radius: f64,
		mouse_position: DVec2,
		start_angle: f64,
		gizmo_type: GizmoType,
		spiral_type: SpiralType,
	) {
		self.layer = Some(layer);
		self.initial_turns = turns;
		self.initial_b = calculate_b(a, turns, outer_radius, spiral_type);
		self.initial_inner_radius = a;
		self.initial_outer_radius = outer_radius;
		self.initial_start_angle = start_angle;
		self.previous_mouse_position = mouse_position;
		self.spiral_type = spiral_type;
		self.gizmo_type = gizmo_type;
		self.update_state(SpiralTurnsState::Hover);
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let viewport = document.metadata().transform_to_viewport(layer);

		match &self.handle_state {
			SpiralTurnsState::Inactive => {
				// Archimedean
				if let Some(((inner_radius, outer_radius, turns, start_angle), spiral_type)) = extract_arc_or_log_spiral_parameters(layer, document).zip(get_spiral_type(layer, document)) {
					let b = calculate_b(inner_radius, turns, outer_radius, spiral_type);
					let end_point = viewport.transform_point2(spiral_point(turns * TAU + start_angle.to_radians(), inner_radius, b, spiral_type));
					let start_point = viewport.transform_point2(spiral_point(0. + start_angle.to_radians(), inner_radius, b, spiral_type));

					if mouse_position.distance(end_point) < POINT_RADIUS_HANDLE_SNAP_THRESHOLD {
						self.store_initial_parameters(layer, inner_radius, turns, outer_radius, mouse_position, start_angle, GizmoType::End, spiral_type);
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
						return;
					}

					if mouse_position.distance(start_point) < POINT_RADIUS_HANDLE_SNAP_THRESHOLD {
						self.store_initial_parameters(layer, inner_radius, turns, outer_radius, mouse_position, start_angle, GizmoType::Start, spiral_type);
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
						return;
					}
				}
			}
			SpiralTurnsState::Hover | SpiralTurnsState::Dragging => {}
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, layer: Option<LayerNodeIdentifier>, _shape_editor: &mut &mut ShapeState, _mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		match &self.handle_state {
			SpiralTurnsState::Inactive | SpiralTurnsState::Hover | SpiralTurnsState::Dragging => {
				let Some(layer) = layer.or(self.layer) else { return };
				let viewport = document.metadata().transform_to_viewport(layer);

				// Is true only when hovered over the gizmo
				let selected = self.layer.is_some();

				let angle = match self.gizmo_type {
					GizmoType::End => TAU,
					GizmoType::Start => 0.,
					GizmoType::None => return,
				};

				if let Some(endpoint) = get_arc_or_log_spiral_endpoints(layer, document, viewport, angle) {
					overlay_context.manipulator_handle(endpoint, selected, Some(COLOR_OVERLAY_RED));
				}
			}
		}
	}

	pub fn update_number_of_turns(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(layer) = self.layer else {
			return;
		};

		let viewport = document.metadata().transform_to_viewport(layer);

		let angle_delta = viewport
			.inverse()
			.transform_point2(input.mouse.position)
			.angle_to(viewport.inverse().transform_point2(self.previous_mouse_position))
			.to_degrees();

		// Increase the number of turns and outer radius in unison such that growth and tightness remain same
		let total_delta = self.total_angle_delta + angle_delta;

		// Convert the total angle (in degrees) to number of full turns
		let turns_delta = total_delta / 360.;

		// Calculate the new outer radius based on spiral type and turn change
		let outer_radius_change = match self.spiral_type {
			SpiralType::Archimedean => turns_delta * (self.initial_b) * TAU,
			SpiralType::Logarithmic => self.initial_outer_radius * ((self.initial_b * TAU * turns_delta).exp() - 1.),
		};

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		match self.gizmo_type {
			GizmoType::Start => {
				let sign = total_delta.signum() * -1.;
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, SPIRAL_START_ANGLE),
					input: NodeInput::value(TaggedValue::F64(self.initial_start_angle + total_delta), false),
				});

				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, SPIRAL_TURNS_INDEX),
					input: NodeInput::value(TaggedValue::F64(self.initial_turns + turns_delta * sign), false),
				});

				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, SPIRAL_OUTER_RADIUS_INDEX),
					input: NodeInput::value(TaggedValue::F64(self.initial_outer_radius + outer_radius_change * sign), false),
				});
			}
			GizmoType::End => {
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, SPIRAL_TURNS_INDEX),
					input: NodeInput::value(TaggedValue::F64(self.initial_turns + turns_delta), false),
				});

				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, SPIRAL_OUTER_RADIUS_INDEX),
					input: NodeInput::value(TaggedValue::F64(self.initial_outer_radius + outer_radius_change), false),
				});
			}
			GizmoType::None => {
				return;
			}
		}

		responses.add(NodeGraphMessage::RunDocumentGraph);

		self.total_angle_delta += angle_delta;
		self.previous_mouse_position = input.mouse.position;
	}
}
