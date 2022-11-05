use crate::messages::prelude::*;
use graph_craft::document::{DocumentNode, NodeInput};
use graphene::{document::Document, layers::layer_info::LayerDataType};

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeGraphMessageHandler {
	pub layer_path: Option<Vec<graphene::LayerId>>,
}

impl NodeGraphMessageHandler {
	/// Get the active graph_craft NodeNetwork struct
	fn get_active_network<'a>(&self, document: &'a mut Document) -> Option<&'a mut graph_craft::document::NodeNetwork> {
		self.layer_path.as_ref().and_then(|path| document.layer_mut(path).ok()).and_then(|layer| match &mut layer.data {
			LayerDataType::NodeGraphFrame(n) => Some(&mut n.network),
			_ => None,
		})
	}
}

impl MessageHandler<NodeGraphMessage, (&mut Document, &InputPreprocessorMessageHandler)> for NodeGraphMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: NodeGraphMessage, (document, _ipp): (&mut Document, &InputPreprocessorMessageHandler), _responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
			NodeGraphMessage::AddLink { from, to, to_index } => {
				if let Some(network) = self.get_active_network(document) {
					if let Some(to) = network.nodes.get_mut(&to) {
						// Extend number of inputs if not already large enough
						if to_index >= to.inputs.len() {
							to.inputs.extend(((to.inputs.len() - 1)..to_index).map(|_| NodeInput::Network));
						}
						to.inputs[to_index] = NodeInput::Node(from);
					}
				}
			}
			NodeGraphMessage::CloseNodeGraph => {
				if let Some(_old_layer_path) = self.layer_path.take() {
					info!("Closing node graph");
					// TODO: Close UI and clean up old node graph
				}
			}
			NodeGraphMessage::CreateNode { node_id, name, identifier } => {
				if let Some(network) = self.get_active_network(document) {
					network.nodes.insert(
						node_id,
						DocumentNode {
							name,
							inputs: Vec::new(),
							// TODO: Allow inserting nodes that contain other nodes.
							implementation: graph_craft::document::DocumentNodeImplementation::Unresolved(identifier),
						},
					);
				}
			}
			NodeGraphMessage::DeleteNode { node_id } => {
				if let Some(network) = self.get_active_network(document) {
					network.nodes.remove(&node_id);
					// TODO: Update UI if it is not already updated.
				}
			}
			NodeGraphMessage::OpenNodeGraph { layer_path } => {
				if let Some(_old_layer_path) = self.layer_path.replace(layer_path) {
					// TODO: Necessary cleanup of old node graph
				}

				if let Some(network) = self.get_active_network(document) {
					info!("Opening node graph with nodes {:?}", network.nodes);
					for (_id, _node) in &network.nodes {
						// TODO: Populate initial frontend with nodes.
					}
				}
			}
			NodeGraphMessage::SetInputValue { node, input_index, value } => {
				if let Some(network) = self.get_active_network(document) {
					if let Some(node) = network.nodes.get_mut(&node) {
						// Extend number of inputs if not already large enough
						if input_index >= node.inputs.len() {
							node.inputs.extend(((node.inputs.len() - 1)..input_index).map(|_| NodeInput::Network));
						}
						node.inputs[input_index] = NodeInput::Value(value);
					}
				}
			}
		}
	}

	advertise_actions!(NodeGraphMessageDiscriminant; DeleteNode,);
}
