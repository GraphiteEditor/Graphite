use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};

use crate::messages::tool::common_functionality::gizmos::shape_gizmos::circle_radius_handle::{RadiusHandle, RadiusHandleState};
use crate::messages::tool::common_functionality::graph_modification_utils;

use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeGizmoHandler;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;

#[derive(Clone, Debug, Default)]
pub struct CircleGizmoHandler {
	circle_radius_handle: RadiusHandle,
}

impl ShapeGizmoHandler for CircleGizmoHandler {
	fn is_any_gizmo_hovered(&self) -> bool {
		self.circle_radius_handle.hovered()
	}

	fn handle_state(&mut self, selected_circle_layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.circle_radius_handle.handle_actions(selected_circle_layer, document, mouse_position, responses);
	}

	fn handle_click(&mut self) {
		if self.circle_radius_handle.hovered() {
			self.circle_radius_handle.update_state(RadiusHandleState::Dragging);
		}
	}

	fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		if self.circle_radius_handle.is_dragging_or_snapped() {
			self.circle_radius_handle.update_inner_radius(document, input, responses, drag_start);
		}
	}

	fn overlays(
		&self,
		document: &DocumentMessageHandler,
		_selected_circle_layer: Option<LayerNodeIdentifier>,
		_input: &InputPreprocessorMessageHandler,
		_shape_editor: &mut &mut ShapeState,
		_mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		self.circle_radius_handle.overlays(document, overlay_context);
	}

	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		_input: &InputPreprocessorMessageHandler,
		_shape_editor: &mut &mut ShapeState,
		_mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if self.circle_radius_handle.is_dragging_or_snapped() {
			self.circle_radius_handle.overlays(document, overlay_context);
		}
	}

	fn cleanup(&mut self) {
		self.circle_radius_handle.cleanup();
	}
}

#[derive(Default)]
pub struct Circle;

impl Circle {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_document_node_type("Circle").expect("Circle can't be found");
		node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(0.), false))])
	}

	pub fn update_shape(document: &DocumentMessageHandler, ipp: &InputPreprocessorMessageHandler, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		let Some(node_id) = graph_modification_utils::get_circle_id(layer, &document.network_interface) else {
			return;
		};

		let viewport = document.metadata().transform_to_viewport(layer);
		let center = viewport.transform_point2(DVec2::ZERO);

		let radius = ipp.mouse.position.distance(center);

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::F64(radius), false),
		});

		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_translation(center),
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});
	}
}
