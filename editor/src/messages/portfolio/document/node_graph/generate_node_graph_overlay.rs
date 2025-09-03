use graph_craft::{
	concrete,
	document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork, value::TaggedValue},
};
use graphene_std::{
	Context, graphic, memo,
	node_graph_overlay::{self, types::NodeGraphOverlayData},
	uuid::NodeId,
};

pub fn generate_node_graph_overlay(node_graph_overlay_data: NodeGraphOverlayData, opacity: f64) -> DocumentNode {
	// TODO: Implement as Network and implement finer grained caching for the background, nodes, and exports
	DocumentNode {
		inputs: vec![
			NodeInput::value(TaggedValue::None, true),
			NodeInput::value(TaggedValue::NodeGraphOverlayData(node_graph_overlay_data), true),
			NodeInput::value(TaggedValue::F64(opacity), true),
		],
		implementation: DocumentNodeImplementation::Network(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: vec![
				// Merge the overlay on top of the artwork
				(
					NodeId(0),
					DocumentNode {
						call_argument: concrete!(Context),
						inputs: vec![NodeInput::node(NodeId(2), 0), NodeInput::node(NodeId(1), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(graphic::extend::IDENTIFIER),
						..Default::default()
					},
				),
				//Wrap Artwork in a table
				(
					NodeId(1),
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(Context), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(graphic::wrap_graphic::IDENTIFIER),
						call_argument: concrete!(Context),
						..Default::default()
					},
				),
				// Cache the full node graph so its not rerendered when the artwork changes
				(
					NodeId(2),
					DocumentNode {
						call_argument: concrete!(Context),
						inputs: vec![NodeInput::node(NodeId(3), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(memo::memo::IDENTIFIER),
						..Default::default()
					},
				),
				// Merge the nodes on top of the dot grid background
				(
					NodeId(3),
					DocumentNode {
						call_argument: concrete!(Context),
						inputs: vec![NodeInput::node(NodeId(5), 0), NodeInput::node(NodeId(4), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(graphic::extend::IDENTIFIER),
						..Default::default()
					},
				),
				// Generate the dot grid background
				(
					NodeId(4),
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(Context), 2)],
						implementation: DocumentNodeImplementation::ProtoNode(node_graph_overlay::dot_grid_background::IDENTIFIER),
						call_argument: concrete!(Context),
						..Default::default()
					},
				),
				// Transform the nodes based on the Context
				(
					NodeId(5),
					DocumentNode {
						call_argument: concrete!(Context),
						inputs: vec![NodeInput::node(NodeId(6), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(node_graph_overlay::transform_nodes::IDENTIFIER),
						..Default::default()
					},
				),
				// Cache the nodes
				(
					NodeId(6),
					DocumentNode {
						call_argument: concrete!(Context),
						inputs: vec![NodeInput::node(NodeId(7), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(memo::memo::IDENTIFIER),
						..Default::default()
					},
				),
				// Create the nodes
				(
					NodeId(7),
					DocumentNode {
						call_argument: concrete!(Context),
						inputs: vec![NodeInput::network(concrete!(Context), 1)],
						implementation: DocumentNodeImplementation::ProtoNode(node_graph_overlay::generate_nodes::IDENTIFIER),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}),
		call_argument: concrete!(Context),
		..Default::default()
	}
}
