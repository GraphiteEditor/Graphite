use crate::messages::tool::common_functionality::shapes::shape_utility::{calculate_polygon_vertex_position, extract_polygon_parameters, inside_polygon, inside_star, polygon_outline, star_outline};
use std::{collections::VecDeque, f64::consts::TAU};

use crate::messages::{portfolio::document::utility_types::document_metadata::LayerNodeIdentifier, prelude::Responses};
use glam::{DAffine2, DVec2};
use graph_craft::document::{NodeInput, value::TaggedValue};

use crate::{
	consts::{GIZMO_HIDE_THRESHOLD, NUMBER_OF_POINTS_HANDLE_SPOKE_EXTENSION, NUMBER_OF_POINTS_HANDLE_SPOKE_LENGTH, POINT_RADIUS_HANDLE_SEGMENT_THRESHOLD},
	messages::{
		frontend::utility_types::MouseCursorIcon,
		message::Message,
		portfolio::document::{overlays::utility_types::OverlayContext, utility_types::network_interface::InputConnector},
		prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage},
		tool::common_functionality::{
			graph_modification_utils,
			shape_editor::ShapeState,
			shapes::shape_utility::{calculate_star_vertex_position, extract_star_parameters},
		},
	},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum NumberOfPointsHandleState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default)]
pub struct NumberOfPointsHandle {
	pub layer: Option<LayerNodeIdentifier>,
	pub initial_points: u32,
	pub handle_state: NumberOfPointsHandleState,
}

impl NumberOfPointsHandle {
	pub fn cleanup(&mut self) {
		self.handle_state = NumberOfPointsHandleState::Inactive;
		self.layer = None;
	}

	pub fn update_state(&mut self, state: NumberOfPointsHandleState) {
		self.handle_state = state;
	}

	pub fn is_hovering(&self) -> bool {
		self.handle_state == NumberOfPointsHandleState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.handle_state == NumberOfPointsHandleState::Dragging
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		match &self.handle_state {
			NumberOfPointsHandleState::Inactive => {
				if let Some((n, radius1, radius2)) = extract_star_parameters(Some(layer), document) {
					let viewport = document.metadata().transform_to_viewport(layer);
					let center = viewport.transform_point2(DVec2::ZERO);

					let point_on_max_radius = calculate_star_vertex_position(viewport, 0, n, radius1, radius2);

					if mouse_position.distance(center) < NUMBER_OF_POINTS_HANDLE_SPOKE_LENGTH && point_on_max_radius.distance(center) > GIZMO_HIDE_THRESHOLD {
						log::info!("should reach here");
						self.layer = Some(layer);
						self.initial_points = n;
						self.update_state(NumberOfPointsHandleState::Hover);
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
					}
				}

				if let Some((n, radius)) = extract_polygon_parameters(Some(layer), document) {
					let viewport = document.metadata().transform_to_viewport(layer);
					let center = viewport.transform_point2(DVec2::ZERO);

					let point_on_max_radius = calculate_polygon_vertex_position(viewport, 0, n, radius);

					if mouse_position.distance(center) < NUMBER_OF_POINTS_HANDLE_SPOKE_LENGTH && point_on_max_radius.distance(center) > GIZMO_HIDE_THRESHOLD {
						self.layer = Some(layer);
						self.initial_points = n;
						self.update_state(NumberOfPointsHandleState::Hover);
						responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
					}
				}
			}
			NumberOfPointsHandleState::Hover | NumberOfPointsHandleState::Dragging => {
				let Some(layer) = self.layer else {
					return;
				};

				let viewport = document.metadata().transform_to_viewport(layer);
				let center = viewport.transform_point2(DVec2::ZERO);

				if mouse_position.distance(center) > NUMBER_OF_POINTS_HANDLE_SPOKE_LENGTH && matches!(&self.handle_state, NumberOfPointsHandleState::Hover) {
					self.update_state(NumberOfPointsHandleState::Inactive);
					self.layer = None;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });

					return;
				}
			}
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, layer: Option<LayerNodeIdentifier>, shape_editor: &mut &mut ShapeState, mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		match &self.handle_state {
			NumberOfPointsHandleState::Inactive => {
				let Some(layer) = layer else { return };
				if let Some((n, radius1, radius2)) = extract_star_parameters(Some(layer), document) {
					let radius = radius1.max(radius2);
					let viewport = document.metadata().transform_to_viewport(layer);
					let center = viewport.transform_point2(DVec2::ZERO);

					if let Some(closest_segment) = shape_editor.upper_closest_segment(&document.network_interface, mouse_position, POINT_RADIUS_HANDLE_SEGMENT_THRESHOLD) {
						if closest_segment.layer() == layer {
							return;
						}
					}
					let point_on_max_radius = calculate_star_vertex_position(viewport, 0, n, radius1, radius2);

					if inside_star(viewport, n, radius1, radius2, mouse_position) && point_on_max_radius.distance(center) > GIZMO_HIDE_THRESHOLD {
						log::info!("reaching here");
						self.draw_spokes(center, viewport, n, radius, overlay_context);
						return;
					}
				}

				if let Some((n, radius)) = extract_polygon_parameters(Some(layer), document) {
					let viewport = document.metadata().transform_to_viewport(layer);
					let center = viewport.transform_point2(DVec2::ZERO);

					if let Some(closest_segment) = shape_editor.upper_closest_segment(&document.network_interface, mouse_position, POINT_RADIUS_HANDLE_SEGMENT_THRESHOLD) {
						if closest_segment.layer() == layer {
							return;
						}
					}
					let point_on_max_radius = calculate_polygon_vertex_position(viewport, 0, n, radius);

					if inside_polygon(viewport, n, radius, mouse_position) && point_on_max_radius.distance(center) > GIZMO_HIDE_THRESHOLD {
						self.draw_spokes(center, viewport, n, radius, overlay_context);
						return;
					}
				}
			}
			NumberOfPointsHandleState::Hover | NumberOfPointsHandleState::Dragging => {
				let Some(layer) = self.layer else {
					return;
				};

				let Some((n, radius)) = extract_star_parameters(Some(layer), document)
					.map(|(n, r1, r2)| (n, r1.max(r2)))
					.or_else(|| extract_polygon_parameters(Some(layer), document).map(|(n, r)| (n, r)))
				else {
					return;
				};

				let viewport = document.metadata().transform_to_viewport(layer);
				let center = viewport.transform_point2(DVec2::ZERO);

				star_outline(Some(layer), document, overlay_context);
				polygon_outline(Some(layer), document, overlay_context);

				self.draw_spokes(center, viewport, n, radius, overlay_context);
			}
		}
	}

	fn draw_spokes(&self, center: DVec2, viewport: DAffine2, n: u32, radius: f64, overlay_context: &mut OverlayContext) {
		for i in 0..n {
			let angle = ((i as f64) * TAU) / (n as f64);

			let point = viewport.transform_point2(DVec2 {
				x: radius * angle.sin(),
				y: -radius * angle.cos(),
			});

			let Some(direction) = (point - center).try_normalize() else {
				continue;
			};

			// If the user zooms out such that shape is very small hide the gizmo
			if point.distance(center) < GIZMO_HIDE_THRESHOLD {
				return;
			}

			let end_point = direction * NUMBER_OF_POINTS_HANDLE_SPOKE_LENGTH;
			if matches!(self.handle_state, NumberOfPointsHandleState::Hover | NumberOfPointsHandleState::Dragging) {
				overlay_context.line(center, end_point * NUMBER_OF_POINTS_HANDLE_SPOKE_EXTENSION + center, None, None);
			} else {
				overlay_context.line(center, end_point + center, None, None);
			}
		}
	}

	pub fn update_no_of_sides(&self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let delta = input.mouse.position - document.metadata().document_to_viewport.transform_point2(drag_start);
		let sign = (input.mouse.position.x - document.metadata().document_to_viewport.transform_point2(drag_start).x).signum();
		let net_delta = (delta.length() / 25.0).round() * sign;

		let Some(layer) = self.layer else {
			return;
		};

		let Some(node_id) = graph_modification_utils::get_star_id(layer, &document.network_interface).or(graph_modification_utils::get_polygon_id(layer, &document.network_interface)) else {
			return;
		};

		let new_point_count = ((self.initial_points as i32) + (net_delta as i32)).max(3);

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::U32(new_point_count as u32), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}
