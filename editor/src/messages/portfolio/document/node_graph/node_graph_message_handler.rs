use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::button_widgets::{BreadcrumbTrailButtons, TextButton};
use crate::messages::prelude::*;

use document_legacy::document::Document;
use document_legacy::layers::layer_info::{LayerDataType, LayerDataTypeDiscriminant};
use document_legacy::layers::nodegraph_layer::NodeGraphFrameLayer;
use document_legacy::LayerId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, DocumentNodeMetadata, NodeId, NodeInput, NodeNetwork};

mod document_node_types;
mod node_properties;
pub use self::document_node_types::*;

use glam::IVec2;

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
	/// Do not allow the node graph window to open or close whilst the user is drawing a node graph frame
	#[serde(skip)]
	pub is_drawing_node_graph_frame: bool,
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
	fn get_active_network<'a>(&self, document: &'a Document) -> Option<&'a graph_craft::document::NodeNetwork> {
		let mut network = self.get_root_network(document);

		for segement in &self.nested_path {
			network = network.and_then(|network| network.nodes.get(segement)).and_then(|node| node.implementation.get_network());
		}
		network
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
		if let Some(network) = self.get_active_network(document) {
			let mut widgets = Vec::new();

			// Don't allow disabling input or output nodes
			let mut selected_nodes = self.selected_nodes.iter().filter(|&&id| !network.inputs.contains(&id) && network.original_output() != id);

			// If there is at least one other selected node then show the hide or show button
			if selected_nodes.next().is_some() {
				// Check if any of the selected nodes are disabled
				let is_hidden = self.selected_nodes.iter().any(|id| network.disabled.contains(id));

				// Check if multiple nodes are selected
				let mutliple_nodes = selected_nodes.next().is_some();

				// Generate the enable or disable button accordingly
				let hide_button = WidgetHolder::new(Widget::TextButton(TextButton {
					label: if is_hidden { "Show" } else { "Hide" }.to_string(),
					tooltip: if is_hidden { "Show node" } else { "Hide node" }.to_string() + if mutliple_nodes { "s" } else { "" },
					tooltip_shortcut: action_keys!(NodeGraphMessageDiscriminant::ToggleHidden),
					on_update: WidgetCallback::new(move |_| NodeGraphMessage::ToggleHidden.into()),
					..Default::default()
				}));
				widgets.push(hide_button);
			}

			// If only one node is selected then show the preview or stop previewing button
			if self.selected_nodes.len() == 1 {
				let node_id = self.selected_nodes[0];
				// Is this node the current output
				let is_output = network.output == node_id;

				// Don't show stop previewing button on the origional output node
				if !(is_output && network.previous_output.filter(|&id| id != self.selected_nodes[0]).is_none()) {
					let output_button = WidgetHolder::new(Widget::TextButton(TextButton {
						label: if is_output { "End Preview" } else { "Preview" }.to_string(),
						tooltip: if is_output { "Restore preview to Output node" } else { "Preview node" }.to_string() + " (shortcut: Alt+click node)",
						on_update: WidgetCallback::new(move |_| NodeGraphMessage::TogglePreview { node_id }.into()),
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
				position: node.metadata.position.into(),
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

	/// Gets the default node input based on the node name and the input index
	fn default_node_input(name: String, index: usize) -> Option<NodeInput> {
		resolve_document_node_type(&name)
			.and_then(|node| node.inputs.get(index))
			.map(|input: &DocumentInputType| input.default.clone())
	}

	/// Returns an iterator of nodes to be copied and their ids, excluding output and input nodes
	fn copy_nodes<'a>(network: &'a NodeNetwork, new_ids: &'a HashMap<NodeId, NodeId>) -> impl Iterator<Item = (NodeId, DocumentNode)> + 'a {
		new_ids
			.iter()
			.filter(|&(&id, _)| id != network.output && !network.inputs.contains(&id))
			.filter_map(|(&id, &new)| network.nodes.get(&id).map(|node| (new, node.clone())))
			.map(move |(new, node)| (new, node.map_ids(Self::default_node_input, new_ids)))
	}
}

impl MessageHandler<NodeGraphMessage, (&mut Document, &mut dyn Iterator<Item = &[LayerId]>)> for NodeGraphMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: NodeGraphMessage, (document, selected): (&mut Document, &mut dyn Iterator<Item = &[LayerId]>), responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
			NodeGraphMessage::CloseNodeGraph => {
				// Don't close when drawing a node graph frame
				if self.is_drawing_node_graph_frame {
					return;
				}

				if let Some(_old_layer_path) = self.layer_path.take() {
					responses.push_back(FrontendMessage::UpdateNodeGraphVisibility { visible: false }.into());
					responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
				}
			}
			NodeGraphMessage::ConnectNodesByLink {
				output_node,
				input_node,
				input_node_connector_index,
			} => {
				log::debug!("Connect primary output from node {output_node} to input of index {input_node_connector_index} on node {input_node}.");
				let node_id = input_node;

				let Some(network) = self.get_active_network(document) else {
					error!("No network");
					return;
				 };
				let Some(input_node) = network.nodes.get(&input_node) else {
					error!("No to");
					return;
				};
				let Some((input_index, _)) = input_node.inputs.iter().enumerate().filter(|input|input.1.is_exposed()).nth(input_node_connector_index) else {
					error!("Failed to find actual index of connector indes {input_node_connector_index} on node {input_node:#?}");
					return;
				};

				responses.push_back(DocumentMessage::StartTransaction.into());

				let input = NodeInput::Node(output_node);
				responses.push_back(NodeGraphMessage::SetNodeInput { node_id, input_index, input }.into());

				responses.push_back(NodeGraphMessage::SendGraph.into());
			}
			NodeGraphMessage::Copy => {
				let Some(network) = self.get_active_network(document) else {
					error!("No network");
					return;
				};

				// Collect the selected nodes
				let new_ids = &self.selected_nodes.iter().copied().enumerate().map(|(new, old)| (old, new as NodeId)).collect();
				let copied_nodes: Vec<_> = Self::copy_nodes(network, new_ids).collect();

				// Prefix to show that this is nodes
				let mut copy_text = String::from("graphite/nodes: ");
				copy_text += &serde_json::to_string(&copied_nodes).expect("Could not serialize paste");

				responses.push_back(FrontendMessage::TriggerTextCopy { copy_text }.into());
			}
			NodeGraphMessage::CreateNode { node_id, node_type, x, y } => {
				let node_id = node_id.unwrap_or_else(crate::application::generate_uuid);

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

				responses.push_back(DocumentMessage::StartTransaction.into());

				let document_node = DocumentNode {
					name: node_type.clone(),
					inputs: document_node_type.inputs.iter().map(|input| input.default.clone()).collect(),
					// TODO: Allow inserting nodes that contain other nodes.
					implementation: DocumentNodeImplementation::Network(inner_network),
					metadata: graph_craft::document::DocumentNodeMetadata { position: (x, y).into() },
				};
				responses.push_back(NodeGraphMessage::InsertNode { node_id, document_node }.into());

				responses.push_back(NodeGraphMessage::SendGraph.into());
			}
			NodeGraphMessage::Cut => {
				responses.push_back(NodeGraphMessage::Copy.into());
				responses.push_back(NodeGraphMessage::DeleteSelectedNodes.into());
			}
			NodeGraphMessage::DeleteNode { node_id } => {
				if let Some(network) = self.get_active_network_mut(document) {
					self.remove_node(network, node_id);
				}
				self.update_selected(document, responses);
			}
			NodeGraphMessage::DeleteSelectedNodes => {
				responses.push_back(DocumentMessage::StartTransaction.into());

				for node_id in self.selected_nodes.clone() {
					responses.push_back(NodeGraphMessage::DeleteNode { node_id }.into());
				}

				responses.push_back(NodeGraphMessage::SendGraph.into());
				responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
			}
			NodeGraphMessage::DisconnectNodes { node_id, input_index } => {
				let Some(network) = self.get_active_network(document) else {
					warn!("No network");
					return;
				};
				let Some(node) = network.nodes.get(&node_id) else {
					warn!("Invalid node");
					return;
				};
				let Some(node_type) = resolve_document_node_type(&node.name) else {
					warn!("Node {} not in library", node.name);
					return;
				};

				responses.push_back(DocumentMessage::StartTransaction.into());

				let input = node_type.inputs[input_index].default.clone();
				responses.push_back(NodeGraphMessage::SetNodeInput { node_id, input_index, input }.into());

				responses.push_back(NodeGraphMessage::SendGraph.into());
			}
			NodeGraphMessage::DoubleClickNode { node } => {
				if let Some(network) = self.get_active_network(document) {
					if network.nodes.get(&node).and_then(|node| node.implementation.get_network()).is_some() {
						self.nested_path.push(node);
					}
				}
				if let Some(network) = self.get_active_network(document) {
					Self::send_graph(network, responses);
				}
				self.collect_nested_addresses(document, responses);
				self.update_selected(document, responses);
			}
			NodeGraphMessage::DuplicateSelectedNodes => {
				if let Some(network) = self.get_active_network(document) {
					responses.push_back(DocumentMessage::StartTransaction.into());

					let new_ids = &self.selected_nodes.iter().map(|&id| (id, crate::application::generate_uuid())).collect();
					self.selected_nodes.clear();

					// Copy the selected nodes
					let copied_nodes = Self::copy_nodes(network, new_ids).collect::<Vec<_>>();
					for (node_id, mut document_node) in copied_nodes {
						// Shift duplicated node
						document_node.metadata.position += IVec2::splat(2);

						// Add new node to the list
						self.selected_nodes.push(node_id);

						// Insert new node into graph
						responses.push_back(NodeGraphMessage::InsertNode { node_id, document_node }.into());
					}

					Self::send_graph(network, responses);
					self.update_selected(document, responses);
				}
			}
			NodeGraphMessage::ExitNestedNetwork { depth_of_nesting } => {
				self.selected_nodes.clear();
				for _ in 0..depth_of_nesting {
					self.nested_path.pop();
				}
				if let Some(network) = self.get_active_network(document) {
					Self::send_graph(network, responses);
				}
				self.collect_nested_addresses(document, responses);
				self.update_selected(document, responses);
			}
			NodeGraphMessage::ExposeInput { node_id, input_index, new_exposed } => {
				let Some(network) = self.get_active_network(document) else{
					warn!("No network");
					return;
				};

				let Some(node) = network.nodes.get(&node_id) else {
					warn!("No node");
					return;
				};

				responses.push_back(DocumentMessage::StartTransaction.into());

				let mut input = node.inputs[input_index].clone();
				if let NodeInput::Value { exposed, .. } = &mut input {
					*exposed = new_exposed;
				} else if let Some(node_type) = document_node_types::resolve_document_node_type(&node.name) {
					if let NodeInput::Value { tagged_value, .. } = &node_type.inputs[input_index].default {
						input = NodeInput::Value {
							tagged_value: tagged_value.clone(),
							exposed: new_exposed,
						};
					}
				}
				responses.push_back(NodeGraphMessage::SetNodeInput { node_id, input_index, input }.into());

				responses.push_back(NodeGraphMessage::SendGraph.into());
				responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
			}
			NodeGraphMessage::InsertNode { node_id, document_node } => {
				if let Some(network) = self.get_active_network_mut(document) {
					network.nodes.insert(node_id, document_node);
				}
			}
			NodeGraphMessage::MoveSelectedNodes { displacement_x, displacement_y } => {
				let Some(network) = self.get_active_network_mut(document) else{
					warn!("No network");
					return;
				};

				for node_id in &self.selected_nodes {
					if let Some(node) = network.nodes.get_mut(node_id) {
						node.metadata.position += IVec2::new(displacement_x, displacement_y)
					}
				}
				Self::send_graph(network, responses);
			}
			NodeGraphMessage::OpenNodeGraph { layer_path } => {
				// Don't open when drawing a node graph frame
				if self.is_drawing_node_graph_frame {
					return;
				}

				self.layer_path = Some(layer_path);

				if let Some(network) = self.get_active_network(document) {
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
				let Some(network) = self.get_active_network(document) else{
					warn!("No network");
					return;
				};

				let data = match serde_json::from_str::<Vec<(NodeId, DocumentNode)>>(&serialized_nodes) {
					Ok(d) => d,
					Err(e) => {
						warn!("Invalid node data {e:?}");
						return;
					}
				};

				// Shift nodes until it is not in the same position as another node
				let mut shift = IVec2::ZERO;
				while data
					.iter()
					.all(|(_, node)| network.nodes.values().any(|existing_node| node.metadata.position + shift == existing_node.metadata.position))
				{
					shift += IVec2::splat(2);
				}

				responses.push_back(DocumentMessage::StartTransaction.into());

				let new_ids: HashMap<_, _> = data.iter().map(|&(id, _)| (id, crate::application::generate_uuid())).collect();
				for (old_id, mut document_node) in data {
					// Shift copied node
					document_node.metadata.position += shift;

					// Get the new, non-conflicting id
					let node_id = *new_ids.get(&old_id).unwrap();
					document_node = document_node.map_ids(Self::default_node_input, &new_ids);

					// Insert node into network
					responses.push_back(NodeGraphMessage::InsertNode { node_id, document_node }.into());
				}

				let nodes = new_ids.values().copied().collect();
				responses.push_back(NodeGraphMessage::SelectNodes { nodes }.into());

				responses.push_back(NodeGraphMessage::SendGraph.into());
			}
			NodeGraphMessage::SelectNodes { nodes } => {
				self.selected_nodes = nodes;
				self.update_selection_action_buttons(document, responses);
				self.update_selected(document, responses);
				responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
			}
			NodeGraphMessage::SendGraph => {
				if let Some(network) = self.get_active_network(document) {
					Self::send_graph(network, responses);
					responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
				}
			}
			NodeGraphMessage::SetDrawing { new_drawing } => {
				let selected: Vec<_> = selected.collect();
				// Check if we stopped drawing a node graph frame
				if self.is_drawing_node_graph_frame && !new_drawing {
					// Check if we should open or close the node graph
					if selected.len() == 1
						&& document
							.layer(selected[0])
							.ok()
							.filter(|layer| LayerDataTypeDiscriminant::from(&layer.data) == LayerDataTypeDiscriminant::NodeGraphFrame)
							.is_some()
					{
						responses.push_back(NodeGraphMessage::OpenNodeGraph { layer_path: selected[0].to_vec() }.into());
					} else {
						responses.push_back(NodeGraphMessage::CloseNodeGraph.into());
					}
				}
				self.is_drawing_node_graph_frame = new_drawing
			}
			NodeGraphMessage::SetInputValue { node: node_id, input_index, value } => {
				if let Some(network) = self.get_active_network(document) {
					if let Some(node) = network.nodes.get(&node_id) {
						responses.push_back(DocumentMessage::StartTransaction.into());

						let input = NodeInput::Value { tagged_value: value, exposed: false };
						responses.push_back(NodeGraphMessage::SetNodeInput { node_id, input_index, input }.into());
						responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
						if node.name != "Imaginate" || input_index == 0 {
							responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
						}
					}
				}
			}
			NodeGraphMessage::SetNodeInput { node_id, input_index, input } => {
				if let Some(network) = self.get_active_network_mut(document) {
					if let Some(node) = network.nodes.get_mut(&node_id) {
						node.inputs[input_index] = input
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
			NodeGraphMessage::ShiftNode { node_id } => {
				let Some(network) = self.get_active_network_mut(document) else{
					warn!("No network");
					return;
				};
				let outwards_links = network.collect_outwards_links();
				let required_shift = |left: NodeId, right: NodeId, network: &NodeNetwork| {
					if let (Some(left), Some(right)) = (network.nodes.get(&left), network.nodes.get(&right)) {
						if right.metadata.position.x < left.metadata.position.x {
							0
						} else {
							(8 - (right.metadata.position.x - left.metadata.position.x)).max(0)
						}
					} else {
						0
					}
				};
				let shift_node = |node_id: NodeId, shift: i32, network: &mut NodeNetwork| {
					if let Some(node) = network.nodes.get_mut(&node_id) {
						node.metadata.position.x += shift
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
				responses.push_back(NodeGraphMessage::SendGraph.into());
			}
			NodeGraphMessage::ToggleHidden => {
				if let Some(network) = self.get_active_network_mut(document) {
					// Check if any of the selected nodes are hidden
					if self.selected_nodes.iter().any(|id| network.disabled.contains(id)) {
						// Remove all selected nodes from the disabled list
						network.disabled.retain(|id| !self.selected_nodes.contains(id));
					} else {
						let original_output = network.original_output();
						// Add all selected nodes to the disabled list (excluding input or output nodes)
						network.disabled.extend(self.selected_nodes.iter().filter(|&id| !network.inputs.contains(id) && original_output != *id));
					}
					Self::send_graph(network, responses);
				}
				self.update_selection_action_buttons(document, responses);
				responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
			}
			NodeGraphMessage::TogglePreview { node_id } => {
				if let Some(network) = self.get_active_network_mut(document) {
					// Check if the node is not already
					if network.output != node_id {
						network.previous_output = Some(network.previous_output.unwrap_or(network.output));
						network.output = node_id;
					} else if let Some(output) = network.previous_output.take() {
						network.output = output
					}
					Self::send_graph(network, responses);
				}
				self.update_selection_action_buttons(document, responses);
				responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
			}
		}
	}

	fn actions(&self) -> ActionList {
		if self.layer_path.is_some() && !self.selected_nodes.is_empty() {
			actions!(NodeGraphMessageDiscriminant; DeleteSelectedNodes, Cut, Copy, DuplicateSelectedNodes, ToggleHidden)
		} else {
			actions!(NodeGraphMessageDiscriminant;)
		}
	}
}
