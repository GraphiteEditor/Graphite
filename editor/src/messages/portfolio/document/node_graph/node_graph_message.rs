use crate::messages::prelude::*;
use graph_craft::document::{value::TaggedValue, NodeId};
use graph_craft::proto::NodeIdentifier;

#[remain::sorted]
#[impl_message(Message, DocumentMessage, NodeGraph)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum NodeGraphMessage {
	// Messages
	CloseNodeGraph,
	ConnectNodesByLink {
		output_node: u64,
		input_node: u64,
		input_node_connector_index: usize,
	},
	CreateNode {
		// Having the caller generate the id means that we don't have to return it. This can be a random u64.
		node_id: NodeId,
		// I don't really know what this is for (perhaps a user identifiable name).
		name: String,
		// The node identifier must mach that found in `node-graph/graph-craft/src/node_registry.rs` e.g. "graphene_core::raster::GrayscaleNode
		identifier: NodeIdentifier,
		num_inputs: u32,
	},
	DeleteNode {
		node_id: NodeId,
	},
	OpenNodeGraph {
		layer_path: Vec<graphene::LayerId>,
	},
	SelectNodes {
		nodes: Vec<NodeId>,
	},
	SetInputValue {
		node: NodeId,
		input_index: usize,
		value: TaggedValue,
	},
}
