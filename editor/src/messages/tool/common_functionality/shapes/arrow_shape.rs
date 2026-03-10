use super::line_shape::generate_line;
use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_document_node_type};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::snapping::SnapData;
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Arrow;

impl Arrow {
	pub fn create_node(shaft_width: f64, head_width: f64, head_length: f64) -> NodeTemplate {
		let identifier = DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::arrow::IDENTIFIER);
		let node_type = resolve_document_node_type(&identifier).expect("Arrow node can't be found");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false)), // arrow_to
			Some(NodeInput::value(TaggedValue::F64(shaft_width), false)),   // shaft_width
			Some(NodeInput::value(TaggedValue::F64(head_width), false)),    // head_width
			Some(NodeInput::value(TaggedValue::F64(head_length), false)),   // head_length
		])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		viewport: &ViewportMessageHandler,
		layer: LayerNodeIdentifier,
		tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let [center, snap_angle, lock_angle] = modifier;

		tool_data.line_data.drag_current = input.mouse.position;

		let keyboard = &input.keyboard;
		let ignore = [layer];
		let snap_data = SnapData::ignore(document, input, viewport, &ignore);
		let document_points = generate_line(tool_data, snap_data, keyboard.key(lock_angle), keyboard.key(snap_angle), keyboard.key(center));

		let arrow_to = document_points[1] - document_points[0];

		if arrow_to.length() < 1e-6 {
			return;
		}

		let Some(node_id) = graph_modification_utils::get_arrow_id(layer, &document.network_interface) else {
			return;
		};

		let document_to_viewport = document.metadata().document_to_viewport;

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::DVec2(arrow_to), false),
		});
		let downstream = document.metadata().downstream_transform_to_viewport(layer);
		let scope = downstream.inverse() * document_to_viewport;
		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_translation(document_points[0]),
			transform_in: TransformIn::Scope { scope },
			skip_rerender: false,
		});

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn overlays(_document: &DocumentMessageHandler, _tool_data: &ShapeToolData, _overlay_context: &mut OverlayContext) {}
}
