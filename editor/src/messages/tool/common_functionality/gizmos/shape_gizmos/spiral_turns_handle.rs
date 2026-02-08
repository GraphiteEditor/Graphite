use crate::consts::{COLOR_OVERLAY_RED, POINT_RADIUS_HANDLE_SNAP_THRESHOLD};
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::Responses;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_spiral_parameters;
use crate::messages::tool::common_functionality::shapes::spiral_shape::calculate_spiral_endpoints;
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::NodeInputDecleration;
use graphene_std::subpath::{calculate_b, spiral_point};
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

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, _responses: &mut VecDeque<Message>) {
		let viewport = document.metadata().transform_to_viewport(layer);

		match &self.handle_state {
			SpiralTurnsState::Inactive => {
				if let Some((spiral_type, start_angle, inner_radius, outer_radius, turns, _)) = extract_spiral_parameters(layer, document) {
					let b = calculate_b(inner_radius, turns, outer_radius, spiral_type);
					let end_point = viewport.transform_point2(spiral_point(turns * TAU + start_angle.to_radians(), inner_radius, b, spiral_type));
					let start_point = viewport.transform_point2(spiral_point(0. + start_angle.to_radians(), inner_radius, b, spiral_type));

					if mouse_position.distance(end_point) < POINT_RADIUS_HANDLE_SNAP_THRESHOLD {
						self.store_initial_parameters(layer, inner_radius, turns, outer_radius, mouse_position, start_angle, GizmoType::End, spiral_type);
						return;
					}

					if mouse_position.distance(start_point) < POINT_RADIUS_HANDLE_SNAP_THRESHOLD {
						self.store_initial_parameters(layer, inner_radius, turns, outer_radius, mouse_position, start_angle, GizmoType::Start, spiral_type);
						return;
					}
				}
			}
			SpiralTurnsState::Hover | SpiralTurnsState::Dragging => {}
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, layer: Option<LayerNodeIdentifier>, _shape_editor: &mut &mut ShapeState, _mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		let Some(layer) = layer.or(self.layer) else { return };
		let viewport = document.metadata().transform_to_viewport(layer);

		match &self.handle_state {
			SpiralTurnsState::Inactive => {
				if let Some((p1, p2)) = calculate_spiral_endpoints(layer, document, viewport, 0.).zip(calculate_spiral_endpoints(layer, document, viewport, TAU)) {
					overlay_context.manipulator_handle(p1, false, None);
					overlay_context.manipulator_handle(p2, false, None);
				}
			}
			SpiralTurnsState::Hover | SpiralTurnsState::Dragging => {
				// Is true only when hovered over the gizmo
				let selected = self.layer.is_some();
				let angle = match self.gizmo_type {
					GizmoType::End => TAU,
					GizmoType::Start => 0.,
					GizmoType::None => return,
				};

				if let Some(endpoint) = calculate_spiral_endpoints(layer, document, viewport, angle) {
					overlay_context.manipulator_handle(endpoint, selected, Some(COLOR_OVERLAY_RED));
				}
			}
		}
	}

	pub fn update_number_of_turns(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		use graphene_std::vector::generator_nodes::spiral::*;

		let Some(layer) = self.layer else {
			return;
		};

		let viewport = document.metadata().transform_to_viewport(layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		let angle_delta = viewport
			.inverse()
			.transform_vector2(input.mouse.position - center)
			.angle_to(viewport.inverse().transform_vector2(self.previous_mouse_position - center))
			.to_degrees();

		// Skip update if angle calculation produced NaN or infinity (can happen when mouse is at center)
		// Also skip very small angle changes to reduce jitter near center
		if !angle_delta.is_finite() || angle_delta.abs() < 0.5 {
			self.previous_mouse_position = input.mouse.position;
			return;
		}

		// Increase the number of turns and outer radius in unison such that growth and tightness remain same
		let total_delta = self.total_angle_delta + angle_delta;
		// Convert the total angle (in degrees) to number of full turns
		let turns_delta = total_delta / 360.;

		// Calculate the new outer radius based on spiral type and turn change
		let outer_radius_change = match self.spiral_type {
			SpiralType::Archimedean => turns_delta * (self.initial_b) * TAU,
			SpiralType::Logarithmic => self.initial_outer_radius * ((self.initial_b * TAU * turns_delta).exp() - 1.),
		};

		// Skip if outer_radius calculation produced invalid values
		if !outer_radius_change.is_finite() {
			return;
		}

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		match self.gizmo_type {
			GizmoType::Start => {
				let sign = -1.;
				let new_turns = (self.initial_turns + turns_delta * sign).max(0.5);
				let new_outer_radius = (self.initial_outer_radius + outer_radius_change * sign).max(0.1);

				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, StartAngleInput::INDEX),
					input: NodeInput::value(TaggedValue::F64(self.initial_start_angle + total_delta), false),
				});
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, TurnsInput::INDEX),
					input: NodeInput::value(TaggedValue::F64(new_turns), false),
				});
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, OuterRadiusInput::INDEX),
					input: NodeInput::value(TaggedValue::F64(new_outer_radius), false),
				});
			}
			GizmoType::End => {
				let new_turns = (self.initial_turns + turns_delta).max(0.5);
				let new_outer_radius = (self.initial_outer_radius + outer_radius_change).max(0.1);

				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, TurnsInput::INDEX),
					input: NodeInput::value(TaggedValue::F64(new_turns), false),
				});
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, OuterRadiusInput::INDEX),
					input: NodeInput::value(TaggedValue::F64(new_outer_radius), false),
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
