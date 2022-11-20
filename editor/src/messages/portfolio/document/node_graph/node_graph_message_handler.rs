use crate::messages::layout::utility_types::layout_widget::LayoutGroup;
use crate::messages::prelude::*;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, DocumentNodeMetadata, NodeInput, NodeNetwork};
use graphene::document::Document;
use graphene::layers::layer_info::LayerDataType;
use graphene::layers::nodegraph_layer::NodeGraphFrameLayer;

mod document_node_types;
mod node_properties;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FrontendGraphDataType {
	#[serde(rename = "general")]
	General,
	#[serde(rename = "raster")]
	Raster,
	#[serde(rename = "color")]
	Color,
	#[serde(rename = "vector")]
	Vector,
	#[serde(rename = "number")]
	Number,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodeGraphInput {
	#[serde(rename = "dataType")]
	data_type: FrontendGraphDataType,
	name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrontendNode {
	pub id: graph_craft::document::NodeId,
	#[serde(rename = "displayName")]
	pub display_name: String,
	#[serde(rename = "exposedInputs")]
	pub exposed_inputs: Vec<NodeGraphInput>,
	pub outputs: Vec<FrontendGraphDataType>,
	pub position: (i32, i32),
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

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrontendNodeType {
	pub name: String,
}
impl FrontendNodeType {
	pub fn new(name: &'static str) -> Self {
		Self { name: name.to_string() }
	}
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

			section.push(node_properties::generate_node_properties(document_node, *node_id));
		}

		section
	}

	fn send_graph(network: &NodeNetwork, responses: &mut VecDeque<Message>) {
		responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
		info!("Opening node graph with nodes {:?}", network.nodes);

		// List of links in format (link_start, link_end, link_end_input_index)
		let links = network
			.nodes
			.iter()
			.flat_map(|(link_end, node)| node.inputs.iter().filter(|input| input.is_exposed()).enumerate().map(move |(index, input)| (input, link_end, index)))
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
			let Some(node_type) = document_node_types::resolve_document_node_type(&node.name) else{
				warn!("Node '{}' does not exist in library", node.name);
				continue
			};
			nodes.push(FrontendNode {
				id: *id,
				display_name: node.name.clone(),
				exposed_inputs: node
					.inputs
					.iter()
					.zip(node_type.inputs)
					.filter(|(input, _)| input.is_exposed())
					.map(|(_, input_type)| NodeGraphInput {
						data_type: input_type.data_type,
						name: input_type.name.to_string(),
					})
					.collect(),
				outputs: node_type.outputs.to_vec(),
				position: node.metadata.position,
			})
		}
		log::debug!("Nodes:\n{:#?}\n\nFrontend Nodes:\n{:#?}\n\nLinks:\n{:#?}", network.nodes, nodes, links);
		responses.push_back(FrontendMessage::UpdateNodeGraph { nodes, links }.into());
	}
}

impl MessageHandler<NodeGraphMessage, (&mut Document, &InputPreprocessorMessageHandler)> for NodeGraphMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: NodeGraphMessage, (document, _ipp): (&mut Document, &InputPreprocessorMessageHandler), responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
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

				let Some(network) = self.get_active_network_mut(document) else {
					error!("No network");
					return;
				 };
				let Some(input_node) = network.nodes.get_mut(&input_node) else {
					error!("No to");
					return;
				};
				let Some((actual_index, _)) = input_node.inputs.iter().enumerate().filter(|input|input.1.is_exposed()).nth(input_node_connector_index) else {
					error!("Failed to find actual index of connector indes {input_node_connector_index} on node {input_node:#?}");
					return;
				};
				input_node.inputs[actual_index] = NodeInput::Node(output_node);

				info!("Inputs: {:?}", input_node.inputs);
				Self::send_graph(network, responses);
				responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
			}
			NodeGraphMessage::CreateNode { node_id, node_type } => {
				let Some(network) = self.get_active_network_mut(document) else{
					warn!("No network");
					return;
				};

				let Some(document_node_type) = document_node_types::resolve_document_node_type(&node_type) else{
					responses.push_back(DialogMessage::DisplayDialogError { title: "Cannot insert node".to_string(), description: format!("The document node '{node_type}' does not exist in the document node list") }.into());
					return;
				};

				let num_inputs = document_node_type.inputs.len();

				let inner_network = NodeNetwork {
					inputs: (0..num_inputs).map(|_| 0).collect(),
					output: 0,
					nodes: [(
						0,
						DocumentNode {
							name: format!("{}_impl", document_node_type.name),
							// TODO: Allow inserting nodes that contain other nodes.
							implementation: DocumentNodeImplementation::Unresolved(document_node_type.identifier.clone()),
							inputs: (0..num_inputs).map(|_| NodeInput::Network).collect(),
							metadata: DocumentNodeMetadata::default(),
						},
					)]
					.into_iter()
					.collect(),
				};
				network.nodes.insert(
					node_id,
					DocumentNode {
						name: node_type.clone(),
						inputs: document_node_type.inputs.iter().map(|input| input.default.clone()).collect(),
						// TODO: Allow inserting nodes that contain other nodes.
						implementation: DocumentNodeImplementation::Network(inner_network),
						metadata: graph_craft::document::DocumentNodeMetadata {
							// TODO: Better position default
							position: (node_id as i32 * 7 - 41, node_id as i32 * 2 - 10),
						},
					},
				);
				Self::send_graph(network, responses);
			}
			NodeGraphMessage::DeleteNode { node_id } => {
				if let Some(network) = self.get_active_network_mut(document) {
					network.nodes.remove(&node_id);
					Self::send_graph(network, responses);
					responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
				}
			}
			NodeGraphMessage::ExposeInput { node_id, input_index, new_exposed } => {
				let Some(network) = self.get_active_network_mut(document) else{
					warn!("No network");
					return;
				};

				let Some(node) = network.nodes.get_mut(&node_id) else {
					warn!("No node");
					return;
				};

				if let NodeInput::Value { exposed, .. } = &mut node.inputs[input_index] {
					*exposed = new_exposed;
				}
				Self::send_graph(network, responses);
			}
			NodeGraphMessage::MoveSelectedNodes { displacement_x, displacement_y } => {
				let Some(network) = self.get_active_network_mut(document) else{
					warn!("No network");
					return;
				};

				for node_id in &self.selected_nodes {
					if let Some(node) = network.nodes.get_mut(node_id) {
						node.metadata.position.0 += displacement_x;
						node.metadata.position.1 += displacement_y;
					}
				}
			}
			NodeGraphMessage::OpenNodeGraph { layer_path } => {
				if let Some(_old_layer_path) = self.layer_path.replace(layer_path) {
					// TODO: Necessary cleanup of old node graph
				}

				if let Some(network) = self.get_active_network_mut(document) {
					self.selected_nodes.clear();
					responses.push_back(FrontendMessage::UpdateNodeGraphVisibility { visible: true }.into());

					Self::send_graph(network, responses);

					let node_types = document_node_types::collect_node_types();
					responses.push_back(FrontendMessage::UpdateNodeTypes { node_types }.into());
				}
			}
			NodeGraphMessage::SelectNodes { nodes } => {
				self.selected_nodes = nodes;
				responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
			}
			NodeGraphMessage::SetInputValue { node, input_index, value } => {
				if let Some(network) = self.get_active_network_mut(document) {
					if let Some(node) = network.nodes.get_mut(&node) {
						// Extend number of inputs if not already large enough
						if input_index >= node.inputs.len() {
							node.inputs.extend(((node.inputs.len() - 1)..input_index).map(|_| NodeInput::Network));
						}
						node.inputs[input_index] = NodeInput::Value { tagged_value: value, exposed: false };
						responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
					}
				}
			}
		}
	}

	advertise_actions!(NodeGraphMessageDiscriminant; DeleteNode,);
}
