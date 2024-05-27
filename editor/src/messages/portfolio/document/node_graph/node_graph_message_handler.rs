use graph_craft::document::{DocumentNode, FlowType, NodeId, NodeInput, NodeNetwork, RootNode, Source};
use graph_craft::proto::GraphErrors;
use graphene_core::*;
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypes;

use super::utility_types::{FrontendGraphInput, FrontendGraphOutput, FrontendNode, FrontendNodeLink};
use super::{document_node_types, node_properties};
use crate::application::generate_uuid;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::graph_operation::load_network_structure;
use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::node_graph::document_node_types::{resolve_document_node_type, DocumentInputType, NodePropertiesContext};
use crate::messages::portfolio::document::node_graph::utility_types::FrontendGraphDataType;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, LayerPanelEntry, SelectedNodes};
use crate::messages::prelude::*;
use glam::IVec2;

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

#[derive(Debug, Clone, PartialEq)]
pub struct NodeGraphMessageHandler {
	pub network: Vec<NodeId>,
	pub resolved_types: ResolvedDocumentNodeTypes,
	pub node_graph_errors: GraphErrors,
	has_selection: bool,
	widgets: [LayoutGroup; 2],
}
/// NodeGraphMessageHandler always modifies the nested graph displayed in the Graph UI. No GraphOperationMessages should be added here, since those
/// messages will only affect the document network.
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
				load_network_structure(document_network, document_metadata, collapsed);
			}
			NodeGraphMessage::SelectedNodesUpdated => {
				self.update_selected(document_network, selected_nodes, responses);
				if selected_nodes.selected_layers(document_metadata).count() <= 1 {
					responses.add(DocumentMessage::SetRangeSelectionLayer {
						new_layer: selected_nodes.selected_layers(document_metadata).next(),
					});
				}
				responses.add(ArtboardToolMessage::UpdateSelectedArtboard);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::ConnectNodesByLink {
				output_node,
				output_node_connector_index,
				input_node,
				input_node_connector_index,
			} => {
				let Some(network) = document_network.nested_network(&self.network) else {
					error!("No network");
					return;
				};
				//If output_node_id is none, then it is the UI only Import node
				let output_node_id = if network.imports_metadata.0 == output_node { None } else { Some(output_node) };
				//If input_node is none, then it is the UI only Export node
				let input_node_id = if network.exports_metadata.0 == input_node { None } else { Some(input_node) };

				let input_index = if network.exports_metadata.0 != input_node {
					let Some(input_node) = network.nodes.get(&input_node) else {
						error!("No to");
						return;
					};
					let input_index = input_node
						.inputs
						.iter()
						.enumerate()
						.filter(|input| input.1.is_exposed())
						.nth(input_node_connector_index)
						.map(|enumerated_input| enumerated_input.0);

					let Some(input_index) = input_index else {
						error!("Failed to find actual index of connector index {input_node_connector_index} on node {input_node:#?}");
						return;
					};
					input_index
				} else {
					input_node_connector_index
				};

				responses.add(DocumentMessage::StartTransaction);

				match (output_node_id, input_node_id) {
					// Connecting 2 document nodes
					(Some(output_node_id), Some(input_node_id)) => {
						let input = NodeInput::node(output_node_id, output_node_connector_index);
						responses.add(NodeGraphMessage::SetNodeInput {
							node_id: input_node_id,
							input_index,
							input,
						});

						if network.connected_to_output(input_node_id) {
							responses.add(NodeGraphMessage::RunDocumentGraph);
						}
					}
					// Connecting a document node output to the Export node input
					(Some(output_node_id), None) => {
						let root_node = Some(RootNode {
							id: output_node_id,
							output_index: output_node_connector_index,
						});
						responses.add(NodeGraphMessage::SetRootNode { root_node });
						let input = NodeInput::node(output_node_id, output_node_connector_index);
						responses.add(NodeGraphMessage::SetNodeInput {
							node_id: network.exports_metadata.0,
							input_index,
							input,
						});
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
					// Connecting a document node input to the Import node output
					(None, Some(input_node_id)) => {
						let input = NodeInput::network(generic!(T), output_node_connector_index);
						responses.add(NodeGraphMessage::SetNodeInput {
							node_id: input_node_id,
							input_index,
							input,
						});
						if network.connected_to_output(input_node_id) {
							responses.add(NodeGraphMessage::RunDocumentGraph);
						}
					}
					// Connecting a Export node input to the Import node output
					(None, None) => {
						let input = NodeInput::network(generic!(T), output_node_connector_index);
						responses.add(NodeGraphMessage::SetNodeInput {
							node_id: network.exports_metadata.0,
							input_index,
							input,
						});
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				}

				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::Copy => {
				let Some(network) = document_network.nested_network(&self.network) else {
					error!("No network");
					return;
				};

				// Collect the selected nodes
				let new_ids = &selected_nodes
					.selected_nodes_filtered(network)
					.copied()
					.enumerate()
					.map(|(new, old)| (old, NodeId(new as u64)))
					.collect();
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
			NodeGraphMessage::DeleteNodes { node_ids, reconnect } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else {
					error!("No network");
					return;
				};

				ModifyInputsContext::delete_nodes(network, selected_nodes, node_ids, reconnect, responses);
				// Load structure if the selected network is the document network
				if self.network.is_empty() {
					load_network_structure(document_network, document_metadata, collapsed);
				}

				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			// Deletes selected_nodes. If `reconnect` is true, then all children nodes (secondary input) of the selected nodes are deleted and the siblings (primary input/output) are reconnected.
			// If `reconnect` is false, then only the selected nodes are deleted and not reconnected.
			NodeGraphMessage::DeleteSelectedNodes { reconnect } => {
				let Some(network) = document_network.nested_network(&self.network) else {
					warn!("No network");
					return;
				};
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::DeleteNodes {
					node_ids: selected_nodes.selected_nodes_filtered(network).copied().collect(),
					reconnect,
				});
			}

			NodeGraphMessage::DisconnectInput { node_id, input_index } => {
				responses.add(DocumentMessage::StartTransaction);

				let Some(network) = document_network.nested_network(&self.network) else {
					warn!("No network");
					return;
				};

				let Some(existing_input) = network.nodes.get(&node_id).map_or_else(
					|| {
						if input_index == 0 {
							responses.add(NodeGraphMessage::SetRootNode { root_node: None })
						};
						network.exports.get(input_index)
					},
					|node| node.inputs.get(input_index),
				) else {
					warn!("Could not find input for {node_id} at index {input_index} when disconnecting");
					return;
				};

				let tagged_value = ModifyInputsContext::get_tagged_value(document_network, self.network.clone(), node_id, &self.resolved_types, input_index);

				let mut input = NodeInput::value(tagged_value, true);
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
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					if network.imports_metadata.0 == node || network.exports_metadata.0 == node {
						return;
					}

					if network.nodes.get(&node).and_then(|node| node.implementation.get_network()).is_some() {
						self.network.push(node);
					}
					if let Some(network) = document_network.nested_network_mut(&self.network) {
						if let Some(NodeInput::Node { node_id, output_index, .. }) = network.exports.get(0) {
							network.root_node = Some(RootNode {
								id: *node_id,
								output_index: *output_index,
							});
						}
					}
					self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);
				}
				self.update_selected(document_network, selected_nodes, responses);
			}
			NodeGraphMessage::DuplicateSelectedNodes => {
				if let Some(network) = document_network.nested_network(&self.network) {
					responses.add(DocumentMessage::StartTransaction);

					let new_ids = &selected_nodes.selected_nodes_filtered(network).map(|&id| (id, NodeId(generate_uuid()))).collect();

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
			NodeGraphMessage::EnforceLayerHasNoMultiParams { node_id } => {
				if !self.eligible_to_be_layer(document_network, node_id) {
					responses.add(NodeGraphMessage::SetToNodeOrLayer { node_id: node_id, is_layer: false })
				}
			}
			NodeGraphMessage::ExitNestedNetwork { depth_of_nesting } => {
				selected_nodes.clear_selected_nodes();
				responses.add(BroadcastEvent::SelectionChanged);

				for _ in 0..depth_of_nesting {
					self.network.pop();
				}

				self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);
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
				} else {
					//TODO: Should network and node inputs be able to be disconnected?
					log::error!("Could not hide/show input: {:?} since it is not NodeInput::Value", input);
					return;
				}

				responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });
				responses.add(NodeGraphMessage::EnforceLayerHasNoMultiParams { node_id });
				responses.add(PropertiesPanelMessage::Refresh);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::InsertNode { node_id, document_node } => {
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					network.nodes.insert(node_id, document_node);
				}
			}
			NodeGraphMessage::InsertNodeBetween {
				post_node_id,
				post_node_input_index,
				insert_node_output_index,
				insert_node_id,
				insert_node_input_index,
				pre_node_output_index,
				pre_node_id,
			} => {
				let Some(network) = document_network.nested_network(&self.network) else {
					error!("No network");
					return;
				};
				responses.add(DocumentMessage::StartTransaction);

				responses.add(NodeGraphMessage::InsertNodeBetween {
					post_node_id,
					post_node_input_index,
					insert_node_output_index,
					insert_node_id,
					insert_node_input_index,
					pre_node_output_index,
					pre_node_id,
				});

				if network.connected_to_output(insert_node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::MoveSelectedNodes { displacement_x, displacement_y } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else {
					warn!("No network");
					return;
				};
				for node_id in selected_nodes.selected_nodes(network).cloned().collect::<Vec<_>>() {
					if network.exports_metadata.0 == node_id {
						network.exports_metadata.1 += IVec2::new(displacement_x, displacement_y);
					} else if network.imports_metadata.0 == node_id {
						network.imports_metadata.1 += IVec2::new(displacement_x, displacement_y);
					} else if let Some(node) = network.nodes.get_mut(&node_id) {
						node.metadata.position += IVec2::new(displacement_x, displacement_y)
					}
				}

				// Since document structure doesn't change, just update the nodes
				if graph_view_overlay_open {
					let Some(network) = document_network.nested_network(&self.network) else {
						warn!("No network");
						return;
					};
					let links = Self::collect_links(network);
					let nodes = self.collect_nodes(document_network, network, &links);
					responses.add(FrontendMessage::UpdateNodeGraph { nodes, links });
				}
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
			NodeGraphMessage::SetRootNode { root_node } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else {
					warn!("No network");
					return;
				};
				network.root_node = root_node;
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
				self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);
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
					if ModifyInputsContext::set_input(network, node_id, input_index, input, self.network.is_empty()) {
						load_network_structure(document_network, document_metadata, collapsed);
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
							node.inputs.extend(((node.inputs.len() - 1)..input_index).map(|_| NodeInput::network(generic!(T), 0)));
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
				for &descendant in outwards_links.get(&node_id).unwrap_or(&Vec::new()) {
					let shift = required_shift(node_id, descendant, network);
					let mut stack = vec![descendant];
					while let Some(id) = stack.pop() {
						shift_node(id, shift, network);
						stack.extend(outwards_links.get(&id).unwrap_or(&Vec::new()).iter().copied())
					}
				}

				self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);
			}
			NodeGraphMessage::ToggleSelectedVisibility => {
				let Some(network) = document_network.nested_network(&self.network) else { return };
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let visible = !selected_nodes
					.selected_nodes_filtered(network)
					.all(|&node_id| network.nodes.get(&node_id).is_some_and(|node| node.visible));

				for &node_id in selected_nodes.selected_nodes_filtered(network) {
					responses.add(NodeGraphMessage::SetVisibility { node_id, visible });
				}
			}
			NodeGraphMessage::ToggleVisibility { node_id } => {
				let Some(network) = document_network.nested_network(&self.network) else { return };

				if network.imports_metadata.0 == node_id || network.exports_metadata.0 == node_id {
					return;
				}

				let Some(node) = network.nodes.get(&node_id) else {
					log::error!("Cannot get node {node_id} in NodeGraphMessage::ToggleVisibility");
					return;
				};

				let visible = !node.visible;
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetVisibility { node_id, visible });
			}
			NodeGraphMessage::SetVisibility { node_id, visible } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else { return };

				// Set what we determined shall be the visibility of the node
				let Some(node) = network.nodes.get_mut(&node_id) else {
					log::error!("Could not get node {node_id} in NodeGraphMessage::SetVisibility");
					return;
				};
				node.visible = visible;

				// Only generate node graph if one of the selected nodes is connected to the output
				if network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				// If change has been made to document_network
				if self.network.is_empty() {
					document_metadata.load_structure(document_network);
					self.update_selection_action_buttons(document_network, selected_nodes, responses);
				}

				responses.add(PropertiesPanelMessage::Refresh);
			}
			NodeGraphMessage::ToggleSelectedLocked => {
				let Some(network) = document_network.nested_network(&self.network) else { return };
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let locked = !selected_nodes
					.selected_nodes_filtered(network)
					.all(|&node_id| network.nodes.get(&node_id).is_some_and(|node| node.locked));

				for &node_id in selected_nodes.selected_nodes_filtered(network) {
					responses.add(NodeGraphMessage::SetLocked { node_id, locked });
				}
			}
			NodeGraphMessage::SetLocked { node_id, locked } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else { return };

				let Some(node) = network.nodes.get_mut(&node_id) else { return };
				node.locked = locked;

				if network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				// If change has been made to document_network
				if self.network.is_empty() {
					document_metadata.load_structure(document_network);
					self.update_selection_action_buttons(document_network, selected_nodes, responses);
				}
			}
			NodeGraphMessage::ToggleSelectedAsLayersOrNodes => {
				let Some(network) = document_network.nested_network(&self.network) else { return };

				for node_id in selected_nodes.selected_nodes_filtered(network).cloned().collect::<Vec<_>>() {
					let Some(network_mut) = document_network.nested_network_mut(&self.network) else { return };
					let Some(node) = network_mut.nodes.get_mut(&node_id) else { continue };

					if node.has_primary_output {
						responses.add(NodeGraphMessage::SetToNodeOrLayer {
							node_id: node_id,
							is_layer: !node.is_layer,
						});
					}

					if network_mut.connected_to_output(node_id) {
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				}
			}
			NodeGraphMessage::SetToNodeOrLayer { node_id, is_layer } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else { return };

				if is_layer && !self.eligible_to_be_layer(network, node_id) {
					log::error!("Could not set node {node_id} to layer");
					return;
				}

				if let Some(node) = network.nodes.get_mut(&node_id) {
					node.is_layer = is_layer;
				}
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(DocumentMessage::DocumentStructureChanged);
			}
			NodeGraphMessage::SetName { node_id, name } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetNameImpl { node_id, name });
			}
			NodeGraphMessage::SetNameImpl { node_id, name } => {
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					if let Some(node) = network.nodes.get_mut(&node_id) {
						node.alias = name;
						self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);
					}
				}
			}
			NodeGraphMessage::TogglePreview { node_id } => {
				let Some(network) = document_network.nested_network(&self.network) else { return };

				if network.imports_metadata.0 == node_id || network.exports_metadata.0 == node_id {
					return;
				}
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::TogglePreviewImpl { node_id });
			}
			NodeGraphMessage::TogglePreviewImpl { node_id } => {
				let toggle_id = node_id;
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					if let Some(export) = network.exports.get_mut(0) {
						if let NodeInput::Node { node_id, .. } = export {
							// End preview, set export back to root node
							if *node_id == toggle_id {
								if let Some(root_node) = network.root_node {
									*export = NodeInput::node(root_node.id, 0);
								} else {
									responses.add(NodeGraphMessage::DisconnectInput {
										node_id: network.exports_metadata.0,
										input_index: 0,
									})
								}
							} else {
								*export = NodeInput::node(toggle_id, 0);
							}
						} else {
							*export = NodeInput::node(toggle_id, 0);
						}
					}
				}

				self.update_selection_action_buttons(document_network, selected_nodes, responses);

				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::UpdateNewNodeGraph => {
				selected_nodes.clear_selected_nodes();
				responses.add(BroadcastEvent::SelectionChanged);

				self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);

				let node_types = document_node_types::collect_node_types();
				responses.add(FrontendMessage::UpdateNodeTypes { node_types });

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
		if self.has_selection {
			actions!(NodeGraphMessageDiscriminant;
				ToggleSelectedLocked,
				ToggleSelectedVisibility,
			)
		} else {
			actions!(NodeGraphMessageDiscriminant;)
		}
	}
}

impl NodeGraphMessageHandler {
	/// Similar to [`NodeGraphMessageHandler::actions`], but this provides additional actions if the node graph is open and should only be called in that circumstance.
	pub fn actions_additional_if_node_graph_is_open(&self) -> ActionList {
		if self.has_selection {
			actions!(NodeGraphMessageDiscriminant;
				Copy,
				Cut,
				DeleteSelectedNodes,
				DuplicateSelectedNodes,
				ToggleSelectedAsLayersOrNodes,
			)
		} else {
			actions!(NodeGraphMessageDiscriminant;)
		}
	}

	/// Send the cached layout to the frontend for the options bar at the top of the node panel
	fn send_node_bar_layout(&self, responses: &mut VecDeque<Message>) {
		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout::new(self.widgets.to_vec())),
			layout_target: LayoutTarget::NodeGraphBar,
		});
	}

	/// Updates the buttons for visibility, locked, and preview
	fn update_selection_action_buttons(&mut self, document_network: &NodeNetwork, selected_nodes: &SelectedNodes, responses: &mut VecDeque<Message>) {
		if let Some(network) = document_network.nested_network(&self.network) {
			let mut widgets = Vec::new();

			// Don't allow disabling input or output nodes
			let mut selection = selected_nodes.selected_nodes_filtered(network);

			// If there is at least one other selected node then show the hide or show button
			if selection.next().is_some() {
				// Check if any of the selected nodes are disabled
				let all_visible = selected_nodes.selected_nodes_filtered(network).all(|id| {
					if let Some(node) = network.nodes.get(id) {
						node.visible
					} else {
						error!("Could not get node {id} in update_selection_action_buttons");
						true
					}
				});

				// Check if multiple nodes are selected
				let multiple_nodes = selection.next().is_some();

				// Generate the visible/hidden button accordingly
				let (hide_show_label, hide_show_icon) = if all_visible { ("Make Hidden", "EyeVisible") } else { ("Make Visible", "EyeHidden") };
				let hide_button = TextButton::new(hide_show_label)
					.icon(Some(hide_show_icon.to_string()))
					.tooltip(if all_visible { "Hide selected nodes/layers" } else { "Show selected nodes/layers" }.to_string() + if multiple_nodes { "s" } else { "" })
					.tooltip_shortcut(action_keys!(NodeGraphMessageDiscriminant::ToggleSelectedVisibility))
					.on_update(move |_| NodeGraphMessage::ToggleSelectedVisibility.into())
					.widget_holder();
				widgets.push(hide_button);

				widgets.push(Separator::new(SeparatorType::Related).widget_holder());
			}

			let mut selection = selected_nodes.selected_nodes_filtered(network);
			// If only one node is selected then show the preview or stop previewing button
			if let (Some(&node_id), None) = (selection.next(), selection.next()) {
				// Is this node the current output
				let is_output = network.outputs_contain(node_id);
				// Prevent showing "End Preview if the root node is the output"
				if !(is_output && network.root_node.is_some_and(|root_node| root_node.id == node_id)) {
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
		let Some(network) = context.network.nested_network(context.nested_path) else {
			warn!("No network in collate_properties");
			return Vec::new();
		};
		// We want:
		// - If only nodes (no layers) are selected: display each node's properties
		// - If one layer is selected, and zero or more of its upstream nodes: display the properties for the layer and its upstream nodes
		// - If multiple layers are selected, or one node plus other non-upstream nodes: display nothing

		// First, we filter all the selections into layers and nodes
		let (mut layers, mut nodes) = (Vec::new(), Vec::new());
		for node_id in selected_nodes.selected_nodes_filtered(network) {
			if let Some(layer_or_node) = network.nodes.get(node_id) {
				if layer_or_node.is_layer {
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
					.filter(|&selected_node_id| !network.is_node_upstream_of_another_by_horizontal_flow(layers[0], selected_node_id));
				if nodes_not_upstream_of_layer.count() > 0 {
					return Vec::new();
				}

				// Iterate through all the upstream nodes, but stop when we reach another layer (since that's a point where we switch from horizontal to vertical flow)
				network
					.upstream_flow_back_from_nodes(vec![layers[0]], graph_craft::document::FlowType::HorizontalFlow)
					.enumerate()
					.take_while(|(i, (node, _))| if *i == 0 { true } else { !node.is_layer })
					.map(|(_, (node, node_id))| node_properties::generate_node_properties(node, node_id, context))
					.collect()
			}
			// If multiple layers and/or nodes are selected, show nothing
			_ => Vec::new(),
		}
	}

	fn collect_links(network: &NodeNetwork) -> Vec<FrontendNodeLink> {
		let mut links = network
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
						dashed: false,
					})
				} else if let NodeInput::Network { import_index, .. } = *input {
					Some(FrontendNodeLink {
						link_start: network.imports_metadata.0,
						link_start_output_index: import_index,
						link_end,
						link_end_input_index,
						dashed: false,
					})
				} else {
					None
				}
			})
			.collect::<Vec<_>>();

		// Connect primary export to root node, since previewing a node will change the primary export
		if let Some(root_node) = network.root_node {
			links.push(FrontendNodeLink {
				link_start: root_node.id,
				link_start_output_index: root_node.output_index,
				link_end: network.exports_metadata.0,
				link_end_input_index: 0,
				dashed: false,
			});
		}
		// Connect rest of exports to their actual export field since they are not affected by previewing
		for (i, export) in network.exports.iter().enumerate() {
			if let NodeInput::Node { node_id, output_index, .. } = export {
				links.push(FrontendNodeLink {
					link_start: *node_id,
					link_start_output_index: *output_index,
					link_end: network.exports_metadata.0,
					link_end_input_index: i,
					dashed: network.root_node.is_some_and(|root_node| root_node.id != *node_id) && *output_index == 0,
				})
			}
		}
		links
	}

	fn collect_nodes(&self, document_network: &NodeNetwork, network: &NodeNetwork, links: &[FrontendNodeLink]) -> Vec<FrontendNode> {
		let mut encapsulating_path = self.network.clone();
		let number_of_imports = if let Some(encapsulating_node) = encapsulating_path.pop() {
			document_network
				.nested_network(&encapsulating_path)
				.expect("Encapsulating path should always exist")
				.nodes
				.get(&encapsulating_node)
				.expect("Last path node should always exist in encapsulating network")
				.inputs
				.len()
		} else {
			1
		};
		let connected_node_to_output_lookup = links
			.iter()
			.map(|link| ((link.link_start, link.link_start_output_index), (link.link_end, link.link_end_input_index)))
			.fold(HashMap::new(), |mut acc, (key, value)| {
				acc.entry(key)
					.and_modify(|v: &mut (Vec<NodeId>, Vec<usize>)| {
						v.0.push(value.0);
						v.1.push(value.1);
					})
					.or_insert_with(|| (vec![value.0], vec![value.1]));
				acc
			});

		let mut nodes = Vec::new();
		for (&node_id, node) in &network.nodes {
			let node_id_path = &[&self.network[..], &[node_id]].concat();
			let frontend_graph_inputs = node.inputs.iter().enumerate().map(|(index, _)| {
				// Convert the index in all inputs to the index in only the exposed inputs
				//TODO: Only display input type if potential inputs in node_registry are all the same type
				let input_type = self.resolved_types.inputs.get(&Source { node: node_id_path.clone(), index }).cloned();
				//.or_else(|| input.as_value().map(|tagged_value| tagged_value.ty()));

				let frontend_data_type = if let Some(input_type) = &input_type {
					FrontendGraphDataType::with_type(input_type)
				} else {
					FrontendGraphDataType::General
				};

				FrontendGraphInput {
					data_type: frontend_data_type,
					name: "Placeholder".to_string(),
					resolved_type: input_type.map(|input| format!("{input:?}")),
					connected: None,
				}
			});

			let mut inputs = node.inputs.iter().zip(frontend_graph_inputs).map(|(node_input, mut frontend_graph_input)| {
				if let NodeInput::Node { node_id: connected_node_id, .. } = node_input {
					frontend_graph_input.connected = Some(*connected_node_id);
				} else if let NodeInput::Network { .. } = node_input {
					frontend_graph_input.connected = Some(network.imports_metadata.0);
				}
				(node_input, frontend_graph_input)
			});

			let primary_input = inputs.next().filter(|(input, _)| input.is_exposed()).map(|(_, input_type)| input_type);
			let exposed_inputs = inputs
				.filter(|(input, _)| input.is_exposed() && !(matches!(input, NodeInput::Network { .. }) && document_network == network))
				.map(|(_, input_type)| input_type)
				.collect();

			let output_types = Self::get_output_types(node, &self.resolved_types, node_id_path.clone());
			let primary_output_type = output_types.get(0).expect("Primary output should always exist");
			let frontend_data_type = if let Some(output_type) = primary_output_type {
				FrontendGraphDataType::with_type(&output_type)
			} else {
				FrontendGraphDataType::General
			};
			let (connected, connected_index) = connected_node_to_output_lookup.get(&(node_id, 0)).unwrap_or(&(Vec::new(), Vec::new())).clone();
			let primary_output = Some(FrontendGraphOutput {
				data_type: frontend_data_type,
				name: "Output 1".to_string(),
				resolved_type: primary_output_type.clone().map(|input| format!("{input:?}")),
				connected,
				connected_index,
			});

			let mut exposed_outputs = Vec::new();
			for (index, exposed_output) in output_types.iter().enumerate() {
				if index == 0 {
					continue;
				}
				let frontend_data_type = if let Some(output_type) = &exposed_output {
					FrontendGraphDataType::with_type(output_type)
				} else {
					FrontendGraphDataType::General
				};
				let (connected, connected_index) = connected_node_to_output_lookup.get(&(node_id, index)).unwrap_or(&(Vec::new(), Vec::new())).clone();
				exposed_outputs.push(FrontendGraphOutput {
					data_type: frontend_data_type,
					name: format!("Output {}", index + 2),
					resolved_type: exposed_output.clone().map(|input| format!("{input:?}")),
					connected,
					connected_index,
				});
			}

			nodes.push(FrontendNode {
				id: node_id,
				is_layer: node.is_layer,
				can_be_layer: self.eligible_to_be_layer(network, node_id),
				alias: Self::untitled_layer_label(node),
				name: node.name.clone(),
				primary_input,
				exposed_inputs,
				primary_output,
				exposed_outputs,
				position: node.metadata.position.into(),
				previewed: network.outputs_contain(node_id),
				visible: node.visible,
				locked: node.locked,
				errors: None,
				ui_only: false,
			});
		}

		// Add import/export UI only nodes
		let mut export_node_inputs = Vec::new();
		for (index, export) in network.exports.iter().enumerate() {
			let (frontend_data_type, input_type) = if let NodeInput::Node { node_id, .. } = export {
				let node_id_path = &[&self.network[..], &[*node_id]].concat();
				let input_type = self.resolved_types.inputs.get(&Source { node: node_id_path.clone(), index });

				if let Some(input_type) = input_type {
					(FrontendGraphDataType::with_type(&input_type), Some(input_type.clone()))
				} else {
					(FrontendGraphDataType::General, None)
				}
			}
			// If type should only be determined from resolved_types then remove this
			else if let NodeInput::Value { tagged_value, .. } = export {
				(FrontendGraphDataType::with_tagged_value(tagged_value), Some(tagged_value.ty()))
			} else if let NodeInput::Network { import_type, .. } = export {
				(FrontendGraphDataType::with_type(import_type), Some(import_type.clone()))
			} else {
				(FrontendGraphDataType::General, None)
			};

			// First import index is visually connected to the root node instead of its actual export input so previewing does not change the connection
			let connected = if index == 0 {
				network.root_node.map(|root_node| root_node.id)
			} else {
				if let NodeInput::Node { node_id, .. } = export {
					Some(*node_id)
				} else {
					None
				}
			};
			export_node_inputs.push(FrontendGraphInput {
				data_type: frontend_data_type,
				name: format!("Export {}", index + 1),
				resolved_type: input_type.map(|input| format!("{input:?}")),
				connected,
			});
		}
		nodes.push(FrontendNode {
			id: network.exports_metadata.0,
			is_layer: false,
			can_be_layer: false,
			alias: "Exports".to_string(),
			name: "Exports".to_string(),
			primary_input: None,
			exposed_inputs: export_node_inputs,
			primary_output: None,
			exposed_outputs: Vec::new(),
			position: network.exports_metadata.1.into(),
			previewed: false,
			visible: true,
			locked: false,
			errors: None,
			ui_only: true,
		});
		if document_network != network {
			let mut import_node_outputs = Vec::new();
			for index in 0..number_of_imports {
				let (connected, connected_index) = connected_node_to_output_lookup.get(&(network.imports_metadata.0, index)).unwrap_or(&(Vec::new(), Vec::new())).clone();
				let input_type = self.resolved_types.inputs.get(&Source { node: self.network.clone(), index }).cloned().or_else(|| {
					// If type should only be determined from resolved_types then remove this
					let mut parent_network_path = self.network.clone();
					let Some(parent_node_id) = parent_network_path.pop() else {
						log::error!("Could not get parent node id from {:?}", parent_network_path);
						return None;
					};
					let Some(parent_network) = document_network.nested_network(&parent_network_path) else {
						log::error!("Could not get network for node {parent_node_id}");
						return None;
					};
					let Some(parent_node) = parent_network.nodes.get(&parent_node_id) else {
						log::error!("Could not get node for {parent_node_id}");
						return None;
					};
					parent_node.inputs.get(index).and_then(|input| input.as_value().map(|tagged_value| tagged_value.ty()))
				});
				let frontend_data_type = if let Some(input_type) = input_type.clone() {
					FrontendGraphDataType::with_type(&input_type)
				} else {
					FrontendGraphDataType::General
				};

				import_node_outputs.push(FrontendGraphOutput {
					data_type: frontend_data_type,
					name: format!("Import {}", index + 1),
					resolved_type: input_type.map(|input| format!("{input:?}")),
					connected,
					connected_index,
				});
			}
			nodes.push(FrontendNode {
				id: network.imports_metadata.0,
				is_layer: false,
				can_be_layer: false,
				alias: "Imports".to_string(),
				name: "Imports".to_string(),
				primary_input: None,
				exposed_inputs: Vec::new(),
				primary_output: None,
				exposed_outputs: import_node_outputs,
				position: network.imports_metadata.1.into(),
				previewed: false,
				visible: true,
				locked: false,
				errors: None,
				ui_only: true,
			});
		}
		nodes
	}
	fn collect_subgraph_names(subgraph_path: &Vec<NodeId>, network: &NodeNetwork) -> Vec<String> {
		let mut current_network = network;
		subgraph_path
			.iter()
			.map(|node_id| {
				let Some(node) = current_network.nodes.get(node_id) else {
					log::error!("Could not find node id {node_id} in network");
					return String::new();
				};
				if let Some(network) = node.implementation.get_network() {
					current_network = network;
				}

				//TODO: Maybe replace with alias and default to name if it does not exist
				node.name.clone()
			})
			.collect::<Vec<String>>()
	}
	fn update_layer_panel(document_network: &NodeNetwork, metadata: &DocumentMetadata, collapsed: &CollapsedLayers, responses: &mut VecDeque<Message>) {
		for (&node_id, node) in &document_network.nodes {
			if node.is_layer {
				let layer = LayerNodeIdentifier::new(node_id, document_network);

				let parents_visible = layer.ancestors(metadata).filter(|&ancestor| ancestor != layer).all(|layer| {
					if layer != LayerNodeIdentifier::ROOT_PARENT {
						document_network.nodes.get(&layer.to_node()).map(|node| node.visible).unwrap_or_default()
					} else {
						true
					}
				});

				let parents_unlocked = layer.ancestors(metadata).filter(|&ancestor| ancestor != layer).all(|layer| {
					if layer != LayerNodeIdentifier::ROOT_PARENT {
						document_network.nodes.get(&layer.to_node()).map(|node| !node.locked).unwrap_or_default()
					} else {
						true
					}
				});

				let data = LayerPanelEntry {
					id: node_id,
					children_allowed:
						// The layer has other layers as children along the secondary input's horizontal flow
						layer.has_children(metadata)
						|| (
							// At least one secondary input is exposed on this layer node
							node.inputs.iter().skip(1).any(|input| input.is_exposed()) &&
							// But nothing is connected to it, since we only get 1 item (ourself) when we ask for the flow from the secondary input
							document_network.upstream_flow_back_from_nodes(vec![node_id], FlowType::HorizontalFlow).count() == 1
						),
					children_present: layer.has_children(metadata),
					expanded: layer.has_children(metadata) && !collapsed.0.contains(&layer),
					depth: layer.ancestors(metadata).count() - 1,
					parent_id: layer.parent(metadata).and_then(|parent| if parent != LayerNodeIdentifier::ROOT_PARENT{ Some(parent.to_node())} else {None}),
					name: node.name.clone(),
					alias: Self::untitled_layer_label(node),
					tooltip: if cfg!(debug_assertions) { format!("Layer ID: {node_id}") } else { "".into() },
					visible: node.visible,
					parents_visible,
					unlocked: !node.locked,
					parents_unlocked,
				};
				responses.add(FrontendMessage::UpdateDocumentLayerDetails { data });
			}
		}
	}

	fn send_graph(&self, document_network: &NodeNetwork, metadata: &mut DocumentMetadata, collapsed: &CollapsedLayers, graph_open: bool, responses: &mut VecDeque<Message>) {
		let nested_path = Self::collect_subgraph_names(&self.network, document_network);

		let Some(network) = document_network.nested_network(&self.network) else {
			log::error!("Could not send graph since nested network does not exist");
			return;
		};
		// View encapsulating network
		// let mut network = &mut crate::messages::portfolio::document::node_graph::document_node_types::wrap_network_in_scope(document_network.clone(), generate_uuid());
		// network.root_node = Some(RootNode { id: NodeId(3), output_index: 0 });
		responses.add(DocumentMessage::DocumentStructureChanged);
		responses.add(PropertiesPanelMessage::Refresh);
		metadata.load_structure(document_network);
		Self::update_layer_panel(document_network, metadata, collapsed, responses);
		if graph_open {
			let links = Self::collect_links(network);
			let nodes = self.collect_nodes(document_network, network, &links);
			responses.add(FrontendMessage::UpdateNodeGraph { nodes, links });
			responses.add(FrontendMessage::UpdateSubgraphPath { subgraph_path: nested_path })
		}
	}

	pub fn get_output_types(node: &DocumentNode, resolved_types: &ResolvedDocumentNodeTypes, node_id_path: Vec<NodeId>) -> Vec<Option<Type>> {
		let mut output_types = Vec::new();

		let primary_output_type = resolved_types.outputs.get(&Source { node: node_id_path.clone(), index: 0 }).cloned();
		output_types.push(primary_output_type);

		// If the node is not a protonode, get types by traversing across exports until a proto node is reached.
		if let graph_craft::document::DocumentNodeImplementation::Network(internal_network) = &node.implementation {
			for export in internal_network.exports.iter().skip(1) {
				let mut current_export = export;
				let mut current_network = internal_network;
				let mut current_path = node_id_path.clone();
				while let NodeInput::Node { node_id, output_index, .. } = current_export {
					current_path.push(*node_id);
					let next_node = current_network.nodes.get(node_id).expect("Export node id should always exist");
					if let graph_craft::document::DocumentNodeImplementation::Network(next_network) = &next_node.implementation {
						current_network = next_network;
						current_export = next_network.exports.get(*output_index).expect("Export at output index should always exist");
					} else {
						break;
					}
				}

				let output_type: Option<Type> = if let NodeInput::Node { output_index, .. } = current_export {
					//Current export is pointing to a proto node where type can be derived
					assert_eq!(*output_index, 0, "Output index for a proto node should always be 0");
					resolved_types.outputs.get(&Source { node: current_path.clone(), index: 0 }).cloned()
				} else if let NodeInput::Value { tagged_value, .. } = current_export {
					Some(tagged_value.ty())
				} else if let NodeInput::Network { import_type, .. } = current_export {
					Some(import_type.clone())
				} else {
					None
				};
				output_types.push(output_type);
			}
		}
		output_types
	}

	/// Updates the frontend's selection state in line with the backend
	fn update_selected(&mut self, document_network: &NodeNetwork, selected_nodes: &SelectedNodes, responses: &mut VecDeque<Message>) {
		self.update_selection_action_buttons(document_network, selected_nodes, responses);
		responses.add(FrontendMessage::UpdateNodeGraphSelection {
			selected: selected_nodes.selected_nodes_ref().clone(),
		});
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

	pub fn eligible_to_be_layer(&self, document_network: &NodeNetwork, node_id: NodeId) -> bool {
		if document_network.imports_metadata.0 == node_id || document_network.exports_metadata.0 == node_id {
			return false;
		}

		let Some(node) = document_network.nodes.get(&node_id) else { return false };

		let exposed_value_count = node.inputs.iter().filter(|input| if let NodeInput::Value { exposed, .. } = input { *exposed } else { false }).count();
		let node_input_count = node
			.inputs
			.iter()
			.filter(|input| {
				if matches!(input, NodeInput::Node { .. }) || matches!(input, NodeInput::Network { .. }) {
					true
				} else {
					false
				}
			})
			.count();
		let input_count = node_input_count + exposed_value_count;

		let output_count = if let graph_craft::document::DocumentNodeImplementation::Network(nested_network) = &node.implementation {
			nested_network.exports.len()
		} else {
			// Node is a protonode, so it must have 1 output
			1
		};

		// TODO: Eventually allow nodes at the bottom of a stack to be layers, where `input_count` is 0
		node.has_primary_output && output_count == 1 && (input_count == 1 || input_count == 2)
	}

	fn untitled_layer_label(node: &DocumentNode) -> String {
		(node.alias != "")
			.then_some(node.alias.clone())
			.unwrap_or(if node.is_layer && node.name == "Merge" { "Untitled Layer".to_string() } else { node.name.clone() })
	}
}

impl Default for NodeGraphMessageHandler {
	fn default() -> Self {
		let right_side_widgets = vec![
			// TODO: Replace this with an "Add Node" button, also next to an "Add Layer" button
			TextLabel::new("Right Click in Graph to Add Nodes").italic(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextButton::new("Node Graph")
				.icon(Some("GraphViewOpen".into()))
				.hover_icon(Some("GraphViewClosed".into()))
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
