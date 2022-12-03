use crate::messages::prelude::*;
use graph_craft::document::{value::TaggedValue, NodeId};

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
		node_id: Option<NodeId>,
		node_type: String,
		x: i32,
		y: i32,
	},
	DeleteNode {
		node_id: NodeId,
	},
	DeleteSelectedNodes,
	ExposeInput {
		node_id: NodeId,
		input_index: usize,
		new_exposed: bool,
	},
	MoveSelectedNodes {
		displacement_x: i32,
		displacement_y: i32,
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
