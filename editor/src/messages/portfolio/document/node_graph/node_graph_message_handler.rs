pub use self::document_node_types::*;
use super::load_network_structure;
use crate::application::generate_uuid;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, LayerClassification, LayerPanelEntry, SelectedNodes};
use crate::messages::prelude::*;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput, NodeNetwork, NodeOutput, Source};
use graph_craft::proto::GraphErrors;
use graphene_core::*;
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypes;
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
	#[serde(rename = "general")]
	Text,
	#[serde(rename = "vector")]
	Subpath,
	#[serde(rename = "number")]
	Number,
	#[serde(rename = "general")]
	Boolean,
	/// Refers to the mathematical vector, with direction and magnitude.
	#[serde(rename = "number")]
	Vector,
	#[serde(rename = "raster")]
	GraphicGroup,
	#[serde(rename = "artboard")]
	Artboard,
	#[serde(rename = "color")]
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
	#[serde(rename = "resolvedType")]
	resolved_type: Option<String>,
	connected: Option<NodeId>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendGraphOutput {
	#[serde(rename = "dataType")]
	data_type: FrontendGraphDataType,
	name: String,
	#[serde(rename = "resolvedType")]
	resolved_type: Option<String>,
	connected: Option<NodeId>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNode {
	pub id: graph_craft::document::NodeId,
	#[serde(rename = "isLayer")]
	pub is_layer: bool,
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
	pub errors: Option<String>,
}

// (link_start, link_end, link_end_input_index)
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendNodeLink {
	#[serde(rename = "linkStart")]
	pub link_start: NodeId,
	#[serde(rename = "linkStartOutputIndex")]
	pub link_start_output_index: usize,
	#[serde(rename = "linkEnd")]
	pub link_end: NodeId,
	#[serde(rename = "linkEndInputIndex")]
	pub link_end_input_index: usize,
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
	pub network: Vec<NodeId>,
	pub resolved_types: ResolvedDocumentNodeTypes,
	pub node_graph_errors: GraphErrors,
	has_selection: bool,
	widgets: [LayoutGroup; 2],
}

impl Default for NodeGraphMessageHandler {
	fn default() -> Self {
		let right_side_widgets = vec![
			// TODO: Replace this with an "Add Node" button, also next to an "Add Layer" button
			TextLabel::new("Right Click in Graph to Add Nodes").italic(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextButton::new("Node Graph")
				.icon(Some("GraphViewOpen".into()))
				.tooltip("Hide Node Graph")
				.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::GraphViewOverlayToggle))
				.on_update(move |_| DocumentMessage::GraphViewOverlayToggle.into())
				.widget_holder(),
		];

		Self {
			network: Vec::new(),
			resolved_types: ResolvedDocumentNodeTypes::default(),
			node_graph_errors: Vec::new(),
			has_selection: false,
			widgets: [LayoutGroup::Row { widgets: Vec::new() }, LayoutGroup::Row { widgets: right_side_widgets }],
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
	fn update_selection_action_buttons(&mut self, document_network: &NodeNetwork, selected_nodes: &SelectedNodes, responses: &mut VecDeque<Message>) {
		if let Some(network) = document_network.nested_network(&self.network) {
			let mut widgets = Vec::new();

			// Don't allow disabling input or output nodes
			let mut selection = selected_nodes.selected_nodes().filter(|&&id| !network.inputs.contains(&id) && !network.original_outputs_contain(id));

			// If there is at least one other selected node then show the hide or show button
			if selection.next().is_some() {
				// Check if any of the selected nodes are disabled
				let is_hidden = selected_nodes.selected_nodes().any(|id| network.disabled.contains(id));

				// Check if multiple nodes are selected
				let multiple_nodes = selection.next().is_some();

				// Generate the enable or disable button accordingly
				let (hide_show_label, hide_show_icon) = if is_hidden { ("Make Visible", "EyeHidden") } else { ("Make Hidden", "EyeVisible") };
				let hide_button = TextButton::new(hide_show_label)
					.icon(Some(hide_show_icon.to_string()))
					.tooltip(if is_hidden { "Show selected nodes/layers" } else { "Hide selected nodes/layers" }.to_string() + if multiple_nodes { "s" } else { "" })
					.tooltip_shortcut(action_keys!(NodeGraphMessageDiscriminant::ToggleSelectedHidden))
					.on_update(move |_| NodeGraphMessage::ToggleSelectedHidden.into())
					.widget_holder();
				widgets.push(hide_button);

				widgets.push(Separator::new(SeparatorType::Related).widget_holder());
			}

			// If only one node is selected then show the preview or stop previewing button
			let mut selection = selected_nodes.selected_nodes();
			if let (Some(&node_id), None) = (selection.next(), selection.next()) {
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

			self.widgets[0] = LayoutGroup::Row { widgets };
		}
		self.send_node_bar_layout(responses);
	}

	/// Collate the properties panel sections for a node graph
	pub fn collate_properties(&self, context: &mut NodePropertiesContext, selected_nodes: &SelectedNodes) -> Vec<LayoutGroup> {
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
		for node_id in selected_nodes.selected_nodes() {
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

	fn collect_links(network: &NodeNetwork) -> Vec<FrontendNodeLink> {
		network
			.nodes
			.iter()
			.flat_map(|(link_end, node)| node.inputs.iter().filter(|input| input.is_exposed()).enumerate().map(move |(index, input)| (input, link_end, index)))
			.filter_map(|(input, &link_end, link_end_input_index)| {
				if let NodeInput::Node {
					node_id: link_start,
					output_index: link_start_output_index,
					// TODO: add ui for lambdas
					lambda: _,
				} = *input
				{
					Some(FrontendNodeLink {
						link_start,
						link_start_output_index,
						link_end,
						link_end_input_index,
					})
				} else {
					None
				}
			})
			.collect::<Vec<_>>()
	}

	fn collect_nodes(&self, links: &[FrontendNodeLink], network: &NodeNetwork) -> Vec<FrontendNode> {
		let connected_node_to_output_lookup = links.iter().map(|link| ((link.link_start, link.link_start_output_index), link.link_end)).collect::<HashMap<_, _>>();

		let mut nodes = Vec::new();
		for (&node_id, node) in &network.nodes {
			let node_path = vec![node_id];
			// TODO: This should be based on the graph runtime type inference system in order to change the colors of node connectors to match the data type in use
			let Some(document_node_definition) = document_node_types::resolve_document_node_type(&node.name) else {
				warn!("Node '{}' does not exist in library", node.name);
				continue;
			};

			// Inputs
			let mut inputs = {
				let frontend_graph_inputs = document_node_definition.inputs.iter().enumerate().map(|(index, input_type)| {
					// Convert the index in all inputs to the index in only the exposed inputs
					let index = node.inputs.iter().take(index).filter(|input| input.is_exposed()).count();

					FrontendGraphInput {
						data_type: input_type.data_type,
						name: input_type.name.to_string(),
						resolved_type: self.resolved_types.inputs.get(&Source { node: node_path.clone(), index }).map(|input| format!("{input:?}")),
						connected: None,
					}
				});

				node.inputs.iter().zip(frontend_graph_inputs).map(|(node_input, mut frontend_graph_input)| {
					if let NodeInput::Node { node_id: connected_node_id, .. } = node_input {
						frontend_graph_input.connected = Some(*connected_node_id);
					}
					(node_input, frontend_graph_input)
				})
			};
			let primary_input = inputs.next().filter(|(input, _)| input.is_exposed()).map(|(_, input_type)| input_type);
			let exposed_inputs = inputs.filter(|(input, _)| input.is_exposed()).map(|(_, input_type)| input_type).collect();

			// Outputs
			let mut outputs = document_node_definition.outputs.iter().enumerate().map(|(index, output_type)| FrontendGraphOutput {
				data_type: output_type.data_type,
				name: output_type.name.to_string(),
				resolved_type: self.resolved_types.outputs.get(&Source { node: node_path.clone(), index }).map(|output| format!("{output:?}")),
				connected: connected_node_to_output_lookup.get(&(node_id, index)).copied(),
			});
			let primary_output = node.has_primary_output.then(|| outputs.next()).flatten();
			let exposed_outputs = outputs.collect::<Vec<_>>();

			// Errors
			let errors = self.node_graph_errors.iter().find(|error| error.node_path.starts_with(&node_path)).map(|error| error.error.clone());

			nodes.push(FrontendNode {
				id: node_id,
				is_layer: node.is_layer(),
				alias: node.alias.clone(),
				name: node.name.clone(),
				primary_input,
				exposed_inputs,
				primary_output,
				exposed_outputs,
				position: node.metadata.position.into(),
				previewed: network.outputs_contain(node_id),
				disabled: network.disabled.contains(&node_id),
				errors: errors.map(|e| format!("{e:?}")),
			});
		}
		nodes
	}

	fn update_layer_panel(network: &NodeNetwork, metadata: &DocumentMetadata, collapsed: &CollapsedLayers, responses: &mut VecDeque<Message>) {
		for (&node_id, node) in &network.nodes {
			if node.is_layer() {
				let layer = LayerNodeIdentifier::new(node_id, network);
				let layer_classification = {
					if metadata.is_artboard(layer) {
						LayerClassification::Artboard
					} else if metadata.is_folder(layer) {
						LayerClassification::Folder
					} else {
						LayerClassification::Layer
					}
				};

				let data = LayerPanelEntry {
					id: node_id,
					layer_classification,
					expanded: layer.has_children(metadata) && !collapsed.0.contains(&layer),
					depth: layer.ancestors(metadata).count() - 1,
					parent_id: layer.parent(metadata).map(|parent| parent.to_node()),
					name: network.nodes.get(&node_id).map(|node| node.alias.clone()).unwrap_or_default(),
					tooltip: if cfg!(debug_assertions) { format!("Layer ID: {node_id}") } else { "".into() },
					disabled: network.disabled.contains(&node_id),
				};
				responses.add(FrontendMessage::UpdateDocumentLayerDetails { data });
			}
		}
	}

	fn send_graph(&self, network: &NodeNetwork, graph_open: bool, metadata: &mut DocumentMetadata, selected_nodes: &mut SelectedNodes, collapsed: &CollapsedLayers, responses: &mut VecDeque<Message>) {
		metadata.load_structure(network, selected_nodes);

		responses.add(DocumentMessage::DocumentStructureChanged);
		responses.add(PropertiesPanelMessage::Refresh);
		Self::update_layer_panel(network, metadata, collapsed, responses);
		if graph_open {
			let links = Self::collect_links(network);
			let nodes = self.collect_nodes(&links, network);
			responses.add(FrontendMessage::UpdateNodeGraph { nodes, links });
		}
	}

	/// Updates the frontend's selection state in line with the backend
	fn update_selected(&mut self, document_network: &NodeNetwork, selected_nodes: &SelectedNodes, responses: &mut VecDeque<Message>) {
		self.update_selection_action_buttons(document_network, selected_nodes, responses);
		responses.add(FrontendMessage::UpdateNodeGraphSelection {
			selected: selected_nodes.selected_nodes_ref().clone(),
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
	fn remove_node(&mut self, document_network: &mut NodeNetwork, selected_nodes: &mut SelectedNodes, node_id: NodeId, responses: &mut VecDeque<Message>, reconnect: bool) -> bool {
		let Some(network) = document_network.nested_network_mut(&self.network) else {
			return false;
		};
		if !Self::remove_references_from_network(network, node_id, reconnect) {
			return false;
		}
		network.nodes.remove(&node_id);
		selected_nodes.retain_selected_nodes(|&id| id != node_id);
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
	pub selected_nodes: &'a mut SelectedNodes,
	pub document_id: DocumentId,
	pub document_name: &'a str,
	pub collapsed: &'a mut CollapsedLayers,
	pub input: &'a InputPreprocessorMessageHandler,
	pub graph_view_overlay_open: bool,
}

impl<'a> MessageHandler<NodeGraphMessage, NodeGraphHandlerData<'a>> for NodeGraphMessageHandler {
	fn process_message(&mut self, message: NodeGraphMessage, responses: &mut VecDeque<Message>, data: NodeGraphHandlerData<'a>) {
		let NodeGraphHandlerData {
			document_network,
			document_metadata,
			selected_nodes,
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
				load_network_structure(document_network, document_metadata, selected_nodes, collapsed);
			}
			NodeGraphMessage::SelectedNodesUpdated => {
				self.update_selection_action_buttons(document_network, selected_nodes, responses);
				self.update_selected(document_network, selected_nodes, responses);
				if selected_nodes.selected_layers(document_metadata).count() <= 1 {
					responses.add(DocumentMessage::SetRangeSelectionLayer {
						new_layer: selected_nodes.selected_layers(document_metadata).next(),
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
				let Some(input_node) = network.nodes.get(&node_id) else {
					error!("No to");
					return;
				};
				let Some((input_index, _)) = input_node.inputs.iter().enumerate().filter(|input| input.1.is_exposed()).nth(input_node_connector_index) else {
					error!("Failed to find actual index of connector index {input_node_connector_index} on node {input_node:#?}");
					return;
				};

				responses.add(DocumentMessage::StartTransaction);

				let input = NodeInput::node(output_node, output_node_connector_index);
				responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });

				if network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::Copy => {
				let Some(network) = document_network.nested_network(&self.network) else {
					error!("No network");
					return;
				};

				// Collect the selected nodes
				let new_ids = &selected_nodes.selected_nodes().copied().enumerate().map(|(new, old)| (old, NodeId(new as u64))).collect();
				let copied_nodes: Vec<_> = Self::copy_nodes(network, new_ids).collect();

				// Prefix to show that this is nodes
				let mut copy_text = String::from("graphite/nodes: ");
				copy_text += &serde_json::to_string(&copied_nodes).expect("Could not serialize paste");

				responses.add(FrontendMessage::TriggerTextCopy { copy_text });
			}
			NodeGraphMessage::CreateNode { node_id, node_type, x, y } => {
				let node_id = node_id.unwrap_or_else(|| NodeId(generate_uuid()));

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
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::Cut => {
				responses.add(NodeGraphMessage::Copy);
				responses.add(NodeGraphMessage::DeleteSelectedNodes { reconnect: true });
			}
			NodeGraphMessage::DeleteNode { node_id, reconnect } => {
				self.remove_node(document_network, selected_nodes, node_id, responses, reconnect);
			}
			NodeGraphMessage::DeleteSelectedNodes { reconnect } => {
				responses.add(DocumentMessage::StartTransaction);

				for node_id in selected_nodes.selected_nodes().copied() {
					responses.add(NodeGraphMessage::DeleteNode { node_id, reconnect });
				}

				if let Some(network) = document_network.nested_network(&self.network) {
					// Only generate node graph if one of the selected nodes is connected to the output
					if selected_nodes.selected_nodes().any(|&node_id| network.connected_to_output(node_id)) {
						responses.add(NodeGraphMessage::RunDocumentGraph);
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

				if network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::EnterNestedNetwork { node } => {
				if let Some(network) = document_network.nested_network(&self.network) {
					if network.nodes.get(&node).and_then(|node| node.implementation.get_network()).is_some() {
						self.network.push(node);
					}
				}
				if let Some(network) = document_network.nested_network(&self.network) {
					self.send_graph(network, graph_view_overlay_open, document_metadata, selected_nodes, collapsed, responses);
				}
				self.update_selected(document_network, selected_nodes, responses);
			}
			NodeGraphMessage::DuplicateSelectedNodes => {
				if let Some(network) = document_network.nested_network(&self.network) {
					responses.add(DocumentMessage::StartTransaction);

					let new_ids = &selected_nodes.selected_nodes().map(|&id| (id, NodeId(generate_uuid()))).collect();

					selected_nodes.clear_selected_nodes();
					responses.add(BroadcastEvent::SelectionChanged);

					// Copy the selected nodes
					let copied_nodes = Self::copy_nodes(network, new_ids).collect::<Vec<_>>();

					// Select the new nodes
					selected_nodes.add_selected_nodes(copied_nodes.iter().map(|(node_id, _)| *node_id));
					responses.add(BroadcastEvent::SelectionChanged);

					for (node_id, mut document_node) in copied_nodes {
						// Shift duplicated node
						document_node.metadata.position += IVec2::splat(2);

						// Insert new node into graph
						responses.add(NodeGraphMessage::InsertNode { node_id, document_node });
					}

					self.update_selected(document_network, selected_nodes, responses);
				}
			}
			NodeGraphMessage::ExitNestedNetwork { depth_of_nesting } => {
				selected_nodes.clear_selected_nodes();
				responses.add(BroadcastEvent::SelectionChanged);

				for _ in 0..depth_of_nesting {
					self.network.pop();
				}
				if let Some(network) = document_network.nested_network(&self.network) {
					self.send_graph(network, graph_view_overlay_open, document_metadata, selected_nodes, collapsed, responses);
				}
				self.update_selected(document_network, selected_nodes, responses);
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

				responses.add(PropertiesPanelMessage::Refresh);
				responses.add(NodeGraphMessage::SendGraph);
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

				for node_id in selected_nodes.selected_nodes() {
					if let Some(node) = network.nodes.get_mut(node_id) {
						node.metadata.position += IVec2::new(displacement_x, displacement_y)
					}
				}
				self.send_graph(network, graph_view_overlay_open, document_metadata, selected_nodes, collapsed, responses);
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

				let new_ids: HashMap<_, _> = data.iter().map(|&(id, _)| (id, NodeId(generate_uuid()))).collect();
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
			}
			NodeGraphMessage::RunDocumentGraph => {
				responses.add(PortfolioMessage::SubmitGraphRender { document_id });
			}
			NodeGraphMessage::SelectedNodesAdd { nodes } => {
				selected_nodes.add_selected_nodes(nodes);
				responses.add(BroadcastEvent::SelectionChanged);
			}
			NodeGraphMessage::SelectedNodesRemove { nodes } => {
				selected_nodes.retain_selected_nodes(|node| !nodes.contains(node));
				responses.add(BroadcastEvent::SelectionChanged);
			}
			NodeGraphMessage::SelectedNodesSet { nodes } => {
				selected_nodes.set_selected_nodes(nodes);
				responses.add(BroadcastEvent::SelectionChanged);
				responses.add(PropertiesPanelMessage::Refresh);
			}
			NodeGraphMessage::SendGraph => {
				if let Some(network) = document_network.nested_network(&self.network) {
					self.send_graph(network, graph_view_overlay_open, document_metadata, selected_nodes, collapsed, responses);
				}
			}
			NodeGraphMessage::SetInputValue { node_id, input_index, value } => {
				if let Some(network) = document_network.nested_network(&self.network) {
					if let Some(node) = network.nodes.get(&node_id) {
						let input = NodeInput::Value { tagged_value: value, exposed: false };
						responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });
						responses.add(PropertiesPanelMessage::Refresh);
						if (node.name != "Imaginate" || input_index == 0) && network.connected_to_output(node_id) {
							responses.add(NodeGraphMessage::RunDocumentGraph);
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
							load_network_structure(document_network, document_metadata, selected_nodes, collapsed);
						}
					}
				}
			}
			NodeGraphMessage::SetQualifiedInputValue { node_path, input_index, value } => {
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
							responses.add(NodeGraphMessage::RunDocumentGraph);
						}
					}
				}
			}
			// Move all the downstream nodes to the right in the graph to allow space for a newly inserted node
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

				self.send_graph(network, graph_view_overlay_open, document_metadata, selected_nodes, collapsed, responses);
			}
			NodeGraphMessage::ToggleSelectedHidden => {
				if let Some(network) = document_network.nested_network(&self.network) {
					responses.add(DocumentMessage::StartTransaction);

					let new_hidden = !selected_nodes.selected_nodes().any(|id| network.disabled.contains(id));
					for &node_id in selected_nodes.selected_nodes() {
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

					// Only generate node graph if one of the selected nodes is connected to the output
					if network.connected_to_output(node_id) {
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				}
				self.update_selection_action_buttons(document_network, selected_nodes, responses);
			}
			NodeGraphMessage::SetName { node_id, name } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetNameImpl { node_id, name });
			}
			NodeGraphMessage::SetNameImpl { node_id, name } => {
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					if let Some(node) = network.nodes.get_mut(&node_id) {
						node.alias = name;

						self.send_graph(network, graph_view_overlay_open, document_metadata, selected_nodes, collapsed, responses);
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
				}

				self.update_selection_action_buttons(document_network, selected_nodes, responses);

				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::UpdateNewNodeGraph => {
				if let Some(network) = document_network.nested_network(&self.network) {
					selected_nodes.clear_selected_nodes();
					responses.add(BroadcastEvent::SelectionChanged);

					self.send_graph(network, graph_view_overlay_open, document_metadata, selected_nodes, collapsed, responses);

					let node_types = document_node_types::collect_node_types();
					responses.add(FrontendMessage::UpdateNodeTypes { node_types });
				}
				self.update_selected(document_network, selected_nodes, responses);
			}
			NodeGraphMessage::UpdateTypes { resolved_types, node_graph_errors } => {
				self.resolved_types = resolved_types;
				self.node_graph_errors = node_graph_errors;
			}
		}
		self.has_selection = selected_nodes.has_selected_nodes();
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
