use crate::consts::{SPIRAL_INNER_RADIUS_INDEX, SPIRAL_OUTER_RADIUS_INDEX};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::Responses;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::tool::common_functionality::graph_modification_utils::{self};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::{calculate_b, extract_arc_or_log_spiral_parameters, get_arc_spiral_end_point, get_spiral_type, spiral_point};
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::uuid::NodeId;
use graphene_std::vector::misc::SpiralType;
use std::collections::VecDeque;
use std::f64::consts::TAU;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum TightnessGizmoState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default)]
pub struct TightnessGizmo {
	pub layer: Option<LayerNodeIdentifier>,
	pub handle_state: TightnessGizmoState,
	initial_outer_radius: f64,
	spiral_type: SpiralType,
	gizmo_line_points: Option<(DVec2, DVec2)>,
	inner_radius: f64,
	angle: f64,
	spiral_slot: i32,
	turns: f64,
	previous_mouse: DVec2,
}

impl TightnessGizmo {
	pub fn cleanup(&mut self) {
		self.handle_state = TightnessGizmoState::Inactive;
		self.layer = None;
		self.gizmo_line_points = None;
	}

	pub fn update_state(&mut self, state: TightnessGizmoState) {
		self.handle_state = state;
	}

	pub fn hovered(&self) -> bool {
		self.handle_state == TightnessGizmoState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.handle_state == TightnessGizmoState::Dragging
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let viewport = document.metadata().transform_to_viewport(layer);

		match &self.handle_state {
			TightnessGizmoState::Inactive => {
				// Archimedean
				if let Some(((a, outer_radius, turns, start_angle), spiral_type)) = extract_arc_or_log_spiral_parameters(layer, document).zip(get_spiral_type(layer, document)) {
					if let Some((start, end, slot_index)) =
						Self::check_which_inter_segment(viewport.inverse().transform_point2(mouse_position), a, outer_radius, turns, start_angle, spiral_type, viewport)
					{
						log::info!("check which inter-segment {:?}", slot_index);
						self.layer = Some(layer);
						self.initial_outer_radius = outer_radius;
						self.previous_mouse = mouse_position;
						self.gizmo_line_points = Some((start, end));
						self.spiral_type = spiral_type;
						self.spiral_slot = slot_index;
						self.turns = turns;
						self.inner_radius = a;
						self.angle = viewport.inverse().transform_point2(mouse_position).angle_to(DVec2::X).rem_euclid(TAU);
						self.update_state(TightnessGizmoState::Hover);
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });

						return;
					};
				}
			}
			TightnessGizmoState::Hover | TightnessGizmoState::Dragging => {}
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, selected_spiral_layer: Option<LayerNodeIdentifier>, _shape_editor: &mut &mut ShapeState, overlay_context: &mut OverlayContext) {
		let Some(layer) = selected_spiral_layer.or(self.layer) else {
			return;
		};
		let viewport = document.metadata().transform_to_viewport(layer);

		match &self.handle_state {
			TightnessGizmoState::Hover | TightnessGizmoState::Dragging => {
				let is_reversed = self.inner_radius > self.initial_outer_radius;
				if let Some((start, end)) = self.gizmo_line_points {
					overlay_context.dashed_line(start, end, None, None, Some(4.0), Some(4.0), Some(0.5));
					if self.spiral_slot == 0 && !is_reversed {
						let required_radius = viewport
							.inverse()
							.transform_point2(get_arc_spiral_end_point(layer, document, viewport, 0.).expect("Failed to get endpoints"))
							.distance(DVec2::ZERO);
						overlay_context.dashed_circle(DVec2::ZERO, required_radius.max(5.), None, None, Some(4.), Some(4.), Some(0.5), Some(viewport));
					}

					if self.spiral_slot == self.turns.floor() as i32 && is_reversed {
						let required_radius = viewport
							.inverse()
							.transform_point2(get_arc_spiral_end_point(layer, document, viewport, TAU).expect("Failed to get endpoints"))
							.distance(DVec2::ZERO);
						overlay_context.dashed_circle(DVec2::ZERO, required_radius.max(5.), None, None, Some(4.), Some(4.), Some(0.5), Some(viewport));
					}
				};
			}

			TightnessGizmoState::Inactive => {}
		}
	}

	fn check_which_inter_segment(
		layer_mouse_position: DVec2,
		inner_radius: f64,
		outer_radius: f64,
		turns: f64,
		start_angle: f64,
		spiral_type: SpiralType,
		viewport: DAffine2,
	) -> Option<(DVec2, DVec2, i32)> {
		let center = DVec2::ZERO;
		let angle = layer_mouse_position.angle_to(DVec2::X).rem_euclid(TAU);
		let start_angle_rad = start_angle.to_radians();
		let normalized_start_angle = start_angle_rad.rem_euclid(TAU);

		let is_reversed = inner_radius > outer_radius;
		let b = calculate_b(inner_radius, turns, outer_radius, spiral_type);
		let max_theta = turns * TAU;

		let spiral_theta = if is_reversed {
			angle + ((start_angle.to_radians() / TAU).floor() * TAU) + turns.floor() * TAU
		} else {
			angle + ((start_angle.to_radians() / TAU).floor() * TAU)
		};

		let viewport_mouse = viewport.transform_point2(layer_mouse_position);
		let viewport_center = viewport.transform_point2(center);

		let spiral_outer = spiral_point(max_theta + start_angle_rad, inner_radius, b, spiral_type);
		let spiral_inner = spiral_point(0. + start_angle_rad, inner_radius, b, spiral_type);
		let viewport_outer = viewport.transform_point2(spiral_outer);
		let viewport_inner = viewport.transform_point2(spiral_inner);

		let first_segment = spiral_point(angle + ((start_angle.to_radians() / TAU).floor() * TAU), inner_radius, b, spiral_type);
		let fist_segment_point_distance = viewport.transform_point2(first_segment).distance(viewport_center);

		let required_endpoint = if is_reversed { viewport_inner } else { viewport_outer };
		let mouse_distance = viewport_mouse.distance(viewport_center);
		let max_radius = required_endpoint.distance(viewport_center);

		if mouse_distance > max_radius {
			return None;
		}

		if is_reversed && mouse_distance > fist_segment_point_distance {
			return None;
		}

		let mut segment_index = if is_reversed { turns.floor() as i32 } else { 0 };

		if !is_reversed {
			let spiral_end = spiral_point(spiral_theta, inner_radius, b, spiral_type);
			let first_point = viewport.transform_point2(spiral_end);

			let r_end = first_point.distance(viewport_center);

			if mouse_distance <= r_end {
				let direction = DVec2::new(angle.cos(), -angle.sin());
				let radius = if is_reversed { spiral_outer.distance(DVec2::ZERO) } else { spiral_inner.distance(DVec2::ZERO) };
				let inner_point = viewport.transform_point2(radius.max(5.0) * direction);
				return Some((inner_point, first_point, segment_index));
			}

			if angle <= normalized_start_angle {
				let spiral_end = spiral_point(spiral_theta + TAU, inner_radius, b, spiral_type);
				let first_point = viewport.transform_point2(spiral_end);
				if mouse_distance <= first_point.distance(viewport_center) {
					let direction = DVec2::new(angle.cos(), -angle.sin());

					let radius = if is_reversed { spiral_outer.distance(DVec2::ZERO) } else { spiral_inner.distance(DVec2::ZERO) };
					let inner_point = viewport.transform_point2(radius.max(5.0) * direction);

					return Some((inner_point, first_point, segment_index));
				}
			}

			segment_index += 1;
		} else {
			let spiral_end = spiral_point(spiral_theta, inner_radius, b, spiral_type);
			let first_point = viewport.transform_point2(spiral_end);

			let r_end = first_point.distance(viewport_center);

			if mouse_distance <= r_end {
				let direction = DVec2::new(angle.cos(), -angle.sin());
				let radius = if is_reversed { spiral_outer.distance(DVec2::ZERO) } else { spiral_inner.distance(DVec2::ZERO) };
				let inner_point = viewport.transform_point2(radius.max(5.0) * direction);
				return Some((inner_point, first_point, segment_index));
			}

			if angle >= (max_theta + start_angle_rad).rem_euclid(TAU) {
				let spiral_end = spiral_point(spiral_theta - TAU, inner_radius, b, spiral_type);
				let first_point = viewport.transform_point2(spiral_end);
				if mouse_distance <= first_point.distance(viewport_center) {
					let direction = DVec2::new(angle.cos(), -angle.sin());

					let radius = if is_reversed { spiral_outer.distance(DVec2::ZERO) } else { spiral_inner.distance(DVec2::ZERO) };
					let inner_point = viewport.transform_point2(radius.max(5.0) * direction);

					return Some((inner_point, first_point, segment_index));
				}
			}

			segment_index -= 1;
		}

		// Remaining segments: full spiral loops
		let mut base_theta = spiral_theta;
		while if is_reversed {
			base_theta >= (start_angle_rad).rem_euclid(TAU)
		} else {
			base_theta <= max_theta + start_angle_rad
		} {
			let theta_start = base_theta;
			let theta_end = if is_reversed { base_theta - TAU } else { base_theta + TAU };
			log::info!("segment index {:?}, theta_start {:?} ,theta_end {:?}", segment_index, theta_start.to_degrees(), theta_end.to_degrees());

			if (!is_reversed && theta_end > max_theta + start_angle_rad) || (is_reversed && theta_end < 0.0) {
				break;
			}
			if is_reversed && (theta_start > max_theta + start_angle_rad || theta_end > max_theta + start_angle_rad) {
				base_theta -= TAU;
				segment_index -= 1;
				continue;
			}

			let spiral_start = spiral_point(theta_start, inner_radius, b, spiral_type);
			let spiral_end = spiral_point(theta_end, inner_radius, b, spiral_type);

			let viewport_start = viewport.transform_point2(spiral_start);
			let viewport_end = viewport.transform_point2(spiral_end);

			let r_start = viewport_start.distance(viewport_center);
			let r_end = viewport_end.distance(viewport_center);

			if mouse_distance >= r_start.min(r_end) && mouse_distance <= r_start.max(r_end) {
				return Some((viewport_start, viewport_end, segment_index));
			}

			base_theta = if is_reversed { base_theta - TAU } else { base_theta + TAU };
			if is_reversed {
				segment_index -= 1;
			} else {
				segment_index += 1;
			}
		}

		None
	}

	pub fn calculate_updated_dash_lines(
		&self,
		inner_radius: f64,
		outer_radius: f64,
		turns: f64,
		start_angle: f64,
		spiral_type: SpiralType,
		viewport: DAffine2,
		_drag_start: DVec2,
		reversed: bool,
	) -> (DVec2, DVec2) {
		let b = calculate_b(inner_radius, turns, outer_radius, spiral_type);
		let start_angle_rad = start_angle.to_radians();
		let max_theta = turns * TAU + start_angle_rad;

		let base_angle = if reversed {
			self.angle + ((start_angle_rad / TAU).floor() * TAU) + turns.floor() * TAU
		} else {
			self.angle + ((start_angle_rad / TAU).floor() * TAU)
		};

		let (start_point, end_point) = if self.spiral_slot == 0 && !reversed {
			let endpoint = spiral_point(0. + start_angle_rad, inner_radius, b, spiral_type);
			let radius = endpoint.distance(DVec2::ZERO);

			(
				viewport.transform_point2(radius * DVec2::new(self.angle.cos(), -self.angle.sin())),
				viewport.transform_point2(spiral_point(base_angle, inner_radius, b, spiral_type)),
			)
		} else if self.spiral_slot == self.turns.floor() as i32 && reversed {
			log::info!("am i plzz reaching here");
			let radius = spiral_point(max_theta, inner_radius, b, spiral_type).distance(DVec2::ZERO);
			let endpoint = if self.angle >= (max_theta + start_angle_rad).rem_euclid(TAU) {
				viewport.transform_point2(spiral_point(base_angle - TAU, inner_radius, b, spiral_type))
			} else {
				viewport.transform_point2(spiral_point(base_angle, inner_radius, b, spiral_type))
			};
			(viewport.transform_point2(radius * DVec2::new(self.angle.cos(), -self.angle.sin())), endpoint)
		} else {
			let ref_angle = (self.spiral_slot as f64 - 1.) * TAU + base_angle;
			let end_point_angle = if reversed { ref_angle - TAU } else { ref_angle + TAU };
			(
				viewport.transform_point2(spiral_point(ref_angle, inner_radius, b, spiral_type)),
				viewport.transform_point2(spiral_point(end_point_angle, inner_radius, b, spiral_type)),
			)
		};

		(start_point, end_point)
	}

	pub fn update_outer_radius_via_dashed_lines(
		&mut self,
		layer: LayerNodeIdentifier,
		node_id: NodeId,
		viewport_transform: DAffine2,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
		drag_start: DVec2,
	) {
		let current_mouse_layer = viewport_transform.inverse().transform_point2(input.mouse.position);
		let previous_mouse_layer = viewport_transform.inverse().transform_point2(self.previous_mouse);
		let drag_start = viewport_transform.inverse().transform_point2(drag_start);
		let center_layer = DVec2::ZERO;

		let delta_vector = current_mouse_layer - previous_mouse_layer;
		let sign = (current_mouse_layer - previous_mouse_layer).dot(drag_start - center_layer).signum();
		let delta = delta_vector.length() * sign;

		let reversed = self.inner_radius > self.initial_outer_radius;
		self.previous_mouse = input.mouse.position;

		let (a, outer_radius, turns, start_angle) = extract_arc_or_log_spiral_parameters(layer, document).expect("Failed to get archimedean spiral inner radius");
		let (new_inner_radius, turns, new_outer_radius) = match self.spiral_type {
			SpiralType::Archimedean => {
				if reversed {
					((a + delta).max(0.), turns, outer_radius)
				} else {
					(a, turns, (outer_radius + delta).max(0.))
				}
			}
			SpiralType::Logarithmic => {
				if reversed {
					((a + delta).max(0.001), turns, outer_radius)
				} else {
					(a, turns, (outer_radius + delta).max(0.001))
				}
			}
		};

		self.gizmo_line_points = Some(self.calculate_updated_dash_lines(new_inner_radius, new_outer_radius, turns, start_angle, self.spiral_type, viewport_transform, drag_start, reversed));

		let (index, new_radius) = if reversed {
			(SPIRAL_INNER_RADIUS_INDEX, new_inner_radius)
		} else {
			(SPIRAL_OUTER_RADIUS_INDEX, new_outer_radius)
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, index),
			input: NodeInput::value(TaggedValue::F64(new_radius), false),
		});
	}

	pub fn update_outer_radius(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let Some(layer) = self.layer else {
			return;
		};

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		let viewport_transform = document.network_interface.document_metadata().transform_to_viewport(layer);

		self.update_outer_radius_via_dashed_lines(layer, node_id, viewport_transform, document, input, responses, drag_start);

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}
