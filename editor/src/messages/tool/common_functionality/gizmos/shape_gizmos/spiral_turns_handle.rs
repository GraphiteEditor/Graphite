use crate::consts::{COLOR_OVERLAY_RED, SPIRAL_END_HOVER_RADIUS};
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::Responses;
use crate::messages::prelude::{DocumentMessageHandler, NodeGraphMessage};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_spiral_parameters;
use crate::messages::tool::common_functionality::shapes::spiral_shape::calculate_spiral_endpoints;
use crate::messages::tool::common_functionality::snap_session::{Kind, SnapSession, SnapView, ValueCandidate};
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::FallibleVec2Operations;
use graphene_std::vector::algorithms::shapes::{calculate_growth_factor, spiral_point};
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
	initial_growth_factor: f64,
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

	#[expect(clippy::too_many_arguments, reason = "the gizmo records one value per spiral parameter, so grouping them would only move the argument list")]
	pub fn store_initial_parameters(
		&mut self,
		layer: LayerNodeIdentifier,
		inner_radius: f64,
		outer_radius: f64,
		turns: f64,
		start_angle: f64,
		mouse_position: DVec2,
		gizmo_type: GizmoType,
		spiral_type: SpiralType,
	) {
		self.layer = Some(layer);
		self.initial_turns = turns;
		self.initial_growth_factor = calculate_growth_factor(inner_radius, turns, outer_radius, spiral_type);
		self.initial_inner_radius = inner_radius;
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
					let growth_factor = calculate_growth_factor(inner_radius, turns, outer_radius, spiral_type);
					let end_point = viewport.transform_point2(spiral_point(turns * TAU + start_angle.to_radians(), inner_radius, growth_factor, spiral_type));
					let start_point = viewport.transform_point2(spiral_point(0. + start_angle.to_radians(), inner_radius, growth_factor, spiral_type));

					if mouse_position.distance(end_point) < SPIRAL_END_HOVER_RADIUS {
						self.store_initial_parameters(layer, inner_radius, outer_radius, turns, start_angle, mouse_position, GizmoType::End, spiral_type);
					} else if mouse_position.distance(start_point) < SPIRAL_END_HOVER_RADIUS {
						self.store_initial_parameters(layer, inner_radius, outer_radius, turns, start_angle, mouse_position, GizmoType::Start, spiral_type);
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

	/// How much the outer radius grows as the spiral gains a number of turns, keeping its growth and tightness the same.
	fn outer_radius_change(&self, turns_gained: f64) -> f64 {
		match self.spiral_type {
			SpiralType::Archimedean => turns_gained * self.initial_growth_factor * TAU,
			SpiralType::Logarithmic => self.initial_outer_radius * ((self.initial_growth_factor * TAU * turns_gained).exp() - 1.),
		}
	}

	/// The start angle in degrees, the turns, and the outer radius that dragging the start writes for a number of turns, which turns the start along as turns come off.
	fn start_dragged_to(&self, turns: f64) -> (f64, f64, f64) {
		let turns_taken_off = self.initial_turns - turns;
		let outer_radius = (self.initial_outer_radius - self.outer_radius_change(turns_taken_off)).max(0.1);
		(self.initial_start_angle + turns_taken_off * 360., turns.max(0.5), outer_radius)
	}

	/// Snaps a number of turns to whole and half turns, judged by how far the pointer travels around the center on screen.
	fn snap_turns(&self, view: SnapView, snap_session: &mut SnapSession, layer: LayerNodeIdentifier, free_turns: f64) -> f64 {
		let document = view.document;
		let Some((spiral_type, start_angle, inner_radius, outer_radius, turns, _)) = extract_spiral_parameters(layer, document) else {
			return free_turns;
		};
		if !free_turns.is_finite() {
			return free_turns;
		}

		// The turns follow the pointer's angle around the center, so a turn is a lap of the pointer there
		let layer_to_viewport = document.metadata().transform_to_viewport(layer);
		let on_screen_per_turn = TAU * view.input.mouse.position.distance(layer_to_viewport.transform_point2(DVec2::ZERO));

		let growth_factor = calculate_growth_factor(inner_radius, turns, outer_radius, spiral_type);
		let viewport_to_document = document.metadata().document_to_viewport.inverse();
		let position_of = |turns_landed_on: f64| {
			let (angle, growth_factor) = match self.gizmo_type {
				GizmoType::Start => {
					let (start_angle, turns, outer_radius) = self.start_dragged_to(turns_landed_on);
					(start_angle.to_radians(), calculate_growth_factor(inner_radius, turns, outer_radius, spiral_type))
				}
				_ => (turns_landed_on * TAU + start_angle.to_radians(), growth_factor),
			};
			viewport_to_document.transform_point2(layer_to_viewport.transform_point2(spiral_point(angle, inner_radius, growth_factor, spiral_type)))
		};

		// The whole and half turns around the asked-for number, counted in half turns
		let nearest_half_turns = (free_turns * 2.).round() as i64;
		let candidates: Vec<ValueCandidate> = (nearest_half_turns.saturating_sub(2).max(1)..=nearest_half_turns.saturating_add(2))
			.map(|half_turns| ValueCandidate {
				value: half_turns as f64 / 2.,
				kind: if half_turns % 2 == 0 { Kind::WholeValue } else { Kind::AngleStep },
				identity: half_turns as u64,
			})
			.collect();

		snap_session.snap_value(view, position_of, free_turns, on_screen_per_turn, &candidates)
	}

	pub fn update_number_of_turns(&mut self, view: SnapView, snap_session: &mut SnapSession, responses: &mut VecDeque<Message>) {
		use graphene_std::vector::generator_nodes::spiral::*;

		let SnapView { document, input, .. } = view;
		let Some(layer) = self.layer else {
			return;
		};

		let viewport = document.metadata().transform_to_viewport(layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		let current_mouse_vector = viewport.inverse().transform_vector2(input.mouse.position - center);
		let previous_mouse_vector = viewport.inverse().transform_vector2(self.previous_mouse_position - center);

		let Some(angle_delta) = current_mouse_vector.try_angle_to(previous_mouse_vector).map(f64::to_degrees) else {
			self.previous_mouse_position = input.mouse.position;
			return;
		};

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

		// Dragging the start takes turns away rather than adding them
		let direction = match self.gizmo_type {
			GizmoType::Start => -1.,
			GizmoType::End => 1.,
			GizmoType::None => return,
		};
		let free_turns = self.initial_turns + turns_delta * direction;
		let snap_correction = (self.snap_turns(view, snap_session, layer, free_turns) - free_turns) * direction;
		let turns_delta = turns_delta + snap_correction;

		// Calculate the new outer radius based on spiral type and turn change
		let outer_radius_change = self.outer_radius_change(turns_delta);

		// Skip if outer_radius calculation produced invalid values
		if !outer_radius_change.is_finite() {
			return;
		}

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		match self.gizmo_type {
			GizmoType::Start => {
				let (new_start_angle, new_turns, new_outer_radius) = self.start_dragged_to(self.initial_turns - turns_delta);

				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, StartAngleInput),
					input: NodeInput::value(TaggedValue::Number(new_start_angle), false),
				});
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, TurnsInput),
					input: NodeInput::value(TaggedValue::Number(new_turns), false),
				});
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, OuterRadiusInput),
					input: NodeInput::value(TaggedValue::Number(new_outer_radius), false),
				});
			}
			GizmoType::End => {
				let new_turns = (self.initial_turns + turns_delta).max(0.5);
				let new_outer_radius = (self.initial_outer_radius + outer_radius_change).max(0.1);

				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, TurnsInput),
					input: NodeInput::value(TaggedValue::Number(new_turns), false),
				});
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, OuterRadiusInput),
					input: NodeInput::value(TaggedValue::Number(new_outer_radius), false),
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
