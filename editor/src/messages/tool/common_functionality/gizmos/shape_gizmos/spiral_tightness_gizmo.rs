use crate::consts::{COLOR_OVERLAY_RED, SPIRAL_INNER_RADIUS_INDEX, SPIRAL_INNER_RADIUS_INDEX_GIZMO_THRESHOLD, SPIRAL_OUTER_RADIUS_INDEX};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::Responses;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::tool::common_functionality::graph_modification_utils::{self};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::{
	calculate_b, extract_arc_or_log_spiral_parameters, get_arc_spiral_end_point, get_log_spiral_end_point, get_spiral_type, spiral_point,
};
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::uuid::NodeId;
use graphene_std::vector::misc::{SpiralType, dvec2_to_point};
use kurbo::{Line, ParamCurveNearest};
use std::collections::VecDeque;
use std::f64::consts::TAU;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum TightnessGizmoState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default, PartialEq)]
enum TightnessGizmoType {
	#[default]
	None,
	Circle,
	DashLines,
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
	previous_mouse: DVec2,
	gizmo_type: TightnessGizmoType,
}

impl TightnessGizmo {
	pub fn cleanup(&mut self) {
		self.handle_state = TightnessGizmoState::Inactive;
		self.layer = None;
		self.gizmo_line_points = None;
		self.gizmo_type = TightnessGizmoType::None;
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
				if let Some(((a, outer_radius, turns, _), spiral_type)) = extract_arc_or_log_spiral_parameters(layer, document).zip(get_spiral_type(layer, document)) {
					if let Some((start, end, slot_index)) = Self::check_which_inter_segment(viewport.inverse().transform_point2(mouse_position), outer_radius, turns, a, spiral_type, viewport) {
						self.layer = Some(layer);
						self.initial_outer_radius = outer_radius;
						self.previous_mouse = mouse_position;
						self.gizmo_line_points = Some((start, end));
						self.spiral_type = spiral_type;
						self.spiral_slot = slot_index;
						self.inner_radius = a;
						self.gizmo_type = TightnessGizmoType::DashLines;
						self.angle = viewport.inverse().transform_point2(mouse_position).angle_to(DVec2::X).rem_euclid(TAU);
						self.update_state(TightnessGizmoState::Hover);
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });

						return;
					};

					let center = viewport.transform_point2(DVec2::ZERO);

					let angle = if a > outer_radius { 0. } else { TAU };

					let Some(endpoint) = get_arc_spiral_end_point(layer, document, viewport, angle).or(get_log_spiral_end_point(layer, document, viewport, angle)) else {
						return;
					};

					let close_to_circle = (endpoint.distance(center) - mouse_position.distance(center)).abs() < 5.;

					if close_to_circle {
						self.layer = Some(layer);
						self.inner_radius = a;
						self.initial_outer_radius = outer_radius;
						self.previous_mouse = mouse_position;
						self.gizmo_type = TightnessGizmoType::Circle;
						self.spiral_type = spiral_type;
						self.update_state(TightnessGizmoState::Hover);
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
					}
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
			TightnessGizmoState::Hover | TightnessGizmoState::Dragging => match self.gizmo_type {
				TightnessGizmoType::Circle => {
					if let Some((inner_radius, outer_radius, _, _)) = extract_arc_or_log_spiral_parameters(layer, document) {
						let required_radius = if self.inner_radius > self.initial_outer_radius { inner_radius } else { outer_radius };
						overlay_context.dashed_circle(DVec2::ZERO, required_radius.max(5.), None, None, Some(8.), Some(4.), Some(0.5), Some(viewport));
					}
				}
				TightnessGizmoType::DashLines => {
					if let Some((start, end)) = self.gizmo_line_points {
						overlay_context.dashed_line(start, end, None, None, Some(4.0), Some(4.0), Some(0.5));
						if self.spiral_slot == 0 {
							let required_radius = if self.inner_radius > self.initial_outer_radius {
								self.initial_outer_radius
							} else {
								self.inner_radius
							};
							overlay_context.dashed_circle(DVec2::ZERO, required_radius.max(5.), None, None, Some(4.), Some(4.), Some(0.5), Some(viewport));
						}
					};
				}
				TightnessGizmoType::None => {}
			},
			TightnessGizmoState::Inactive => {}
		}
	}

	fn check_which_inter_segment(layer_mouse_position: DVec2, outer_radius: f64, turns: f64, inner_radius: f64, spiral_type: SpiralType, viewport: DAffine2) -> Option<(DVec2, DVec2, i32)> {
		let center = DVec2::ZERO;
		let mut angle = layer_mouse_position.angle_to(DVec2::X).rem_euclid(TAU);

		let is_reversed = inner_radius > outer_radius;
		let b = calculate_b(inner_radius, turns, outer_radius, spiral_type);
		let max_theta = turns * TAU;

		let viewport_mouse = viewport.transform_point2(layer_mouse_position);
		let viewport_center = viewport.transform_point2(center);

		// Compute spiral endpoints at θ = 0 and θ = max
		let spiral_outer = spiral_point(max_theta, inner_radius, b, spiral_type);
		let spiral_inner = spiral_point(0., inner_radius, b, spiral_type);
		let viewport_outer = viewport.transform_point2(spiral_outer);
		let viewport_inner = viewport.transform_point2(spiral_inner);

		let smaller_radius = inner_radius.min(outer_radius);
		let adjusted_angle = if is_reversed { max_theta - (TAU - angle) } else { angle };

		let required_endpoint = if is_reversed { viewport_inner } else { viewport_outer };

		// Reject if mouse is beyond spiral's radial extent
		if viewport_mouse.distance(viewport_center) > required_endpoint.distance(viewport_center) {
			return None;
		}

		let mouse_distance = viewport_mouse.distance(viewport_center);
		let mut segment_index = 0;

		// First segment: from center to first spiral point at θ = adjusted_angle
		{
			let spiral_end = spiral_point(adjusted_angle, inner_radius, b, spiral_type);
			let first_point = viewport.transform_point2(spiral_end);
			let r_end = first_point.distance(viewport_center);

			if mouse_distance <= r_end {
				let direction = DVec2::new(adjusted_angle.cos(), -adjusted_angle.sin());
				return Some((viewport.transform_point2(smaller_radius.max(5.) * direction), first_point, segment_index));
			}

			segment_index += 1;
		}

		// Loop through each full turn segment along the spiral ray
		let mut base_theta = adjusted_angle;
		while if is_reversed { base_theta >= 0. } else { base_theta <= max_theta } {
			let theta_start = base_theta;
			let theta_end = if is_reversed { base_theta - TAU } else { base_theta + TAU };

			if (!is_reversed && theta_end > max_theta) || (is_reversed && theta_end < 0.) {
				break;
			}

			let spiral_start = spiral_point(theta_start, inner_radius, b, spiral_type);
			let spiral_end = spiral_point(theta_end, inner_radius, b, spiral_type);

			let viewport_start = viewport.transform_point2(spiral_start);
			let viewport_end = viewport.transform_point2(spiral_end);

			let r_start = viewport_start.distance(viewport_center);
			let r_end = viewport_end.distance(viewport_center);

			if mouse_distance >= r_start.min(r_end) && mouse_distance <= r_start.max(r_end) {
				let (point1, point2) = Self::calculate_gizmo_line_points(viewport_start, viewport_end);
				return Some((point1, point2, segment_index));
			}

			base_theta = if is_reversed { base_theta - TAU } else { base_theta + TAU };

			segment_index += 1;
		}

		None
	}

	pub fn calculate_updated_dash_lines(&self, inner_radius: f64, outer_radius: f64, turns: f64, spiral_type: SpiralType, viewport: DAffine2, drag_start: DVec2, reversed: bool) -> (DVec2, DVec2) {
		let b = calculate_b(inner_radius, turns, outer_radius, spiral_type);
		let max_theta = turns * TAU;
		let base_angle = if reversed { max_theta - (TAU - self.angle) } else { self.angle };
		let smaller_radius = inner_radius.min(outer_radius);

		let center = DVec2::ZERO;

		let (start_point, end_point) = if self.spiral_slot == 0 {
			(
				viewport.transform_point2(smaller_radius * DVec2::new(base_angle.cos(), -base_angle.sin())),
				viewport.transform_point2(spiral_point(base_angle, inner_radius, b, spiral_type)),
			)
		} else {
			let ref_angle = (self.spiral_slot as f64 - 1.) * TAU + base_angle;
			let end_point_angle = if reversed { ref_angle - TAU } else { ref_angle + TAU };
			(
				viewport.transform_point2(spiral_point(ref_angle, inner_radius, b, spiral_type)),
				viewport.transform_point2(spiral_point(end_point_angle, inner_radius, b, spiral_type)),
			)
		};

		Self::calculate_gizmo_line_points(start_point, end_point)
	}

	// (start_point,end_point)
	fn calculate_gizmo_line_points(start_point: DVec2, end_point: DVec2) -> (DVec2, DVec2) {
		let length = start_point.distance(end_point);

		let direction = (end_point - start_point).normalize_or_zero();

		let new_endpoint = end_point - direction * length;
		let new_start_point = start_point + direction * length;

		(new_start_point, new_endpoint)
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

		let (a, outer_radius, turns, _) = extract_arc_or_log_spiral_parameters(layer, document).expect("Failed to get archimedean spiral inner radius");
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

		let b = calculate_b(new_inner_radius, turns, new_outer_radius, self.spiral_type);
		self.gizmo_line_points = Some(self.calculate_updated_dash_lines(new_inner_radius, new_outer_radius, turns, self.spiral_type, viewport_transform, drag_start, reversed));

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

	pub fn update_outer_radius_via_circle(&mut self, node_id: NodeId, viewport_transform: DAffine2, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let current_mouse_layer = viewport_transform.inverse().transform_point2(input.mouse.position);
		let net_radius = current_mouse_layer.distance(DVec2::ZERO);

		let net_radius = match self.spiral_type {
			SpiralType::Archimedean => net_radius.max(0.),
			SpiralType::Logarithmic => net_radius.max(0.001),
		};

		let index = if self.initial_outer_radius > self.inner_radius {
			SPIRAL_OUTER_RADIUS_INDEX
		} else {
			SPIRAL_INNER_RADIUS_INDEX
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, index),
			input: NodeInput::value(TaggedValue::F64(net_radius), false),
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

		match &self.gizmo_type {
			TightnessGizmoType::Circle => self.update_outer_radius_via_circle(node_id, viewport_transform, input, responses),
			TightnessGizmoType::DashLines => self.update_outer_radius_via_dashed_lines(layer, node_id, viewport_transform, document, input, responses, drag_start),
			TightnessGizmoType::None => {}
		}

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}
