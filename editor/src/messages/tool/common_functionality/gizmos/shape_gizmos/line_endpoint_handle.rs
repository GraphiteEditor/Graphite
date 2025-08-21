use crate::consts::BOUNDS_SELECT_THRESHOLD;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::Responses;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::tool::common_functionality::graph_modification_utils::{self};
use crate::messages::tool::common_functionality::shapes::LineEnd;
use crate::messages::tool::common_functionality::shapes::shape_utility::{extract_line_parameters, generate_line};
use crate::messages::tool::common_functionality::snapping::{SnapData, SnapManager};
use crate::messages::tool::tool_messages::tool_prelude::Key;
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum LineEndPointHandleState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineEndPointHandle {
	pub layer: Option<LayerNodeIdentifier>,
	pub handle_state: LineEndPointHandleState,
	end_point: LineEnd,
	drag_start: DVec2,
	drag_current: DVec2,
	angle: f64,
}

impl LineEndPointHandle {
	pub fn cleanup(&mut self) {
		self.handle_state = LineEndPointHandleState::Inactive;
		self.layer = None;
	}

	pub fn hovered(&self) -> bool {
		self.handle_state == LineEndPointHandleState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.handle_state == LineEndPointHandleState::Dragging
	}

	pub fn update_state(&mut self, state: LineEndPointHandleState) {
		self.handle_state = state;
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, mouse_position: DVec2) {
		match &self.handle_state {
			LineEndPointHandleState::Inactive => {
				if self.clicked_on_line_endpoints(layer, document, mouse_position) {
					self.drag_current = mouse_position;
					self.update_state(LineEndPointHandleState::Hover);
				}
			}
			_ => {}
		}
	}

	pub fn overlays(&self, selected_line_layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
		let Some(layer) = selected_line_layer.or(self.layer) else { return };
		let Some((start, end)) = extract_line_parameters(Some(layer), document) else { return };

		let [viewport_start, viewport_end] = [start, end].map(|point| document.metadata().transform_to_viewport(layer).transform_point2(point));
		overlay_context.line(viewport_start, viewport_end, None, None);
		if !start.abs_diff_eq(end, f64::EPSILON * 1000.) {
			overlay_context.square(viewport_start, Some(6.), None, None);
			overlay_context.square(viewport_end, Some(6.), None, None);
		}
	}

	pub fn update_endpoint_position(&mut self, document: &DocumentMessageHandler, snap: &mut SnapManager, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(layer) = self.layer else { return };

		self.drag_current = input.mouse.position;
		let ignore = [layer];
		let snap_data = SnapData::ignore(document, input, &ignore);
		let to_document = document.metadata().transform_to_document(layer);
		let (mut document_points, angle) = generate_line(
			self.angle,
			to_document.transform_point2(self.drag_start),
			input.mouse.position,
			snap,
			snap_data,
			input.keyboard.key(Key::Shift),
			input.keyboard.key(Key::Control),
			input.keyboard.key(Key::Alt),
		);

		self.angle = angle;

		if self.end_point == LineEnd::Start {
			document_points.swap(0, 1);
		}
		let Some(node_id) = graph_modification_utils::get_line_id(layer, &document.network_interface) else {
			return;
		};
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::DVec2(to_document.inverse().transform_point2(document_points[0])), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 2),
			input: NodeInput::value(TaggedValue::DVec2(to_document.inverse().transform_point2(document_points[1])), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn clicked_on_line_endpoints(&mut self, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, drag_start: DVec2) -> bool {
		let Some((document_start, document_end)) = extract_line_parameters(Some(layer), document) else {
			return false;
		};

		let transform = document.metadata().transform_to_viewport(layer);
		let viewport_x = transform.transform_vector2(DVec2::X).normalize_or_zero() * BOUNDS_SELECT_THRESHOLD;
		let viewport_y = transform.transform_vector2(DVec2::Y).normalize_or_zero() * BOUNDS_SELECT_THRESHOLD;
		let threshold_x = transform.inverse().transform_vector2(viewport_x).length();
		let threshold_y = transform.inverse().transform_vector2(viewport_y).length();

		let [start, end] = [document_start, document_end].map(|point| transform.transform_point2(point));

		let start_click = (drag_start.y - start.y).abs() < threshold_y && (drag_start.x - start.x).abs() < threshold_x;
		let end_click = (drag_start.y - end.y).abs() < threshold_y && (drag_start.x - end.x).abs() < threshold_x;

		if start_click || end_click {
			self.end_point = if end_click { LineEnd::End } else { LineEnd::Start };
			self.drag_start = if end_click { document_start } else { document_end };
			self.layer = Some(layer);
			return true;
		}
		false
	}
}
