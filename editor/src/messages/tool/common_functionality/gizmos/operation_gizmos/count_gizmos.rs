use crate::consts::{NUMBER_OF_POINTS_DIAL_SPOKE_EXTENSION, NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::NodeGraphMessage;
use crate::messages::prelude::Responses;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shapes::shape_utility::{GizmoContext, extract_circular_repeat_parameters};
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, TAU};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RepeatCountDialState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default)]
pub struct RepeatCountDial {
	pub layer: Option<LayerNodeIdentifier>,
	pub initial_points: u32,
	pub handle_state: RepeatCountDialState,

	// store the other layers whose gizmo is not hovered but still has repeat node in them
	selected_layers: HashMap<LayerNodeIdentifier, u32>,
}

impl RepeatCountDial {
	pub fn cleanup(&mut self) {
		self.handle_state = RepeatCountDialState::Inactive;
		self.layer = None;
		self.selected_layers.clear();
	}

	pub fn update_state(&mut self, state: RepeatCountDialState) {
		self.handle_state = state;
	}

	pub fn is_hovering(&self) -> bool {
		self.handle_state == RepeatCountDialState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.handle_state == RepeatCountDialState::Dragging
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, ctx: &mut GizmoContext) {
		let GizmoContext { document, .. } = ctx;

		match &self.handle_state {
			RepeatCountDialState::Inactive => {
				let Some((_, radius, count)) = extract_circular_repeat_parameters(Some(layer), document) else {
					return;
				};
				let viewport = document.metadata().transform_to_viewport(layer);

				let center = viewport.transform_point2(DVec2::ZERO);
				let offset_vector = document.metadata().downstream_transform_to_viewport(layer).transform_vector2(DVec2::NEG_Y * radius);

				let repeat_center = center + offset_vector;

				if repeat_center.distance(mouse_position) < NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH {
					self.layer = Some(layer);
					self.update_state(RepeatCountDialState::Hover);
					self.initial_points = count;
				}

				self.selected_layers.insert(layer, count);
			}
			RepeatCountDialState::Hover | RepeatCountDialState::Dragging => {
				// Even though we the gizmo is in hovered state store the other layers
				let Some((_, _, count)) = extract_circular_repeat_parameters(Some(layer), document) else {
					return;
				};
				self.selected_layers.insert(layer, count);
			}
		}
	}

	pub fn overlays(&self, layer: Option<LayerNodeIdentifier>, mouse_position: DVec2, ctx: &mut GizmoContext, overlay_context: &mut OverlayContext) {
		let GizmoContext { document, .. } = ctx;

		match &self.handle_state {
			RepeatCountDialState::Inactive => {
				let Some(layer) = layer else { return };
				let Some((angle, radius, count)) = extract_circular_repeat_parameters(Some(layer), document) else {
					return;
				};
				let viewport = document.metadata().transform_to_viewport(layer);

				let center = viewport.transform_point2(DVec2::ZERO);
				let offset_vector = document.metadata().downstream_transform_to_viewport(layer).transform_vector2(DVec2::NEG_Y * radius);
				let repeat_center = center + offset_vector;

				if repeat_center.distance(mouse_position) < radius.abs() {
					self.draw_spokes(repeat_center, document.metadata().downstream_transform_to_viewport(layer), count, angle.to_radians(), overlay_context);
				}
			}
			RepeatCountDialState::Hover | RepeatCountDialState::Dragging => {
				let Some(layer) = self.layer else { return };
				let Some((angle, radius, count)) = extract_circular_repeat_parameters(Some(layer), document) else {
					return;
				};
				let viewport = document.metadata().transform_to_viewport(layer);

				let center = viewport.transform_point2(DVec2::ZERO);
				let offset_vector = document.metadata().downstream_transform_to_viewport(layer).transform_vector2(DVec2::NEG_Y * radius);

				let repeat_center = center + offset_vector;

				self.draw_spokes(repeat_center, document.metadata().downstream_transform_to_viewport(layer), count, angle.to_radians(), overlay_context);
			}
		}
	}

	fn draw_spokes(&self, center: DVec2, viewport: DAffine2, count: u32, angle: f64, overlay_context: &mut OverlayContext) {
		for i in 0..count {
			let angle = ((i as f64) * TAU) / (count as f64) + angle + FRAC_PI_2;

			let direction_vector = viewport.transform_vector2(DVec2 { x: angle.cos(), y: -angle.sin() });

			let end_point = direction_vector * 20.;
			if matches!(self.handle_state, RepeatCountDialState::Hover | RepeatCountDialState::Dragging) {
				overlay_context.line(center, end_point * NUMBER_OF_POINTS_DIAL_SPOKE_EXTENSION + center, None, None);
			} else {
				overlay_context.line(center, end_point + center, None, None);
			}
		}
	}

	pub fn update_number_of_sides(&self, drag_start: DVec2, ctx: &mut GizmoContext) {
		let GizmoContext { document, input, responses, .. } = ctx;

		let delta = input.mouse.position - drag_start;
		let sign = (input.mouse.position.x - drag_start.x).signum();
		let net_delta = (delta.length() / 25.).round() * sign;

		for (layer, count) in &self.selected_layers {
			let Some(node_id) = graph_modification_utils::get_circular_repeat(*layer, &document.network_interface) else {
				return;
			};
			let new_point_count = ((*count as i32) + (net_delta as i32)).max(1);

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 3),
				input: NodeInput::value(TaggedValue::U32(new_point_count as u32), false),
			});
		}
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}
