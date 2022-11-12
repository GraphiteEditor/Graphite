use crate::messages::layout::utility_types::layout_widget::{LayoutGroup, Widget, WidgetHolder};
use crate::messages::layout::utility_types::widgets::label_widgets::TextLabel;
use crate::messages::prelude::*;
use graph_craft::document::{DocumentNode, NodeInput};
use graphene::document::Document;
use graphene::layers::layer_info::LayerDataType;
use graphene::layers::nodegraph_layer::NodeGraphFrameLayer;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrontendNode {
	pub id: graph_craft::document::NodeId,
	#[serde(rename = "displayName")]
	pub display_name: String,
}

// (link_start, link_end, link_end_input_index)
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrontendNodeLink {
	#[serde(rename = "linkStart")]
	pub link_start: u64,
	#[serde(rename = "linkEnd")]
	pub link_end: u64,
	#[serde(rename = "linkEndInputIndex")]
	pub link_end_input_index: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeGraphMessageHandler {
	pub layer_path: Option<Vec<graphene::LayerId>>,
	pub selected_nodes: Vec<graph_craft::document::NodeId>,
}

impl NodeGraphMessageHandler {
	/// Get the active graph_craft NodeNetwork struct
	fn get_active_network_mut<'a>(&self, document: &'a mut Document) -> Option<&'a mut graph_craft::document::NodeNetwork> {
		self.layer_path.as_ref().and_then(|path| document.layer_mut(path).ok()).and_then(|layer| match &mut layer.data {
			LayerDataType::NodeGraphFrame(n) => Some(&mut n.network),
			_ => None,
		})
	}

	pub fn collate_properties(&self, node_graph_frame: &NodeGraphFrameLayer) -> Vec<LayoutGroup> {
		let network = &node_graph_frame.network;
		let mut section = Vec::new();
		for node_id in &self.selected_nodes {
			let Some(document_node) = network.nodes.get(node_id) else {
				continue;
			};
			let name = format!("Node {} Properties", document_node.name);
			use graph_craft::document::DocumentNodeImplementation;
			let layout = match &document_node.implementation {
				DocumentNodeImplementation::Network(_) => vec![LayoutGroup::Row {
					widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Cannot currently display properties for network".to_string(),
						..Default::default()
					}))],
				}],
				DocumentNodeImplementation::Unresolved(identifier) => match identifier.name.as_ref() {
					"bob" => vec![],
					unknown => {
						vec![
							LayoutGroup::Row {
								widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
									value: format!("TODO: {} properties", unknown),
									..Default::default()
								}))],
							},
							LayoutGroup::Row {
								widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
									value: "Add in editor/src/messages/portfolio/document/node_graph/node_graph_message_handler.rs".to_string(),
									..Default::default()
								}))],
							},
						]
					}
				},
			};
			section.push(LayoutGroup::Section { name, layout });
		}

		section
	}
}

impl MessageHandler<NodeGraphMessage, (&mut Document, &InputPreprocessorMessageHandler)> for NodeGraphMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: NodeGraphMessage, (document, _ipp): (&mut Document, &InputPreprocessorMessageHandler), responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
			NodeGraphMessage::AddLink { from, to, to_index } => {
				if let Some(network) = self.get_active_network_mut(document) {
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
					responses.push_back(FrontendMessage::UpdateNodeGraphVisibility { visible: false }.into());
					responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
					// TODO: Close UI and clean up old node graph
				}
			}
			NodeGraphMessage::ConnectNodesByLink {
				output_node,
				input_node,
				input_node_connector_index,
			} => {
				log::debug!("Connect primary output from node {output_node} to input of index {input_node_connector_index} on node {input_node}.");
			}
			NodeGraphMessage::CreateNode { node_id, name, identifier } => {
				if let Some(network) = self.get_active_network_mut(document) {
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
				if let Some(network) = self.get_active_network_mut(document) {
					network.nodes.remove(&node_id);
					// TODO: Update UI if it is not already updated.
				}
			}
			NodeGraphMessage::OpenNodeGraph { layer_path } => {
				if let Some(_old_layer_path) = self.layer_path.replace(layer_path) {
					// TODO: Necessary cleanup of old node graph
				}

				if let Some(network) = self.get_active_network_mut(document) {
					self.selected_nodes.clear();
					responses.push_back(FrontendMessage::UpdateNodeGraphVisibility { visible: true }.into());
					responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
					info!("Opening node graph with nodes {:?}", network.nodes);

					// List of links in format (link_start, link_end, link_end_input_index)
					let links = network
						.nodes
						.iter()
						.flat_map(|(link_end, node)| node.inputs.iter().enumerate().map(move |(index, input)| (input, link_end, index)))
						.filter_map(|(input, &link_end, link_end_input_index)| {
							if let NodeInput::Node(link_start) = *input {
								Some(FrontendNodeLink {
									link_start,
									link_end,
									link_end_input_index: link_end_input_index as u64,
								})
							} else {
								None
							}
						})
						.collect::<Vec<_>>();

					let mut nodes = Vec::new();
					for (id, node) in &network.nodes {
						nodes.push(FrontendNode {
							id: *id,
							display_name: node.name.clone(),
						})
					}
					log::debug!("Nodes:\n{:#?}\n\nFrontend Nodes:\n{:#?}\n\nLinks:\n{:#?}", network.nodes, nodes, links);
					responses.push_back(FrontendMessage::UpdateNodeGraph { nodes, links }.into());
				}
			}
			NodeGraphMessage::SelectNode { node } => {
				self.selected_nodes = vec![node];
				responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
			}
			NodeGraphMessage::SetInputValue { node, input_index, value } => {
				if let Some(network) = self.get_active_network_mut(document) {
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
