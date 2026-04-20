use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Rectangle;

impl Rectangle {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER).expect("Rectangle node can't be found");
		node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(1.), false)), Some(NodeInput::value(TaggedValue::F64(1.), false))])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		viewport: &ViewportMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let [center, lock_ratio, _] = modifier;

		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, viewport, center, lock_ratio) {
			let Some(node_id) = graph_modification_utils::get_rectangle_id(layer, &document.network_interface) else {
				return;
			};

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 1),
				input: NodeInput::value(TaggedValue::F64((start.x - end.x).abs()), false),
			});
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 2),
				input: NodeInput::value(TaggedValue::F64((start.y - end.y).abs()), false),
			});
			responses.add(GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_translation(start.midpoint(end)),
				transform_in: TransformIn::Viewport,
				skip_rerender: false,
			});
		}
	}
}
#[cfg(test)]
mod test_polygon {
	use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
	use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
	use crate::test_utils::test_prelude::*;
	use graph_craft::document::value::TaggedValue;
	struct ResolvedPolygon {
		vertices: u32,
		radius: f64,
	}
	async fn get_polygons(editor: &mut EditorTestUtils) -> Vec<ResolvedPolygon> {
		let document = editor.active_document();
		let network_interface = &document.network_interface;
		document
			.metadata()
			.all_layers()
			.filter_map(|layer| {
				let node_inputs = NodeGraphLayer::new(layer, network_interface)
					.find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER))?;
				let Some(&TaggedValue::U32(vertices)) = node_inputs[1].as_value() else {
					return None;
				};
				let Some(&TaggedValue::F64(radius)) = node_inputs[2].as_value() else {
					return None;
				};
				Some(ResolvedPolygon { vertices, radius })
			})
			.collect()
	}
	#[tokio::test]
	async fn polygon_draw_simple() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Shape, 0., 0., 60., 60., ModifierKeys::empty()).await;
		assert_eq!(editor.active_document().metadata().all_layers().count(), 1);
		let polys = get_polygons(&mut editor).await;
		assert_eq!(polys.len(), 1);
		// Default vertices count (6 for Shape tool default)
		assert!(polys[0].vertices >= 3, "polygon should have at least 3 vertices");
		// For a 60×60 drag both dimensions equal → radius = smaller_dim / 2 = 30
		assert!((polys[0].radius - 30.).abs() < 1e-10);
	}
	#[tokio::test]
	async fn polygon_draw_non_square() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		// Drag a non-square region: 40 wide, 60 tall → smaller dimension is 40 → radius = 20
		editor.drag_tool(ToolType::Shape, 0., 0., 40., 60., ModifierKeys::empty()).await;
		let polys = get_polygons(&mut editor).await;
		assert_eq!(polys.len(), 1);
		assert!((polys[0].radius - 20.).abs() < 1e-10);
	}
	#[tokio::test]
	async fn polygon_cancel() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool_cancel_rmb(ToolType::Shape).await;
		let polys = get_polygons(&mut editor).await;
		assert_eq!(polys.len(), 0);
	}
}
