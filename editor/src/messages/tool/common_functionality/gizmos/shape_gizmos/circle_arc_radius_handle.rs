use crate::consts::GIZMO_HIDE_THRESHOLD;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::prelude::{FrontendMessage, Responses};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, get_arc_id, get_stroke_width};
use crate::messages::tool::common_functionality::shapes::shape_utility::{extract_arc_parameters, extract_circle_radius};
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;
use std::f64::consts::FRAC_PI_2;

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

	pub fn is_dragging(&self) -> bool {
		self.handle_state == RadiusHandleState::Dragging
	}

	pub fn update_state(&mut self, state: RadiusHandleState) {
		self.handle_state = state;
	}

	pub fn check_if_inside_dash_lines(angle: f64, mouse_position: DVec2, viewport: DAffine2, radius: f64, document: &DocumentMessageHandler, layer: LayerNodeIdentifier) -> bool {
		let center = viewport.transform_point2(DVec2::ZERO);
		if let Some(stroke_width) = get_stroke_width(layer, &document.network_interface) {
			let circle_point = calculate_circle_point_position(angle, radius.abs());
			let Some(direction) = circle_point.try_normalize() else { return false };
			let mouse_distance = mouse_position.distance(center);

			let spacing = Self::calculate_extra_spacing(viewport, radius, center, stroke_width, 15.);

			let inner_point = viewport.transform_point2(circle_point - direction * spacing).distance(center);
			let outer_point = viewport.transform_point2(circle_point + direction * spacing).distance(center);

			mouse_distance >= inner_point && mouse_distance <= outer_point
		} else {
			let point_position = viewport.transform_point2(calculate_circle_point_position(angle, radius.abs()));
			mouse_position.distance(center) <= point_position.distance(center)
		}
	}

	fn calculate_extra_spacing(viewport: DAffine2, radius: f64, viewport_center: DVec2, stroke_width: f64, threshold: f64) -> f64 {
		let start_point = viewport.transform_point2(calculate_circle_point_position(0., radius)).distance(viewport_center);
		let end_point = viewport.transform_point2(calculate_circle_point_position(FRAC_PI_2, radius)).distance(viewport_center);
		let min_radius = start_point.min(end_point);
		let extra_spacing = if min_radius < threshold { 10. * (min_radius / threshold) } else { 10. };

		stroke_width + extra_spacing
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		match &self.handle_state {
			RadiusHandleState::Inactive => {
				let Some(radius) = extract_circle_radius(layer, document).or(extract_arc_parameters(Some(layer), document).map(|(r, _, _, _)| r)) else {
					return;
				};
				let viewport = document.metadata().transform_to_viewport(layer);
				let angle = viewport.inverse().transform_point2(mouse_position).angle_to(DVec2::X);
				let point_position = viewport.transform_point2(calculate_circle_point_position(angle, radius.abs()));
				let center = viewport.transform_point2(DVec2::ZERO);

				if point_position.distance(center) < GIZMO_HIDE_THRESHOLD {
					return;
				}

				if Self::check_if_inside_dash_lines(angle, mouse_position, viewport, radius.abs(), document, layer) {
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
				let Some(radius) = extract_circle_radius(layer, document).or(extract_arc_parameters(Some(layer), document).map(|(r, _, _, _)| r)) else {
					return;
				};
				let viewport = document.metadata().transform_to_viewport(layer);
				let center = viewport.transform_point2(DVec2::ZERO);

				let x_point = viewport.transform_point2(calculate_circle_point_position(0., radius));
				let y_point = viewport.transform_point2(calculate_circle_point_position(FRAC_PI_2, radius));

				let direction_x = viewport.transform_vector2(DVec2::X);
				let direction_y = viewport.transform_vector2(-DVec2::Y);

				if let Some(stroke_width) = get_stroke_width(layer, &document.network_interface) {
					let spacing = Self::calculate_extra_spacing(viewport, radius, center, stroke_width, 15.);
					let smaller_radius_x = (x_point - direction_x * spacing).distance(center);
					let smaller_radius_y = (y_point - direction_y * spacing).distance(center);

					let larger_radius_x = (x_point + direction_x * spacing).distance(center);
					let larger_radius_y = (y_point + direction_y * spacing).distance(center);

					overlay_context.dashed_ellipse(center, smaller_radius_x, smaller_radius_y, None, None, None, None, None, None, Some(4.), Some(4.), Some(0.5));
					overlay_context.dashed_ellipse(center, larger_radius_x, larger_radius_y, None, None, None, None, None, None, Some(4.), Some(4.), Some(0.5));

					return;
				}

				let radius_x = x_point.distance(center);
				let radius_y = y_point.distance(center);
				overlay_context.dashed_ellipse(center, radius_x, radius_y, None, None, None, None, None, None, Some(4.), Some(4.), Some(0.5));
			}
		}
	}

	pub fn update_inner_radius(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let Some(layer) = self.layer else { return };
		let Some(node_id) = graph_modification_utils::get_circle_id(layer, &document.network_interface).or(get_arc_id(layer, &document.network_interface)) else {
			return;
		};
		let Some(current_radius) = extract_circle_radius(layer, document).or(extract_arc_parameters(Some(layer), document).map(|(r, _, _, _)| r)) else {
			return;
		};

		let viewport_transform = document.network_interface.document_metadata().transform_to_viewport(layer);
		let center = viewport_transform.transform_point2(DVec2::ZERO);

		let delta_vector = viewport_transform.inverse().transform_point2(input.mouse.position) - viewport_transform.inverse().transform_point2(self.previous_mouse_position);
		let radius = drag_start - center;
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

fn calculate_circle_point_position(theta: f64, radius: f64) -> DVec2 {
	DVec2::new(radius * theta.cos(), -radius * theta.sin())
}
