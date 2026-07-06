use super::shape_utility::{ShapeToolModifierKey, update_radius_sign};
use super::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_document_node_type};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, NodeGraphLayer};
use crate::messages::tool::tool_messages::shape_tool::ShapeOptionsUpdate;
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Polygon;

impl Polygon {
	pub fn create_node(vertices: u32) -> NodeTemplate {
		let identifier = DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER);
		let node_type = resolve_document_node_type(&identifier).expect("Regular Polygon can't be found");
		node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::U32(vertices), false)), Some(NodeInput::value(TaggedValue::F64(0.5), false))])
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
			// TODO: We need to determine how to allow the polygon node to make irregular shapes
			update_radius_sign(end, start, layer, document, responses);

			let dimensions = (start - end).abs();

			// We keep the smaller dimension's scale at 1 and scale the other dimension accordingly
			let mut aspect = DVec2::ONE;
			let radius;
			if dimensions.x > dimensions.y {
				aspect.x = dimensions.x / dimensions.y;
				radius = dimensions.y / 2.;
			} else {
				aspect.y = dimensions.y / dimensions.x;
				radius = dimensions.x / 2.;
			}

			let radius = radius / viewport_zoom(document);

			let Some(node_id) = graph_modification_utils::get_polygon_id(layer, &document.network_interface) else {
				return;
			};

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 2),
				input: NodeInput::value(TaggedValue::F64(radius), false),
			});

			responses.add(window_aligned_transform_set(document, layer, start.midpoint(end), aspect));
		}
	}

	/// Updates the number of sides of a polygon or star node and syncs the Shape tool UI widget accordingly.
	/// Increases or decreases the side count based on user input, clamped to a minimum of 3.
	pub fn decrease_or_increase_sides(decrease: bool, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(node_id) = graph_modification_utils::get_polygon_id(layer, &document.network_interface).or(graph_modification_utils::get_star_id(layer, &document.network_interface)) else {
			return;
		};

		let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface)
			.find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER))
			.or(NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::star::IDENTIFIER)))
		else {
			return;
		};

		let Some(&TaggedValue::U32(n)) = node_inputs.get(1).unwrap().as_value() else {
			return;
		};

		let new_dimension = if decrease { (n - 1).max(3) } else { n + 1 };

		responses.add(ShapeToolMessage::UpdateOptions {
			options: ShapeOptionsUpdate::Vertices(new_dimension),
		});

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::U32(new_dimension), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}
