pub use self::document_node_types::*;
use super::load_network_structure;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::LayerId;
use crate::messages::prelude::*;
use crate::node_graph_executor::GraphIdentifier;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput, NodeNetwork, NodeOutput};
use graphene_core::*;
mod document_node_types;
mod node_properties;

use glam::IVec2;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
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
	/// Refers to the mathematical vector, with direction and magnitude.
	#[serde(rename = "vec2")]
	Vector,
	#[serde(rename = "graphic")]
	GraphicGroup,
	#[serde(rename = "artboard")]
	Artboard,
	#[serde(rename = "palette")]
	Palette,
}
impl FrontendGraphDataType {
	pub const fn with_tagged_value(value: &TaggedValue) -> Self {
		match value {
			TaggedValue::String(_) => Self::Text,
			TaggedValue::F32(_) | TaggedValue::F64(_) | TaggedValue::U32(_) | TaggedValue::DAffine2(_) => Self::Number,
			TaggedValue::Bool(_) => Self::Boolean,
			TaggedValue::DVec2(_) | TaggedValue::IVec2(_) => Self::Vector,
			TaggedValue::Image(_) => Self::Raster,
			TaggedValue::ImageFrame(_) => Self::Raster,
			TaggedValue::Color(_) => Self::Color,
			TaggedValue::RcSubpath(_) | TaggedValue::Subpaths(_) | TaggedValue::VectorData(_) => Self::Subpath,
			TaggedValue::GraphicGroup(_) => Self::GraphicGroup,
			TaggedValue::Artboard(_) => Self::Artboard,
			TaggedValue::Palette(_) => Self::Palette,
			_ => Self::General,
		}
	}
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphInput {
	#[serde(rename = "dataType")]
	data_type: FrontendGraphDataType,
	name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphOutput {
	#[serde(rename = "dataType")]
	data_type: FrontendGraphDataType,
	name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNode {
	#[serde(rename = "isLayer")]
	pub is_layer: bool,
	pub id: graph_craft::document::NodeId,
	pub alias: String,
	pub name: String,
	#[serde(rename = "primaryInput")]
	pub primary_input: Option<FrontendGraphInput>,
	#[serde(rename = "exposedInputs")]
	pub exposed_inputs: Vec<FrontendGraphInput>,
	#[serde(rename = "primaryOutput")]
	pub primary_output: Option<FrontendGraphOutput>,
	#[serde(rename = "exposedOutputs")]
	pub exposed_outputs: Vec<FrontendGraphOutput>,
	pub position: (i32, i32),
	pub disabled: bool,
	pub previewed: bool,
}

// (link_start, link_end, link_end_input_index)
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeLink {
	#[serde(rename = "linkStart")]
	pub link_start: u64,
	#[serde(rename = "linkStartOutputIndex")]
	pub link_start_output_index: usize,
	#[serde(rename = "linkEnd")]
	pub link_end: u64,
	#[serde(rename = "linkEndInputIndex")]
	pub link_end_input_index: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct NodeGraphMessageHandler {
	pub layer_path: Option<Vec<LayerId>>,
	pub network: Vec<NodeId>,
	has_selection: bool,
	pub widgets: [LayoutGroup; 2],
}

impl Default for NodeGraphMessageHandler {
	fn default() -> Self {
		// TODO: Replace this with an "Add Node" button, also next to an "Add Layer" button
		let add_nodes_label = TextLabel::new("Right Click Graph to Add Nodes").italic(true).widget_holder();
		let add_nodes_label_row = LayoutGroup::Row { widgets: vec![add_nodes_label] };

		Self {
			layer_path: None,
			network: Vec::new(),
			has_selection: false,
			widgets: [add_nodes_label_row, LayoutGroup::default()],
		}
	}
}

impl NodeGraphMessageHandler {
	/// Send the cached layout to the frontend for the options bar at the top of the node panel
	fn send_node_bar_layout(&self, responses: &mut VecDeque<Message>) {
		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout::new(self.widgets.to_vec())),
			layout_target: LayoutTarget::NodeGraphBar,
		});
	}

	/// Updates the buttons for disable and preview
	fn update_selection_action_buttons(&mut self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, responses: &mut VecDeque<Message>) {
		if let Some(network) = document_network.nested_network(&self.network) {
			let mut widgets = Vec::new();

			// Don't allow disabling input or output nodes
			let mut selected_nodes = document_metadata.selected_nodes().filter(|&&id| !network.inputs.contains(&id) && !network.original_outputs_contain(id));

			// If there is at least one other selected node then show the hide or show button
			if selected_nodes.next().is_some() {
				widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

				// Check if any of the selected nodes are disabled
				let is_hidden = document_metadata.selected_nodes().any(|id| network.disabled.contains(id));

				// Check if multiple nodes are selected
				let multiple_nodes = selected_nodes.next().is_some();

				// Generate the enable or disable button accordingly
				let (hide_show_label, hide_show_icon) = if is_hidden { ("Make Visible", "EyeHidden") } else { ("Make Hidden", "EyeVisible") };
				let hide_button = TextButton::new(hide_show_label)
					.icon(Some(hide_show_icon.to_string()))
					.tooltip(if is_hidden { "Show selected nodes/layers" } else { "Hide selected nodes/layers" }.to_string() + if multiple_nodes { "s" } else { "" })
					.tooltip_shortcut(action_keys!(NodeGraphMessageDiscriminant::ToggleSelectedHidden))
					.on_update(move |_| NodeGraphMessage::ToggleSelectedHidden.into())
					.widget_holder();
				widgets.push(hide_button);
			}

			// If only one node is selected then show the preview or stop previewing button
			let mut selected_nodes = document_metadata.selected_nodes();
			if let (Some(&node_id), None) = (selected_nodes.next(), selected_nodes.next()) {
				widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

				// Is this node the current output
				let is_output = network.outputs_contain(node_id);

				// Don't show stop previewing button on the original output node
				if !(is_output && network.previous_outputs_contain(node_id).unwrap_or(true)) {
					let output_button = TextButton::new(if is_output { "End Preview" } else { "Preview" })
						.icon(Some("Rescale".to_string()))
						.tooltip(if is_output { "Restore preview to the graph output" } else { "Preview selected node/layer" }.to_string() + " (Shortcut: Alt-click node/layer)")
						.on_update(move |_| NodeGraphMessage::TogglePreview { node_id }.into())
						.widget_holder();
					widgets.push(output_button);
				}
			}

			self.widgets[1] = LayoutGroup::Row { widgets };
		}
		self.send_node_bar_layout(responses);
	}

	/// Collate the properties panel sections for a node graph
	pub fn collate_properties(&self, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
		let mut network = context.network;

		for segment in &self.network {
			network = network.nodes.get(segment).and_then(|node| node.implementation.get_network()).unwrap();
		}

		// We want:
		// - If only nodes (no layers) are selected: display each node's properties
		// - If one layer is selected, and zero or more of its upstream nodes: display the properties for the layer and its upstream nodes
		// - If multiple layers are selected, or one node plus other non-upstream nodes: display nothing

		// First, we filter all the selections into layers and nodes
		let (mut layers, mut nodes) = (Vec::new(), Vec::new());
		for node_id in context.metadata.selected_nodes() {
			if let Some(layer_or_node) = network.nodes.get(node_id) {
				if layer_or_node.is_layer() {
					layers.push(*node_id);
				} else {
					nodes.push(*node_id);
				}
			};
		}

		// Next, we decide what to display based on the number of layers and nodes selected
		match layers.len() {
			// If no layers are selected, show properties for all selected nodes
			0 => nodes
				.iter()
				.filter_map(|node_id| network.nodes.get(node_id).map(|node| node_properties::generate_node_properties(node, *node_id, context)))
				.collect(),
			// If one layer is selected, filter out all selected nodes that are not upstream of it. If there are no nodes left, show properties for the layer. Otherwise, show nothing.
			1 => {
				let nodes_not_upstream_of_layer = nodes
					.into_iter()
					.filter(|&selected_node_id| !network.is_node_upstream_of_another_by_primary_flow(layers[0], selected_node_id));
				if nodes_not_upstream_of_layer.count() > 0 {
					return Vec::new();
				}

				// Iterate through all the upstream nodes, but stop when we reach another layer (since that's a point where we switch from horizontal to vertical flow)
				network
					.upstream_flow_back_from_nodes(vec![layers[0]], true)
					.enumerate()
					.take_while(|(i, (node, _))| if *i == 0 { true } else { !node.is_layer() })
					.map(|(_, (node, node_id))| node_properties::generate_node_properties(node, node_id, context))
					.collect()
			}
			// If multiple layers and/or nodes are selected, show nothing
			_ => Vec::new(),
		}
	}

	fn send_graph(network: &NodeNetwork, layer_path: &Option<Vec<LayerId>>, graph_view_overlay_open: bool, responses: &mut VecDeque<Message>) {
		responses.add(PropertiesPanelMessage::Refresh);

		if !graph_view_overlay_open {
			return;
		}

		let layer_id = layer_path.as_ref().and_then(|path| path.last().copied());

		// List of links in format (link_start, link_end, link_end_input_index)
		let links = network
			.nodes
			.iter()
			.flat_map(|(link_end, node)| node.inputs.iter().filter(|input| input.is_exposed()).enumerate().map(move |(index, input)| (input, link_end, index)))
			.filter_map(|(input, &link_end, link_end_input_index)| {
				if let NodeInput::Node {
					node_id: link_start,
					output_index: link_start_index,
					// TODO: add ui for lambdas
					lambda: _,
				} = *input
				{
					Some(FrontendNodeLink {
						link_start,
						link_start_output_index: link_start_index,
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
			// TODO: This should be based on the graph runtime type inference system in order to change the colors of node connectors to match the data type in use
			let Some(node_type) = document_node_types::resolve_document_node_type(&node.name) else {
				warn!("Node '{}' does not exist in library", node.name);
				continue;
			};

			// Inputs
			let mut inputs = node.inputs.iter().zip(node_type.inputs.iter().map(|input_type| FrontendGraphInput {
				data_type: input_type.data_type,
				name: input_type.name.to_string(),
			}));
			let primary_input = inputs.next().filter(|(input, _)| input.is_exposed()).map(|(_, input_type)| input_type);
			let exposed_inputs = inputs.filter(|(input, _)| input.is_exposed()).map(|(_, input_type)| input_type).collect();

			// Outputs
			let mut outputs = node_type.outputs.iter().map(|output_type| FrontendGraphOutput {
				data_type: output_type.data_type,
				name: output_type.name.to_string(),
			});
			let primary_output = if node.has_primary_output { outputs.next() } else { None };

			let _graph_identifier = GraphIdentifier::new(layer_id);

			nodes.push(FrontendNode {
				is_layer: node.is_layer(),
				id: *id,
				alias: node.alias.clone(),
				name: node.name.clone(),
				primary_input,
				exposed_inputs,
				primary_output,
				exposed_outputs: outputs.collect::<Vec<_>>(),
				position: node.metadata.position.into(),
				previewed: network.outputs_contain(*id),
				disabled: network.disabled.contains(id),
			})
		}
		responses.add(FrontendMessage::UpdateNodeGraph { nodes, links });
	}

	/// Updates the frontend's selection state in line with the backend
	fn update_selected(&mut self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, responses: &mut VecDeque<Message>) {
		self.update_selection_action_buttons(document_network, document_metadata, responses);
		responses.add(FrontendMessage::UpdateNodeGraphSelection {
			selected: document_metadata.selected_nodes_ref().clone(),
		});
	}

	fn remove_references_from_network(network: &mut NodeNetwork, deleting_node_id: NodeId, reconnect: bool) -> bool {
		if network.inputs.contains(&deleting_node_id) {
			warn!("Deleting input node!");
			return false;
		}
		if network.outputs_contain(deleting_node_id) {
			warn!("Deleting the output node!");
			return false;
		}

		let mut reconnect_to_input: Option<NodeInput> = None;

		if reconnect {
			// Check whether the being-deleted node's first (primary) input is a node
			if let Some(node) = network.nodes.get(&deleting_node_id) {
				// Reconnect to the node below when deleting a layer node.
				let reconnect_from_input_index = if node.is_layer() { 1 } else { 0 };
				if matches!(&node.inputs.get(reconnect_from_input_index), Some(NodeInput::Node { .. })) {
					reconnect_to_input = Some(node.inputs[reconnect_from_input_index].clone());
				}
			}
		}

		for (node_id, node) in network.nodes.iter_mut() {
			if *node_id == deleting_node_id {
				continue;
			}
			for (input_index, input) in node.inputs.iter_mut().enumerate() {
				let NodeInput::Node {
					node_id: upstream_node_id,
					output_index,
					..
				} = input
				else {
					continue;
				};
				if *upstream_node_id != deleting_node_id {
					continue;
				}

				let Some(node_type) = document_node_types::resolve_document_node_type(&node.name) else {
					warn!("Removing input of invalid node type '{}'", node.name);
					return false;
				};

				if let NodeInput::Value { tagged_value, .. } = &node_type.inputs[input_index].default {
					let mut refers_to_output_node = false;

					// Use the first input node as the new input if deleting node's first input is a node,
					// and the current node uses its primary output too
					if let Some(reconnect_to_input) = &reconnect_to_input {
						if *output_index == 0 {
							refers_to_output_node = true;
							*input = reconnect_to_input.clone()
						}
					}

					if !refers_to_output_node {
						*input = NodeInput::value(tagged_value.clone(), true);
					}
				}
			}
		}
		true
	}

	/// Tries to remove a node from the network, returning true on success.
	fn remove_node(&mut self, document_network: &mut NodeNetwork, document_metadata: &mut DocumentMetadata, node_id: NodeId, responses: &mut VecDeque<Message>, reconnect: bool) -> bool {
		let Some(network) = document_network.nested_network_mut(&self.network) else {
			return false;
		};
		if !Self::remove_references_from_network(network, node_id, reconnect) {
			return false;
		}
		network.nodes.remove(&node_id);
		document_metadata.retain_selected_nodes(|&id| id != node_id);
		responses.add(BroadcastEvent::SelectionChanged);
		true
	}

	/// Gets the default node input based on the node name and the input index
	pub fn default_node_input(name: String, index: usize) -> Option<NodeInput> {
		resolve_document_node_type(&name)
			.and_then(|node| node.inputs.get(index))
			.map(|input: &DocumentInputType| input.default.clone())
	}

	/// Returns an iterator of nodes to be copied and their ids, excluding output and input nodes
	pub fn copy_nodes<'a>(network: &'a NodeNetwork, new_ids: &'a HashMap<NodeId, NodeId>) -> impl Iterator<Item = (NodeId, DocumentNode)> + 'a {
		new_ids
			.iter()
			.filter(|&(&id, _)| !network.outputs_contain(id))
			.filter_map(|(&id, &new)| network.nodes.get(&id).map(|node| (new, node.clone())))
			.map(move |(new, node)| (new, node.map_ids(Self::default_node_input, new_ids)))
	}
}

#[derive(Debug)]
pub struct NodeGraphHandlerData<'a> {
	pub document_network: &'a mut NodeNetwork,
	pub document_metadata: &'a mut DocumentMetadata,
	pub document_id: u64,
	pub document_name: &'a str,
	pub collapsed: &'a mut Vec<LayerNodeIdentifier>,
	pub input: &'a InputPreprocessorMessageHandler,
	pub graph_view_overlay_open: bool,
}

impl<'a> MessageHandler<NodeGraphMessage, NodeGraphHandlerData<'a>> for NodeGraphMessageHandler {
	fn process_message(&mut self, message: NodeGraphMessage, responses: &mut VecDeque<Message>, data: NodeGraphHandlerData<'a>) {
		let NodeGraphHandlerData {
			document_network,
			document_metadata: metadata,
			document_id,
			collapsed,
			graph_view_overlay_open,
			..
		} = data;
		match message {
			// TODO: automatically remove broadcast messages.
			NodeGraphMessage::Init => {
				responses.add(BroadcastMessage::SubscribeEvent {
					on: BroadcastEvent::SelectionChanged,
					send: Box::new(NodeGraphMessage::SelectedNodesUpdated.into()),
				});
				load_network_structure(document_network, metadata, collapsed);
				responses.add(DocumentMessage::DocumentStructureChanged);
			}
			NodeGraphMessage::SelectedNodesUpdated => {
				self.update_selection_action_buttons(document_network, metadata, responses);
				self.update_selected(document_network, metadata, responses);
				if metadata.selected_layers().count() <= 1 {
					responses.add(DocumentMessage::SetRangeSelectionLayer {
						new_layer: metadata.selected_layers().next(),
					});
				}
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::ConnectNodesByLink {
				output_node,
				output_node_connector_index,
				input_node,
				input_node_connector_index,
			} => {
				let node_id = input_node;

				let Some(network) = document_network.nested_network(&self.network) else {
					error!("No network");
					return;
				};
				let Some(input_node) = network.nodes.get(&input_node) else {
					error!("No to");
					return;
				};
				let Some((input_index, _)) = input_node.inputs.iter().enumerate().filter(|input| input.1.is_exposed()).nth(input_node_connector_index) else {
					error!("Failed to find actual index of connector index {input_node_connector_index} on node {input_node:#?}");
					return;
				};
				responses.add(DocumentMessage::DocumentStructureChanged);

				responses.add(DocumentMessage::StartTransaction);

				let input = NodeInput::node(output_node, output_node_connector_index);
				responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });

				let should_rerender = network.connected_to_output(node_id);
				responses.add(NodeGraphMessage::SendGraph { should_rerender });
			}
			NodeGraphMessage::Copy => {
				let Some(network) = document_network.nested_network(&self.network) else {
					error!("No network");
					return;
				};

				// Collect the selected nodes
				let new_ids = &metadata.selected_nodes().copied().enumerate().map(|(new, old)| (old, new as NodeId)).collect();
				let copied_nodes: Vec<_> = Self::copy_nodes(network, new_ids).collect();

				// Prefix to show that this is nodes
				let mut copy_text = String::from("graphite/nodes: ");
				copy_text += &serde_json::to_string(&copied_nodes).expect("Could not serialize paste");

				responses.add(FrontendMessage::TriggerTextCopy { copy_text });
			}
			NodeGraphMessage::CreateNode { node_id, node_type, x, y } => {
				let node_id = node_id.unwrap_or_else(crate::application::generate_uuid);

				let Some(document_node_type) = document_node_types::resolve_document_node_type(&node_type) else {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Cannot insert node".to_string(),
						description: format!("The document node '{node_type}' does not exist in the document node list"),
					});
					return;
				};

				responses.add(DocumentMessage::StartTransaction);

				let document_node = document_node_type.to_document_node(
					document_node_type.inputs.iter().map(|input| input.default.clone()),
					graph_craft::document::DocumentNodeMetadata::position((x, y)),
				);
				responses.add(NodeGraphMessage::InsertNode { node_id, document_node });

				responses.add(NodeGraphMessage::SendGraph { should_rerender: false });
			}
			NodeGraphMessage::Cut => {
				responses.add(NodeGraphMessage::Copy);
				responses.add(NodeGraphMessage::DeleteSelectedNodes { reconnect: true });
			}
			NodeGraphMessage::DeleteNode { node_id, reconnect } => {
				self.remove_node(document_network, metadata, node_id, responses, reconnect);
			}
			NodeGraphMessage::DeleteSelectedNodes { reconnect } => {
				responses.add(DocumentMessage::StartTransaction);

				for node_id in metadata.selected_nodes().copied() {
					responses.add(NodeGraphMessage::DeleteNode { node_id, reconnect });
				}

				responses.add(NodeGraphMessage::SendGraph { should_rerender: false });

				if let Some(network) = document_network.nested_network(&self.network) {
					// Only generate node graph if one of the selected nodes is connected to the output
					if metadata.selected_nodes().any(|&node_id| network.connected_to_output(node_id)) {
						if let Some(layer_path) = self.layer_path.clone() {
							responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path });
						} else {
							responses.add(NodeGraphMessage::RunDocumentGraph);
						}
					}
				}
			}
			NodeGraphMessage::DisconnectNodes { node_id, input_index } => {
				let Some(network) = document_network.nested_network(&self.network) else {
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

				responses.add(DocumentMessage::StartTransaction);

				let Some((input_index, existing_input)) = node.inputs.iter().enumerate().filter(|(_, input)| input.is_exposed()).nth(input_index) else {
					return;
				};
				let mut input = node_type.inputs[input_index].default.clone();
				if let NodeInput::Value { exposed, .. } = &mut input {
					*exposed = existing_input.is_exposed();
				}
				responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });

				let should_rerender = network.connected_to_output(node_id);
				responses.add(NodeGraphMessage::SendGraph { should_rerender });
			}
			NodeGraphMessage::DoubleClickNode { node } => {
				if let Some(network) = document_network.nested_network(&self.network) {
					if network.nodes.get(&node).and_then(|node| node.implementation.get_network()).is_some() {
						self.network.push(node);
					}
				}
				if let Some(network) = document_network.nested_network(&self.network) {
					Self::send_graph(network, &self.layer_path, graph_view_overlay_open, responses);
				}
				self.update_selected(document_network, metadata, responses);
			}
			NodeGraphMessage::DuplicateSelectedNodes => {
				if let Some(network) = document_network.nested_network(&self.network) {
					responses.add(DocumentMessage::StartTransaction);

					let new_ids = &metadata.selected_nodes().map(|&id| (id, crate::application::generate_uuid())).collect();

					metadata.clear_selected_nodes();
					responses.add(BroadcastEvent::SelectionChanged);

					// Copy the selected nodes
					let copied_nodes = Self::copy_nodes(network, new_ids).collect::<Vec<_>>();

					// Select the new nodes
					metadata.add_selected_nodes(copied_nodes.iter().map(|(node_id, _)| *node_id));
					responses.add(BroadcastEvent::SelectionChanged);

					for (node_id, mut document_node) in copied_nodes {
						// Shift duplicated node
						document_node.metadata.position += IVec2::splat(2);

						// Insert new node into graph
						responses.add(NodeGraphMessage::InsertNode { node_id, document_node });
					}

					Self::send_graph(network, &self.layer_path, graph_view_overlay_open, responses);
					self.update_selected(document_network, metadata, responses);
					responses.add(NodeGraphMessage::SendGraph { should_rerender: false });
				}
			}
			NodeGraphMessage::ExitNestedNetwork { depth_of_nesting } => {
				metadata.clear_selected_nodes();
				responses.add(BroadcastEvent::SelectionChanged);

				for _ in 0..depth_of_nesting {
					self.network.pop();
				}
				if let Some(network) = document_network.nested_network(&self.network) {
					Self::send_graph(network, &self.layer_path, graph_view_overlay_open, responses);
				}
				self.update_selected(document_network, metadata, responses);
			}
			NodeGraphMessage::ExposeInput { node_id, input_index, new_exposed } => {
				let Some(network) = document_network.nested_network(&self.network) else {
					warn!("No network");
					return;
				};

				let Some(node) = network.nodes.get(&node_id) else {
					warn!("No node");
					return;
				};

				responses.add(DocumentMessage::StartTransaction);

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
				responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });

				let should_rerender = network.connected_to_output(node_id);
				responses.add(NodeGraphMessage::SendGraph { should_rerender });
				responses.add(PropertiesPanelMessage::Refresh);
			}
			NodeGraphMessage::InsertNode { node_id, document_node } => {
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					network.nodes.insert(node_id, document_node);
				}
			}
			NodeGraphMessage::MoveSelectedNodes { displacement_x, displacement_y } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else {
					warn!("No network");
					return;
				};

				for node_id in metadata.selected_nodes() {
					if let Some(node) = network.nodes.get_mut(node_id) {
						node.metadata.position += IVec2::new(displacement_x, displacement_y)
					}
				}
				Self::send_graph(network, &self.layer_path, graph_view_overlay_open, responses);
			}
			NodeGraphMessage::PasteNodes { serialized_nodes } => {
				let Some(network) = document_network.nested_network(&self.network) else {
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

				if data.is_empty() {
					return;
				}

				// Shift nodes until it is not in the same position as another node
				let mut shift = IVec2::ZERO;
				while data
					.iter()
					.all(|(_, node)| network.nodes.values().any(|existing_node| node.metadata.position + shift == existing_node.metadata.position))
				{
					shift += IVec2::splat(2);
				}

				responses.add(DocumentMessage::StartTransaction);

				let new_ids: HashMap<_, _> = data.iter().map(|&(id, _)| (id, crate::application::generate_uuid())).collect();
				for (old_id, mut document_node) in data {
					// Shift copied node
					document_node.metadata.position += shift;

					// Get the new, non-conflicting id
					let node_id = *new_ids.get(&old_id).unwrap();
					document_node = document_node.map_ids(Self::default_node_input, &new_ids);

					// Insert node into network
					responses.add(NodeGraphMessage::InsertNode { node_id, document_node });
				}

				let nodes = new_ids.values().copied().collect();
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes });

				responses.add(NodeGraphMessage::SendGraph { should_rerender: false });
			}
			NodeGraphMessage::RunDocumentGraph => responses.add(PortfolioMessage::SubmitGraphRender { document_id, layer_path: Vec::new() }),
			NodeGraphMessage::SelectedNodesAdd { nodes } => {
				metadata.add_selected_nodes(nodes);
				responses.add(BroadcastEvent::SelectionChanged);
			}
			NodeGraphMessage::SelectedNodesRemove { nodes } => {
				metadata.retain_selected_nodes(|node| !nodes.contains(node));
				responses.add(BroadcastEvent::SelectionChanged);
			}
			NodeGraphMessage::SelectedNodesSet { nodes } => {
				metadata.set_selected_nodes(nodes);
				responses.add(BroadcastEvent::SelectionChanged);
				responses.add(PropertiesPanelMessage::Refresh);
			}
			NodeGraphMessage::SendGraph { should_rerender } => {
				if let Some(network) = document_network.nested_network(&self.network) {
					Self::send_graph(network, &self.layer_path, graph_view_overlay_open, responses);
					if should_rerender {
						if let Some(layer_path) = self.layer_path.clone() {
							responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path });
						} else {
							responses.add(NodeGraphMessage::RunDocumentGraph);
						}
					}
				}
			}
			NodeGraphMessage::SetInputValue { node_id, input_index, value } => {
				if let Some(network) = document_network.nested_network(&self.network) {
					if let Some(node) = network.nodes.get(&node_id) {
						responses.add(DocumentMessage::StartTransaction);

						let input = NodeInput::Value { tagged_value: value, exposed: false };
						responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });
						responses.add(PropertiesPanelMessage::Refresh);
						if (node.name != "Imaginate" || input_index == 0) && network.connected_to_output(node_id) {
							if let Some(layer_path) = self.layer_path.clone() {
								responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path });
							} else {
								responses.add(NodeGraphMessage::RunDocumentGraph);
							}
						}
					}
				}
			}
			NodeGraphMessage::SetNodeInput { node_id, input_index, input } => {
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					if let Some(node) = network.nodes.get_mut(&node_id) {
						let Some(node_input) = node.inputs.get_mut(input_index) else {
							error!("Tried to set input {input_index} to {input:?}, but the index was invalid. Node {node_id}:\n{node:#?}");
							return;
						};
						let structure_changed = node_input.as_node().is_some() || input.as_node().is_some();
						*node_input = input;
						if structure_changed {
							load_network_structure(document_network, metadata, collapsed);
						}
					}
				}
			}
			NodeGraphMessage::SetQualifiedInputValue {
				layer_path,
				node_path,
				input_index,
				value,
			} => {
				let Some((node_id, node_path)) = node_path.split_last() else {
					error!("Node path is empty");
					return;
				};

				let network = document_network.nested_network_mut(node_path);

				if let Some(network) = network {
					if let Some(node) = network.nodes.get_mut(node_id) {
						// Extend number of inputs if not already large enough
						if input_index >= node.inputs.len() {
							node.inputs.extend(((node.inputs.len() - 1)..input_index).map(|_| NodeInput::Network(generic!(T))));
						}
						node.inputs[input_index] = NodeInput::Value { tagged_value: value, exposed: false };
						if network.connected_to_output(*node_id) {
							responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path });
						}
					}
				}
			}
			NodeGraphMessage::ShiftNode { node_id } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else {
					warn!("No network");
					return;
				};
				debug_assert!(network.is_acyclic(), "Not acyclic. Network: {network:#?}");
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
					.filter_map(|input| if let NodeInput::Node { node_id: previous_id, .. } = input { Some(*previous_id) } else { None })
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
				responses.add(NodeGraphMessage::SendGraph { should_rerender: false });
			}
			NodeGraphMessage::ToggleSelectedHidden => {
				if let Some(network) = document_network.nested_network(&self.network) {
					responses.add(DocumentMessage::StartTransaction);

					let new_hidden = !metadata.selected_nodes().any(|id| network.disabled.contains(id));
					for &node_id in metadata.selected_nodes() {
						responses.add(NodeGraphMessage::SetHidden { node_id, hidden: new_hidden });
					}
				}
			}
			NodeGraphMessage::ToggleHidden { node_id } => {
				if let Some(network) = document_network.nested_network(&self.network) {
					let new_hidden = !network.disabled.contains(&node_id);
					responses.add(NodeGraphMessage::SetHidden { node_id, hidden: new_hidden });
				}
			}
			NodeGraphMessage::SetHidden { node_id, hidden } => {
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					if !hidden {
						network.disabled.retain(|&id| node_id != id);
					} else if !network.inputs.contains(&node_id) && !network.original_outputs().iter().any(|output| output.node_id == node_id) {
						network.disabled.push(node_id);
					}
					Self::send_graph(network, &self.layer_path, graph_view_overlay_open, responses);

					// Only generate node graph if one of the selected nodes is connected to the output
					if network.connected_to_output(node_id) {
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				}
				self.update_selection_action_buttons(document_network, metadata, responses);
			}
			NodeGraphMessage::SetName { node_id, name } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetNameImpl { node_id, name });
			}
			NodeGraphMessage::SetNameImpl { node_id, name } => {
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					if let Some(node) = network.nodes.get_mut(&node_id) {
						node.alias = name;
						responses.add(NodeGraphMessage::SendGraph { should_rerender: false });
					}
				}
			}
			NodeGraphMessage::TogglePreview { node_id } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::TogglePreviewImpl { node_id });
			}
			NodeGraphMessage::TogglePreviewImpl { node_id } => {
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					// Check if the node is not already being previewed
					if !network.outputs_contain(node_id) {
						network.previous_outputs = Some(network.previous_outputs.to_owned().unwrap_or_else(|| network.outputs.clone()));
						network.outputs[0] = NodeOutput::new(node_id, 0);
					} else if let Some(outputs) = network.previous_outputs.take() {
						network.outputs = outputs
					} else {
						return;
					}
					Self::send_graph(network, &self.layer_path, graph_view_overlay_open, responses);
				}
				self.update_selection_action_buttons(document_network, metadata, responses);
				if let Some(layer_path) = self.layer_path.clone() {
					responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path });
				} else {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
			}
			NodeGraphMessage::UpdateNewNodeGraph => {
				if let Some(network) = document_network.nested_network(&self.network) {
					metadata.clear_selected_nodes();
					responses.add(BroadcastEvent::SelectionChanged);

					Self::send_graph(network, &self.layer_path, graph_view_overlay_open, responses);

					let node_types = document_node_types::collect_node_types();
					responses.add(FrontendMessage::UpdateNodeTypes { node_types });
				}
				self.update_selected(document_network, metadata, responses);
			}
		}
		self.has_selection = metadata.has_selected_nodes();
	}

	fn actions(&self) -> ActionList {
		unimplemented!("Must use `actions_with_graph_open` instead (unless we change every implementation of the MessageHandler trait).")
	}
}

impl NodeGraphMessageHandler {
	pub fn actions_with_node_graph_open(&self, graph_open: bool) -> ActionList {
		if self.has_selection && graph_open {
			actions!(NodeGraphMessageDiscriminant; DeleteSelectedNodes, Cut, Copy, DuplicateSelectedNodes, ToggleSelectedHidden)
		} else {
			actions!(NodeGraphMessageDiscriminant;)
		}
	}
}
