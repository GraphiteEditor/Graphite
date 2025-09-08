use graph_craft::{
	concrete,
	document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork, value::TaggedValue},
};
use graphene_std::{
	node_graph_overlay::{types::NodeGraphOverlayData, ui_context::UIContext},
	table::Table,
	uuid::NodeId,
};

/// https://excalidraw.com/#json=LgKS6I4lQvGPmke06ZJyp,D9aON9vVZJAjNnZWfwy_SQ
pub fn generate_node_graph_overlay(node_graph_overlay_data: NodeGraphOverlayData, opacity: f64) -> DocumentNode {
	let generate_nodes_id = NodeId::new();
	let cache_nodes_id = NodeId::new();
	let transform_nodes_id = NodeId::new();

	let generate_node_graph_bg = NodeId::new();
	let cache_node_graph_bg = NodeId::new();

	let merge_nodes_and_bg_id = NodeId::new();
	let render_overlay_id = NodeId::new();
	let send_overlay_id = NodeId::new();
	let cache_output_id = NodeId::new();
	// TODO: Replace with new cache node
	let identity_implementation = DocumentNodeImplementation::ProtoNode(graphene_std::ops::identity::IDENTIFIER);
	let memo_implementation = DocumentNodeImplementation::ProtoNode(graphene_std::memo::memo::IDENTIFIER);

	DocumentNode {
		inputs: vec![
			NodeInput::value(TaggedValue::Vector(Table::new()), true),
			NodeInput::value(TaggedValue::NodeGraphOverlayData(node_graph_overlay_data), true),
			NodeInput::value(TaggedValue::F64(opacity), true),
		],

		implementation: DocumentNodeImplementation::Network(NodeNetwork {
			exports: vec![NodeInput::node(cache_output_id, 0)],
			nodes: vec![
				// Create the nodes
				(
					generate_nodes_id,
					DocumentNode {
						call_argument: concrete!(UIContext),
						inputs: vec![NodeInput::network(concrete!(UIContext), 1), NodeInput::scope("font-cache")],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::node_graph_overlay::GenerateNodesNode".into()),
						..Default::default()
					},
				),
				// Cache the nodes
				(
					cache_nodes_id,
					DocumentNode {
						call_argument: concrete!(UIContext),
						inputs: vec![NodeInput::node(generate_nodes_id, 0)],
						implementation: identity_implementation.clone(),
						..Default::default()
					},
				),
				// Transform the nodes based on the Context
				(
					transform_nodes_id,
					DocumentNode {
						call_argument: concrete!(UIContext),
						inputs: vec![NodeInput::node(cache_nodes_id, 0)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::node_graph_overlay::TransformNodesNode".into()),
						..Default::default()
					},
				),
				// Generate the dot grid background
				(
					generate_node_graph_bg,
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(UIContext), 2)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::node_graph_overlay::DotGridBackgroundNode".into()),
						call_argument: concrete!(UIContext),
						..Default::default()
					},
				),
				// Cache the dot grid background
				(
					cache_node_graph_bg,
					DocumentNode {
						call_argument: concrete!(UIContext),
						inputs: vec![NodeInput::node(generate_node_graph_bg, 0)],
						implementation: identity_implementation.clone(),
						..Default::default()
					},
				),
				// Merge the nodes on top of the dot grid background
				(
					merge_nodes_and_bg_id,
					DocumentNode {
						call_argument: concrete!(UIContext),
						inputs: vec![NodeInput::node(transform_nodes_id, 0), NodeInput::node(cache_node_graph_bg, 0)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::node_graph_overlay::NodeGraphUiExtendNode".into()),
						..Default::default()
					},
				),
				// Render the node graph UI graphic to an SVG
				(
					render_overlay_id,
					DocumentNode {
						call_argument: concrete!(UIContext),
						inputs: vec![NodeInput::node(merge_nodes_and_bg_id, 0)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_std::wasm_application_io::RenderNodeGraphUiNode".into()),
						..Default::default()
					},
				),
				// Send the overlay to the frontend
				(
					send_overlay_id,
					DocumentNode {
						call_argument: concrete!(UIContext),
						inputs: vec![NodeInput::node(render_overlay_id, 0)],
						implementation: DocumentNodeImplementation::ProtoNode("graphene_core::node_graph_overlay::SendRenderNode".into()),
						..Default::default()
					},
				),
				// Cache the full node graph so its not rerendered when nothing changes
				(
					cache_output_id,
					DocumentNode {
						call_argument: concrete!(UIContext),
						inputs: vec![NodeInput::node(send_overlay_id, 0)],
						implementation: memo_implementation.clone(),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}),
		call_argument: concrete!(UIContext),
		..Default::default()
	}
}
