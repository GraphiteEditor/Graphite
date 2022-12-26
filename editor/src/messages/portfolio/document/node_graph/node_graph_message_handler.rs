use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::button_widgets::{BreadcrumbTrailButtons, TextButton};
use crate::messages::prelude::*;

use document_legacy::document::Document;
use document_legacy::layers::layer_info::LayerDataType;
use document_legacy::layers::nodegraph_layer::NodeGraphFrameLayer;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, DocumentNodeMetadata, NodeId, NodeInput, NodeNetwork};

mod document_node_types;
mod node_properties;
pub use self::document_node_types::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FrontendGraphDataType {
	#[default]
	#[serde(rename = "general")]
	General,
	#[serde(rename = "raster")]
	Raster,
	#[serde(rename = "color")]
	Color,
	#[serde(rename = "number")]
	Text,
	#[serde(rename = "vector")]
	Subpath,
	#[serde(rename = "number")]
	Number,
	#[serde(rename = "number")]
	Boolean,
	#[serde(rename = "vec2")]
	Vector,
}
impl FrontendGraphDataType {
	pub const fn with_tagged_value(value: &TaggedValue) -> Self {
		match value {
			TaggedValue::String(_) => Self::Text,
			TaggedValue::F32(_) | TaggedValue::F64(_) | TaggedValue::U32(_) => Self::Number,
			TaggedValue::Bool(_) => Self::Boolean,
			TaggedValue::DVec2(_) => Self::Vector,
			TaggedValue::Image(_) => Self::Raster,
			TaggedValue::Color(_) => Self::Color,
			TaggedValue::RcSubpath(_) | TaggedValue::Subpath(_) => Self::Subpath,
			_ => Self::General,
		}
	}
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
	#[serde(rename = "primaryInput")]
	pub primary_input: Option<FrontendGraphDataType>,
	#[serde(rename = "exposedInputs")]
	pub exposed_inputs: Vec<NodeGraphInput>,
	pub outputs: Vec<FrontendGraphDataType>,
	pub position: (i32, i32),
	pub disabled: bool,
	pub output: bool,
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
	pub category: String,
}
impl FrontendNodeType {
	pub fn new(name: &'static str, category: &'static str) -> Self {
		Self {
			name: name.to_string(),
			category: category.to_string(),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeGraphMessageHandler {
	pub layer_path: Option<Vec<document_legacy::LayerId>>,
	pub nested_path: Vec<graph_craft::document::NodeId>,
	pub selected_nodes: Vec<graph_craft::document::NodeId>,
	#[serde(skip)]
	pub widgets: [LayoutGroup; 2],
}

impl NodeGraphMessageHandler {
	fn get_root_network<'a>(&self, document: &'a Document) -> Option<&'a graph_craft::document::NodeNetwork> {
		self.layer_path.as_ref().and_then(|path| document.layer(path).ok()).and_then(|layer| match &layer.data {
			LayerDataType::NodeGraphFrame(n) => Some(&n.network),
			_ => None,
		})
	}

	fn get_root_network_mut<'a>(&self, document: &'a mut Document) -> Option<&'a mut graph_craft::document::NodeNetwork> {
		self.layer_path.as_ref().and_then(|path| document.layer_mut(path).ok()).and_then(|layer| match &mut layer.data {
			LayerDataType::NodeGraphFrame(n) => Some(&mut n.network),
			_ => None,
		})
	}

	/// Get the active graph_craft NodeNetwork struct
	fn get_active_network_mut<'a>(&self, document: &'a mut Document) -> Option<&'a mut graph_craft::document::NodeNetwork> {
		let mut network = self.get_root_network_mut(document);

		for segement in &self.nested_path {
			network = network.and_then(|network| network.nodes.get_mut(segement)).and_then(|node| node.implementation.get_network_mut());
		}
		network
	}

	/// Send the cached layout for the bar at the top of the node panel to the frontend
	fn send_node_bar_layout(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(
			LayoutMessage::SendLayout {
				layout: Layout::WidgetLayout(WidgetLayout::new(self.widgets.to_vec())),
				layout_target: crate::messages::layout::utility_types::misc::LayoutTarget::NodeGraphBar,
			}
			.into(),
		);
	}

	/// Collect the addresses of the currently viewed nested node e.g. Root -> MyFunFilter -> Exposure
	fn collect_nested_addresses(&mut self, document: &Document, responses: &mut VecDeque<Message>) {
		// Build path list
		let mut path = vec!["Root".to_string()];
		let mut network = self.get_root_network(document);
		for node_id in &self.nested_path {
			let node = network.and_then(|network| network.nodes.get(node_id));
			if let Some(DocumentNode { name, .. }) = node {
				path.push(name.clone());
			}
			network = node.and_then(|node| node.implementation.get_network());
		}
		let nesting = path.len();

		// Update UI
		self.widgets[0] = LayoutGroup::Row {
			widgets: vec![WidgetHolder::new(Widget::BreadcrumbTrailButtons(BreadcrumbTrailButtons {
				labels: path.clone(),
				on_update: WidgetCallback::new(move |input: &u64| {
					NodeGraphMessage::ExitNestedNetwork {
						depth_of_nesting: nesting - (*input as usize) - 1,
					}
					.into()
				}),
				..Default::default()
			}))],
		};

		self.send_node_bar_layout(responses);
	}

	/// Updates the buttons for disable and preview
	fn update_selection_action_buttons(&mut self, document: &mut Document, responses: &mut VecDeque<Message>) {
		if let Some(network) = self.get_active_network_mut(document) {
			let mut widgets = Vec::new();

			// Show an enable or disable nodes button if there is a selection
			if !self.selected_nodes.is_empty() {
				let is_enabled = self.selected_nodes.iter().any(|id| !network.disabled.contains(id));
				let enable_button = WidgetHolder::new(Widget::TextButton(TextButton {
					label: if is_enabled { "Disable" } else { "Enable" }.to_string(),
					on_update: WidgetCallback::new(move |_| NodeGraphMessage::SetSelectedEnabled { enabled: !is_enabled }.into()),
					..Default::default()
				}));
				widgets.push(enable_button);
			}

			// If only one node is selected then show the preview or stop previewing button
			if self.selected_nodes.len() == 1 {
				let is_output = network.output == self.selected_nodes[0];
				// Don't show stop previewing on the output node
				if !(is_output && network.previous_output.filter(|&id| id != self.selected_nodes[0]).is_none()) {
					let output_button = WidgetHolder::new(Widget::TextButton(TextButton {
						label: if is_output { "Stop Previewing" } else { "Preview" }.to_string(),
						on_update: WidgetCallback::new(move |_| NodeGraphMessage::SetSelectedOutput { output: !is_output }.into()),
						..Default::default()
					}));
					widgets.push(output_button);
				}
			}

			self.widgets[1] = LayoutGroup::Row { widgets };
		}
		self.send_node_bar_layout(responses);
	}

	pub fn collate_properties(&self, node_graph_frame: &NodeGraphFrameLayer, context: &mut NodePropertiesContext, sections: &mut Vec<LayoutGroup>) {
		let mut network = &node_graph_frame.network;
		for segement in &self.nested_path {
			network = network.nodes.get(segement).and_then(|node| node.implementation.get_network()).unwrap();
		}

		// If empty, show all nodes in the network starting with the output
		if self.selected_nodes.is_empty() {
			let mut stack = vec![network.output];
			let mut nodes = Vec::new();
			while let Some(node_id) = stack.pop() {
				let Some(document_node) = network.nodes.get(&node_id) else {
					continue;
				};

				stack.extend(document_node.inputs.iter().filter_map(|input| if let NodeInput::Node(ref_id) = input { Some(*ref_id) } else { None }));
				nodes.push((document_node, node_id));
			}
			for &(document_node, node_id) in nodes.iter().rev() {
				sections.push(node_properties::generate_node_properties(document_node, node_id, context));
			}
		}
		for node_id in &self.selected_nodes {
			let Some(document_node) = network.nodes.get(node_id) else {
				continue;
			};

			sections.push(node_properties::generate_node_properties(document_node, *node_id, context));
		}
	}

	fn send_graph(network: &NodeNetwork, responses: &mut VecDeque<Message>) {
		responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());

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
				primary_input: node
					.inputs
					.first()
					.filter(|input| input.is_exposed())
					.and_then(|_| node_type.inputs.get(0))
					.map(|input_type| input_type.data_type),
				exposed_inputs: node
					.inputs
					.iter()
					.zip(node_type.inputs)
					.skip(1)
					.filter(|(input, _)| input.is_exposed())
					.map(|(_, input_type)| NodeGraphInput {
						data_type: input_type.data_type,
						name: input_type.name.to_string(),
					})
					.collect(),
				outputs: node_type.outputs.to_vec(),
				position: node.metadata.position,
				output: network.output == *id,
				disabled: network.disabled.contains(id),
			})
		}
		log::debug!("Frontend Nodes:\n{:#?}\n\nLinks:\n{:#?}", nodes, links);
		responses.push_back(FrontendMessage::UpdateNodeGraph { nodes, links }.into());
	}

	/// Updates the frontend's selection state in line with the backend
	fn update_selected(&mut self, document: &mut Document, responses: &mut VecDeque<Message>) {
		self.update_selection_action_buttons(document, responses);
		responses.push_back(
			FrontendMessage::UpdateNodeGraphSelection {
				selected: self.selected_nodes.clone(),
			}
			.into(),
		);
	}

	fn remove_references_from_network(network: &mut NodeNetwork, node_id: NodeId) -> bool {
		if network.inputs.iter().any(|&id| id == node_id) {
			warn!("Deleting input node");
			return false;
		}
		if network.output == node_id {
			warn!("Deleting the output node!");
			return false;
		}
		for (id, node) in network.nodes.iter_mut() {
			if *id == node_id {
				continue;
			}
			for (input_index, input) in node.inputs.iter_mut().enumerate() {
				let NodeInput::Node(id) = input else {
					continue;
				};
				if *id != node_id {
					continue;
				}

				let Some(node_type) = document_node_types::resolve_document_node_type(&node.name) else {
						warn!("Removing input of invalid node type '{}'", node.name);
						return false;
					};
				if let NodeInput::Value { tagged_value, .. } = &node_type.inputs[input_index].default {
					*input = NodeInput::Value {
						tagged_value: tagged_value.clone(),
						exposed: true,
					};
				}
			}
			if let DocumentNodeImplementation::Network(network) = &mut node.implementation {
				Self::remove_references_from_network(network, node_id);
			}
		}
		true
	}

	fn remove_node(&mut self, network: &mut NodeNetwork, node_id: NodeId) -> bool {
		if Self::remove_references_from_network(network, node_id) {
			network.nodes.remove(&node_id);
			self.selected_nodes.retain(|&id| id != node_id);
			true
		} else {
			false
		}
	}
}

impl MessageHandler<NodeGraphMessage, (&mut Document, &InputPreprocessorMessageHandler)> for NodeGraphMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: NodeGraphMessage, (document, _ipp): (&mut Document, &InputPreprocessorMessageHandler), responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
			NodeGraphMessage::CloseNodeGraph => {
				if let Some(_old_layer_path) = self.layer_path.take() {
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

				Self::send_graph(network, responses);
				responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
			}
			NodeGraphMessage::Copy => {
				let Some(network) = self.get_active_network_mut(document) else {
					error!("No network");
					return;
				};

				// Collect the selected nodes
				let selected_nodes = self
					.selected_nodes
					.iter()
					.filter(|&&id| !network.inputs.contains(&id) && network.output != id) // Don't copy input or output nodes
					.filter_map(|id| network.nodes.get(id))
					.collect::<Vec<_>>();

				// Prefix to show that this is nodes
				let mut copy_text = String::from("graphite/nodes: ");
				copy_text += &serde_json::to_string(&selected_nodes).expect("Could not serialize paste");

				responses.push_back(FrontendMessage::TriggerTextCopy { copy_text }.into());
			}
			NodeGraphMessage::CreateNode { node_id, node_type, x, y } => {
				let node_id = node_id.unwrap_or_else(crate::application::generate_uuid);
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
					..Default::default()
				};

				network.nodes.insert(
					node_id,
					DocumentNode {
						name: node_type.clone(),
						inputs: document_node_type.inputs.iter().map(|input| input.default.clone()).collect(),
						// TODO: Allow inserting nodes that contain other nodes.
						implementation: DocumentNodeImplementation::Network(inner_network),
						metadata: graph_craft::document::DocumentNodeMetadata { position: (x, y) },
					},
				);
				Self::send_graph(network, responses);
			}
			NodeGraphMessage::Cut => {
				responses.push_back(NodeGraphMessage::Copy.into());
				responses.push_back(NodeGraphMessage::DeleteSelectedNodes.into());
			}
			NodeGraphMessage::DeleteNode { node_id } => {
				if let Some(network) = self.get_active_network_mut(document) {
					if self.remove_node(network, node_id) {
						Self::send_graph(network, responses);
						responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
					}
				}
				self.update_selected(document, responses);
			}
			NodeGraphMessage::DeleteSelectedNodes => {
				if let Some(network) = self.get_active_network_mut(document) {
					let mut modified = false;
					for node_id in self.selected_nodes.clone() {
						modified = modified || self.remove_node(network, node_id);
					}
					if modified {
						Self::send_graph(network, responses);
						responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
					}
				}
				self.update_selected(document, responses);
			}
			NodeGraphMessage::DisconnectNodes { node_id, input_index } => {
				let Some(network) = self.get_active_network_mut(document) else {
					warn!("No network");
					return;
				};
				let Some(node) = network.nodes.get_mut(&node_id) else {
					warn!("Invalid node");
					return;
				};
				let Some(node_type) = resolve_document_node_type(&node.name) else {
					warn!("Node {} not in library", node.name);
					return;
				};
				node.inputs[input_index] = node_type.inputs[input_index].default.clone();
				Self::send_graph(network, responses);
			}
			NodeGraphMessage::DoubleClickNode { node } => {
				self.selected_nodes.clear();
				if let Some(network) = self.get_active_network_mut(document) {
					if network.nodes.get(&node).and_then(|node| node.implementation.get_network()).is_some() {
						self.nested_path.push(node);
					}
				}
				if let Some(network) = self.get_active_network_mut(document) {
					Self::send_graph(network, responses);
				}
				self.collect_nested_addresses(document, responses);
				self.update_selected(document, responses);
			}
			NodeGraphMessage::DuplicateSelectedNodes => {
				if let Some(network) = self.get_active_network_mut(document) {
					let mut new_selected = Vec::new();
					for &id in &self.selected_nodes {
						// Don't allow copying input or output nodes.
						if id != network.output && !network.inputs.contains(&id) {
							if let Some(node) = network.nodes.get(&id) {
								let new_id = crate::application::generate_uuid();
								let mut node = node.clone();

								// Shift duplicated nodes
								node.metadata.position.0 += 2;
								node.metadata.position.1 += 2;

								network.nodes.insert(new_id, node);
								new_selected.push(new_id);
							}
						}
					}
					self.selected_nodes = new_selected;
					Self::send_graph(network, responses);
					self.update_selected(document, responses);
				}
			}
			NodeGraphMessage::ExitNestedNetwork { depth_of_nesting } => {
				self.selected_nodes.clear();
				for _ in 0..depth_of_nesting {
					self.nested_path.pop();
				}
				if let Some(network) = self.get_active_network_mut(document) {
					Self::send_graph(network, responses);
				}
				self.collect_nested_addresses(document, responses);
				self.update_selected(document, responses);
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
				} else if let Some(node_type) = document_node_types::resolve_document_node_type(&node.name) {
					if let NodeInput::Value { tagged_value, .. } = &node_type.inputs[input_index].default {
						node.inputs[input_index] = NodeInput::Value {
							tagged_value: tagged_value.clone(),
							exposed: new_exposed,
						};
					}
				}
				Self::send_graph(network, responses);
				responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
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
				Self::send_graph(network, responses);
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
				self.collect_nested_addresses(document, responses);
				self.update_selected(document, responses);
			}
			NodeGraphMessage::PasteNodes { serialized_nodes } => {
				let Some(network) = self.get_active_network_mut(document) else{
					warn!("No network");
					return;
				};

				let data = match serde_json::from_str::<Vec<DocumentNode>>(&serialized_nodes) {
					Ok(d) => d,
					Err(e) => {
						warn!("Invalid node data {e:?}");
						return;
					}
				};

				self.selected_nodes.clear();
				for node in data {
					let id = crate::application::generate_uuid();
					network.nodes.insert(id, node);

					// Select the newly pasted node
					self.selected_nodes.push(id);
				}

				Self::send_graph(network, responses);
				self.update_selected(document, responses);
			}
			NodeGraphMessage::SelectNodes { nodes } => {
				self.selected_nodes = nodes;
				self.update_selection_action_buttons(document, responses);
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
			NodeGraphMessage::SetQualifiedInputValue {
				layer_path,
				node_path,
				input_index,
				value,
			} => {
				let mut network = document.layer_mut(&layer_path).ok().and_then(|layer| match &mut layer.data {
					LayerDataType::NodeGraphFrame(n) => Some(&mut n.network),
					_ => None,
				});

				let Some((node_id, node_path)) = node_path.split_last() else {
					error!("Node path is empty");
					return
				};
				for segement in node_path {
					network = network.and_then(|network| network.nodes.get_mut(segement)).and_then(|node| node.implementation.get_network_mut());
				}

				if let Some(network) = network {
					if let Some(node) = network.nodes.get_mut(node_id) {
						// Extend number of inputs if not already large enough
						if input_index >= node.inputs.len() {
							node.inputs.extend(((node.inputs.len() - 1)..input_index).map(|_| NodeInput::Network));
						}
						node.inputs[input_index] = NodeInput::Value { tagged_value: value, exposed: false };
						responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
					}
				}
			}
			NodeGraphMessage::SetSelectedEnabled { enabled } => {
				if let Some(network) = self.get_active_network_mut(document) {
					if enabled {
						network.disabled.retain(|id| !self.selected_nodes.contains(id));
					} else {
						network.disabled.extend(&self.selected_nodes);
					}
					Self::send_graph(network, responses);
				}
				self.update_selection_action_buttons(document, responses);
				responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
			}
			NodeGraphMessage::SetSelectedOutput { output } => {
				if let Some(network) = self.get_active_network_mut(document) {
					if output {
						network.previous_output = Some(network.previous_output.unwrap_or(network.output));
						network.output = self.selected_nodes[0];
					} else if let Some(output) = network.previous_output.take() {
						network.output = output
					}
					Self::send_graph(network, responses);
				}
				self.update_selection_action_buttons(document, responses);
				responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
			}
			NodeGraphMessage::ShiftNode { node_id } => {
				let Some(network) = self.get_active_network_mut(document) else{
					warn!("No network");
					return;
				};
				let outwards_links = network.collect_outwards_links();
				let required_shift = |left: NodeId, right: NodeId, network: &NodeNetwork| {
					if let (Some(left), Some(right)) = (network.nodes.get(&left), network.nodes.get(&right)) {
						if right.metadata.position.0 < left.metadata.position.0 {
							0
						} else {
							(8 - (right.metadata.position.0 - left.metadata.position.0)).max(0)
						}
					} else {
						0
					}
				};
				let shift_node = |node_id: NodeId, shift: i32, network: &mut NodeNetwork| {
					if let Some(node) = network.nodes.get_mut(&node_id) {
						node.metadata.position.0 += shift
					}
				};
				// Shift the actual node
				let inputs = network
					.nodes
					.get(&node_id)
					.map_or(&Vec::new(), |node| &node.inputs)
					.iter()
					.filter_map(|input| if let NodeInput::Node(previous_id) = input { Some(*previous_id) } else { None })
					.collect::<Vec<_>>();

				for input_node in inputs {
					let shift = required_shift(input_node, node_id, network);
					shift_node(node_id, shift, network);
				}

				// Shift nodes connected to the output port of the specified node
				for &decendant in outwards_links.get(&node_id).unwrap_or(&Vec::new()) {
					let shift = required_shift(node_id, decendant, network);
					let mut stack = vec![decendant];
					while let Some(id) = stack.pop() {
						shift_node(id, shift, network);
						stack.extend(outwards_links.get(&id).unwrap_or(&Vec::new()).iter().copied())
					}
				}
				Self::send_graph(network, responses);
			}
		}
	}

	fn actions(&self) -> ActionList {
		if self.layer_path.is_some() && !self.selected_nodes.is_empty() {
			actions!(NodeGraphMessageDiscriminant; DeleteSelectedNodes, Cut, Copy, DuplicateSelectedNodes)
		} else {
			actions!(NodeGraphMessageDiscriminant;)
		}
	}
}
