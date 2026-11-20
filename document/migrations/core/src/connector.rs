use crate::Declaration;
use document_graph_storage::{NetworkId, NodeId, NodeInput};

/// One output of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutputConnector {
	pub node: NodeId,
	pub output: u32,
}

impl From<OutputConnector> for NodeInput {
	fn from(connector: OutputConnector) -> Self {
		NodeInput::Node {
			id: connector.node,
			index: connector.output,
		}
	}
}

/// A slot that reads an output: a node's input, or an export of a network.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InputConnector {
	Node { node: NodeId, input: u32 },
	Export { network: NetworkId, index: u32 },
}

/// A node to add, with every input given explicitly.
#[derive(Clone, Debug, PartialEq)]
pub struct NewNode {
	pub declaration: Declaration,
	pub inputs: Vec<NodeInput>,
}
