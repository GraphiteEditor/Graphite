use crate::consts::{COLOR_OVERLAY_RED, SPIRAL_INNER_RADIUS_GIZMO_THRESHOLD, SPIRAL_OUTER_RADIUS_INDEX, SPIRAL_TURNS_INDEX};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::Responses;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, get_stroke_width};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::{
	archimedean_spiral_point, calculate_b, extract_arc_spiral_parameters, extract_log_spiral_parameters, get_arc_spiral_end_point, get_log_spiral_end_point,
};
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
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

#[derive(Clone, Debug, Default)]
pub struct TightnessGizmo {
	pub layer: Option<LayerNodeIdentifier>,
	pub handle_state: TightnessGizmoState,
	initial_outer_radius: f64,
	spiral_type: SpiralType,
	gizmo_line_points: Option<(DVec2, DVec2)>,
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
				if let Some((a, outer_radius, turns)) = extract_arc_spiral_parameters(layer, document) {
					let b = calculate_b(a, turns, outer_radius, SpiralType::Archimedean);
					if let Some((start, end)) = Self::check_which_inter_segment(viewport.inverse().transform_point2(mouse_position), outer_radius, turns, a, viewport) {
						let line = Line::new(dvec2_to_point(start), dvec2_to_point(end));
						if line.nearest(dvec2_to_point(mouse_position), 1e-6).distance_sq < SPIRAL_INNER_RADIUS_GIZMO_THRESHOLD {
							self.layer = Some(layer);
							self.initial_outer_radius = outer_radius;
							self.previous_mouse = mouse_position;
							self.gizmo_line_points = Some((start, end));
							self.spiral_type = SpiralType::Archimedean;
							self.update_state(TightnessGizmoState::Hover);
							responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
						}
						return;
					};

					let layer_mouse = viewport.inverse().transform_point2(mouse_position);

					let center = viewport.transform_point2(DVec2::ZERO);

					let Some(endpoint) = get_arc_spiral_end_point(layer, document, viewport, TAU) else { return };

					let close_to_circle = (DVec2::ZERO.distance(layer_mouse) - outer_radius).abs() < 5.;

					if close_to_circle {
						self.layer = Some(layer);
						self.initial_outer_radius = outer_radius;
						self.previous_mouse = mouse_position;
						// self.gizmo_line_points = Some((start, end));
						self.spiral_type = SpiralType::Archimedean;
						self.update_state(TightnessGizmoState::Hover);
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
					}
				}

				// // Logarithmic
				// if let Some(((_, _, turns), end_point)) = extract_log_spiral_parameters(layer, document).zip(get_log_spiral_end_point(layer, document, viewport)) {
				// 	if mouse_position.distance(end_point) < POINT_RADIUS_HANDLE_SNAP_THRESHOLD {
				// 		self.layer = Some(layer);
				// 		self.initial_turns = turns;
				// 		self.previous_mouse_position = mouse_position;
				// 		self.spiral_type = SpiralType::Logarithmic;
				// 		self.update_state(SpiralTurnsState::Hover);
				// 		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
				// 	}
				// }
			}
			TightnessGizmoState::Hover | TightnessGizmoState::Dragging => {
				// let Some(layer) = self.layer else { return };

				// let viewport = document.metadata().transform_to_viewport(layer);
				// let center = viewport.transform_point2(DVec2::ZERO);

				// if mouse_position.distance(center) > NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH && matches!(&self.handle_state, NumberOfPointsDialState::Hover) {
				// 	self.update_state(NumberOfPointsDialState::Inactive);
				// 	self.layer = None;
				// 	responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
				// }
			}
		}
	}

	pub fn overlays(
		&self,
		document: &DocumentMessageHandler,
		selected_spiral_layer: Option<LayerNodeIdentifier>,
		_shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		let Some(layer) = selected_spiral_layer.or(self.layer) else {
			return;
		};
		let viewport = document.metadata().transform_to_viewport(layer);

		match &self.handle_state {
			TightnessGizmoState::Hover | TightnessGizmoState::Dragging => {
				let Some(layer) = selected_spiral_layer.or(self.layer) else {
					return;
				};

				if let Some((start, end)) = self.gizmo_line_points {
					overlay_context.dashed_line(start, end, Some(COLOR_OVERLAY_RED), None, Some(4.0), Some(4.0), Some(0.5));
				};

				let viewport = document.metadata().transform_to_viewport(layer);
				if let Some((_, outer_radius, _)) = extract_arc_spiral_parameters(layer, document).or(extract_log_spiral_parameters(layer, document)) {
					overlay_context.dashed_circle(DVec2::ZERO, outer_radius.max(5.), None, Some(COLOR_OVERLAY_RED), Some(8.), Some(4.), Some(0.5), Some(viewport));
				}

				// let viewport = document.metadata().transform_to_viewport(layer);
				// if let Some((a, outer_radius, turns)) = extract_arc_spiral_parameters(layer, document) {
				// 	let b = calculate_b(a, turns, outer_radius, SpiralType::Archimedean);
				// 	let Some((start, end)) = Self::check_which_inter_segment(viewport.inverse().transform_point2(mouse_position), outer_radius, turns, a, b, viewport) else {
				// 		return;
				// 	};
				// 	overlay_context.dashed_line(start, end, None, None, Some(4.), Some(4.), Some(0.5));
				// }
			}
			TightnessGizmoState::Inactive => {
				if let Some((_, outer_radius, _)) = extract_arc_spiral_parameters(layer, document).or(extract_log_spiral_parameters(layer, document)) {
					overlay_context.dashed_circle(DVec2::ZERO, outer_radius.max(5.), None, Some(COLOR_OVERLAY_RED), Some(8.), Some(4.), Some(0.5), Some(viewport));
				}
			}
		}
	}

	fn check_which_inter_segment(mouse_position: DVec2, outer_radius: f64, turns: f64, a: f64, transform: DAffine2) -> Option<(DVec2, DVec2)> {
		let b = calculate_b(a, turns, outer_radius, SpiralType::Archimedean);
		let center = DVec2::ZERO;
		let angle = mouse_position.angle_to(DVec2::X).rem_euclid(TAU);

		let viewport_mouse = transform.transform_point2(mouse_position);
		let viewport_center = transform.transform_point2(center);

		let max_theta = turns * TAU;
		let spiral_outer = archimedean_spiral_point(max_theta, a, b);
		let viewport_outer = transform.transform_point2(spiral_outer);

		if viewport_mouse.distance(viewport_center) > viewport_outer.distance(viewport_center) {
			return None;
		}

		let mouse_distance = viewport_mouse.distance(viewport_center);

		let mut segment_index = 0;

		// ---- First segment: from center to spiral at θ = angle
		{
			let start = viewport_center;
			let spiral_end = archimedean_spiral_point(angle, a, b);
			let end = transform.transform_point2(spiral_end);

			let r_end = end.distance(viewport_center);

			if mouse_distance <= r_end {
				return Some(Self::calculate_gizmo_line_points(viewport_center, end));
			}

			segment_index += 1;
		}

		// ---- Remaining segments: each full turn outward along the ray
		let mut base_theta = angle;

		while base_theta <= max_theta {
			let theta_start = base_theta;
			let theta_end = base_theta + TAU;

			if theta_end > max_theta {
				break;
			}

			let spiral_start = archimedean_spiral_point(theta_start, a, b);
			let spiral_end = archimedean_spiral_point(theta_end, a, b);

			let viewport_start = transform.transform_point2(spiral_start);
			let viewport_end = transform.transform_point2(spiral_end);

			let r_start = viewport_start.distance(viewport_center);
			let r_end = viewport_end.distance(viewport_center);

			if mouse_distance >= r_start && mouse_distance <= r_end {
				return Some(Self::calculate_gizmo_line_points(viewport_start, viewport_end));
			}

			base_theta += TAU;
			segment_index += 1;
		}

		None
	}

	// (start_point,end_point)
	fn calculate_gizmo_line_points(start_point: DVec2, end_point: DVec2) -> (DVec2, DVec2) {
		let length = start_point.distance(end_point);
		let factor = 0.25 * length;

		let direction = (end_point - start_point).normalize();

		let new_endpoint = end_point - direction * factor;
		let new_start_point = start_point + direction * factor;

		(new_start_point, new_endpoint)
	}

	pub fn update_number_of_turns(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let Some(layer) = self.layer else {
			return;
		};

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

		let (a, turns, net_radius) = match self.spiral_type {
			SpiralType::Archimedean => {
				let (a, outer_radius, turns) = extract_arc_spiral_parameters(layer, document).expect("Failed to get archimedean spiral inner radius");
				(a, turns, (outer_radius + delta).max(0.0))
			}
			SpiralType::Logarithmic => {
				let (a, outer_radius, turns) = extract_log_spiral_parameters(layer, document).expect("Failed to get logarithmic spiral inner radius");
				(a, turns, (outer_radius + delta).max(0.001))
			}
		};

		self.gizmo_line_points = Self::check_which_inter_segment(current_mouse_layer, net_radius, turns, a, viewport_transform);

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, SPIRAL_OUTER_RADIUS_INDEX),
			input: NodeInput::value(TaggedValue::F64(net_radius), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}
