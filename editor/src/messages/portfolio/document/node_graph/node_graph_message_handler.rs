use graph_craft::document::{DocumentNode, FlowType, NodeId, NodeInput, NodeNetwork, Source};
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
				self.update_selection_action_buttons(document_network, document_metadata, selected_nodes, responses);
				self.update_selected(document_network, document_metadata, selected_nodes, responses);
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
			NodeGraphMessage::DeleteNodes { node_ids, reconnect } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else {
					error!("No network");
					return;
				};
				let mut delete_nodes = HashSet::new();

				for node_id in &node_ids {
					delete_nodes.insert(*node_id);

					if !reconnect {
						continue;
					};

					let Some(node) = network.nodes.get(&node_id) else {
						continue;
					};
					let child_id = node.inputs.get(1).and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id) } else { None });
					let Some(child_id) = child_id else {
						continue;
					};

					let outward_links = network.collect_outwards_links();

					for (_, upstream_id) in network.upstream_flow_back_from_nodes(vec![*child_id], graph_craft::document::FlowType::UpstreamFlow) {
						// This does a downstream traversal starting from the current node, and ending at either a node in the delete_nodes set or the output.
						// If the traversal find as child node of a node in the delete_nodes set, then it is a sole dependent. If the output node is eventually reached, then it is not a sole dependent.
						let mut stack = vec![upstream_id];
						let mut can_delete = true;

						while let Some(current_node) = stack.pop() {
							if let Some(downstream_nodes) = outward_links.get(&current_node) {
								for downstream_node in downstream_nodes {
									if network.root_node.expect("Root node should always exist if a node is being deleted").id == *downstream_node {
										can_delete = false;
									} else if !delete_nodes.contains(downstream_node) {
										stack.push(*downstream_node);
									}
									// Continue traversing over the downstream sibling, which happens if the current node is a sibling to a node in node_ids
									else {
										for deleted_node_id in &node_ids {
											let Some(output_node) = network.nodes.get(&deleted_node_id) else {
												continue;
											};
											let Some(input) = output_node.inputs.get(0) else {
												continue;
											};

											if let NodeInput::Node { node_id, .. } = input {
												if *node_id == current_node {
													stack.push(*deleted_node_id);
												};
											};
										}
									};
								}
							}
						}

						if can_delete {
							delete_nodes.insert(upstream_id);
						}
					}
				}
				for delete_node_id in delete_nodes {
					let Some(delete_node) = network.nodes.get(&delete_node_id) else {
						continue;
					};

					if delete_node.is_layer {
						// Delete node from document metadata
						let layer_node = LayerNodeIdentifier::new(delete_node_id, network);
						layer_node.delete(document_metadata);
					}
					self.remove_node(network, selected_nodes, delete_node_id, responses, reconnect);
				}

				// Only generate node graph if one of the selected nodes is connected to the output
				if selected_nodes.selected_nodes().any(|&node_id| network.connected_to_output(node_id)) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				// There is no need to call `load_network_structure()` since the metadata is already updated
			}
			// Deletes selected_nodes. If `reconnect` is true, then all children nodes (secondary input) of the selected nodes are deleted and the siblings (primary input/output) are reconnected.
			// If `reconnect` is false, then only the selected nodes are deleted and not reconnected.
			NodeGraphMessage::DeleteSelectedNodes { reconnect } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::DeleteNodes {
					node_ids: selected_nodes.selected_nodes().copied().collect(),
					reconnect,
				});
			}
			NodeGraphMessage::DisconnectNodes { node_id, input_index } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::DisconnectInput { node_id, input_index });

				let Some(network) = document_network.nested_network(&self.network) else {
					warn!("No network");
					return;
				};

				if network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::DisconnectLayerFromStack { node_id, reconnect_to_sibling } => {
				let Some(network) = document_network.nested_network(&self.network) else {
					warn!("No network");
					return;
				};

				// Ensure node is a layer and create LayerNodeIdentifier
				if network.nodes.get(&node_id).is_some_and(|node| !node.is_layer) {
					log::error!("Non layer node passed to DisconnectLayer");
					return;
				}

				let layer_to_disconnect = LayerNodeIdentifier::new(node_id, &network);

				let Some((downstream_node_id, downstream_input_index)) = DocumentMessageHandler::get_downstream_node(&network, &document_metadata, layer_to_disconnect) else {
					log::error!("Downstream node should always exist when moving layer");
					return;
				};
				let layer_to_move_sibling_input = network.nodes.get(&layer_to_disconnect.to_node()).and_then(|node| node.inputs.get(0));
				if let Some(NodeInput::Node { node_id, .. }) = layer_to_move_sibling_input.and_then(|node_input| if reconnect_to_sibling { Some(node_input) } else { None }) {
					let upstream_sibling_id = *node_id;
					let Some(downstream_node) = document_network.nodes.get_mut(&downstream_node_id) else { return };

					if let Some(NodeInput::Node { node_id, .. }) = downstream_node.inputs.get_mut(downstream_input_index) {
						*node_id = upstream_sibling_id;
					}

					let upstream_shift = IVec2::new(0, -3);
					responses.add(NodeGraphMessage::ShiftUpstream {
						node_id: upstream_sibling_id,
						shift: upstream_shift,
						shift_self: true,
					});
				} else {
					// Disconnect node directly downstream if upstream sibling doesn't exist
					responses.add(GraphOperationMessage::DisconnectInput {
						node_id: downstream_node_id,
						input_index: downstream_input_index,
					});
				}

				responses.add(GraphOperationMessage::DisconnectInput {
					node_id: layer_to_disconnect.to_node(),
					input_index: 0,
				});
			}
			NodeGraphMessage::EnterNestedNetwork { node } => {
				if let Some(network) = document_network.nested_network(&self.network) {
					if network.imports_metadata.0 == node || network.exports_metadata.0 == node {
						return;
					}

					if network.nodes.get(&node).and_then(|node| node.implementation.get_network()).is_some() {
						self.network.push(node);
					}
					self.send_graph(document_network, document_metadata, selected_nodes, collapsed, graph_view_overlay_open, responses);
				}
				self.update_selected(document_network, document_metadata, selected_nodes, responses);
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

					self.update_selected(document_network, document_metadata, selected_nodes, responses);
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
				self.send_graph(document_network, document_metadata, selected_nodes, collapsed, graph_view_overlay_open, responses);

				self.update_selected(document_network, document_metadata, selected_nodes, responses);
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

				responses.add(GraphOperationMessage::InsertNodeBetween {
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

				for node_id in selected_nodes.selected_nodes() {
					if network.exports_metadata.0 == *node_id {
						network.exports_metadata.1 += IVec2::new(displacement_x, displacement_y);
					} else if network.imports_metadata.0 == *node_id {
						network.imports_metadata.1 += IVec2::new(displacement_x, displacement_y);
					} else if let Some(node) = network.nodes.get_mut(node_id) {
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
				self.send_graph(document_network, document_metadata, selected_nodes, collapsed, graph_view_overlay_open, responses);
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
			NodeGraphMessage::SetNodePosition { node_id, position } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else {
					warn!("No network");
					return;
				};

				let Some(node) = network.nodes.get_mut(&node_id) else {
					log::error!("Failed to find node {node_id} when setting position");
					return;
				};

				node.metadata.position = position;

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

				self.send_graph(document_network, document_metadata, selected_nodes, collapsed, graph_view_overlay_open, responses);
			}
			NodeGraphMessage::ShiftUpstream { node_id, shift, shift_self } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else {
					warn!("No network");
					return;
				};

				let mut modify_inputs = ModifyInputsContext::new(network, document_metadata, self, responses);
				modify_inputs.shift_upstream(node_id, shift, shift_self);
			}
			NodeGraphMessage::ToggleSelectedVisibility => {
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let visible = selected_nodes.selected_nodes().all(|&node_id| document_metadata.node_is_visible(node_id));
				let visible = !visible;

				for &node_id in selected_nodes.selected_nodes() {
					responses.add(NodeGraphMessage::SetVisibility { node_id, visible });
				}
			}
			NodeGraphMessage::ToggleVisibility { node_id } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else { return };

				if network.imports_metadata.0 == node_id || network.exports_metadata.0 == node_id {
					return;
				}

				responses.add(DocumentMessage::StartTransaction);
				let visible = document_metadata.node_is_visible(node_id);
				let visible = !visible;

				responses.add(NodeGraphMessage::SetVisibility { node_id, visible });
			}
			NodeGraphMessage::SetVisibility { node_id, visible } => {
				(|| {
					let Some(network) = document_network.nested_network_mut(&self.network) else { return };

					// Set what we determined shall be the visibility of the node
					let Some(node) = network.nodes.get_mut(&node_id) else { return };
					node.visible = visible;

					// Only generate node graph if one of the selected nodes is connected to the output
					if network.connected_to_output(node_id) {
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				})();

				document_metadata.load_structure(document_network, selected_nodes);

				self.update_selection_action_buttons(document_network, document_metadata, selected_nodes, responses);

				responses.add(PropertiesPanelMessage::Refresh);
			}
			NodeGraphMessage::ToggleSelectedLocked => {
				responses.add(DocumentMessage::StartTransaction);

				let is_locked = !selected_nodes.selected_nodes().any(|&id| document_metadata.node_is_locked(id));

				for &node_id in selected_nodes.selected_nodes() {
					responses.add(NodeGraphMessage::SetLocked { node_id, locked: is_locked });
				}
			}
			NodeGraphMessage::ToggleLocked { node_id } => {
				responses.add(DocumentMessage::StartTransaction);
				let is_locked = !document_metadata.node_is_locked(node_id);
				responses.add(NodeGraphMessage::SetLocked { node_id, locked: is_locked });
			}
			NodeGraphMessage::SetLocked { node_id, locked } => {
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					let Some(node) = network.nodes.get_mut(&node_id) else { return };
					node.locked = locked;

					if network.connected_to_output(node_id) {
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				}
				document_metadata.load_structure(document_network, selected_nodes);
				self.update_selection_action_buttons(document_network, document_metadata, selected_nodes, responses);
			}
			NodeGraphMessage::ToggleSelectedAsLayersOrNodes => {
				let Some(network) = document_network.nested_network_mut(&self.network) else { return };

				for node_id in selected_nodes.selected_nodes() {
					let Some(node) = network.nodes.get_mut(&node_id) else { continue };

					if node.has_primary_output {
						responses.add(NodeGraphMessage::SetToNodeOrLayer {
							node_id: *node_id,
							is_layer: !node.is_layer,
						});
					}

					if network.connected_to_output(*node_id) {
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
						self.send_graph(document_network, document_metadata, selected_nodes, collapsed, graph_view_overlay_open, responses);
					}
				}
			}
			NodeGraphMessage::TogglePreview { node_id } => {
				let Some(network) = document_network.nested_network_mut(&self.network) else { return };

				if network.imports_metadata.0 == node_id || network.exports_metadata.0 == node_id {
					return;
				}
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::TogglePreviewImpl { node_id });
			}
			NodeGraphMessage::TogglePreviewImpl { node_id } => {
				if let Some(network) = document_network.nested_network_mut(&self.network) {
					if let Some(export) = network.exports.get_mut(0) {
						*export = NodeInput::node(node_id, 0);
					}
				}

				self.update_selection_action_buttons(document_network, document_metadata, selected_nodes, responses);

				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::UpdateNewNodeGraph => {
				selected_nodes.clear_selected_nodes();
				responses.add(BroadcastEvent::SelectionChanged);

				self.send_graph(document_network, document_metadata, selected_nodes, collapsed, graph_view_overlay_open, responses);

				let node_types = document_node_types::collect_node_types();
				responses.add(FrontendMessage::UpdateNodeTypes { node_types });

				self.update_selected(document_network, document_metadata, selected_nodes, responses);
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
	fn update_selection_action_buttons(&mut self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, selected_nodes: &SelectedNodes, responses: &mut VecDeque<Message>) {
		if let Some(network) = document_network.nested_network(&self.network) {
			let mut widgets = Vec::new();

			// Don't allow disabling input or output nodes
			let mut selection = selected_nodes
				.selected_nodes()
				.filter(|node_id| network.imports_metadata.0 != **node_id && network.exports_metadata.0 != **node_id);

			// If there is at least one other selected node then show the hide or show button
			if selection.next().is_some() {
				// Check if any of the selected nodes are disabled
				let all_visible = selected_nodes.selected_nodes().all(|&id| document_metadata.node_is_visible(id));

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

			// If only one node is selected then show the preview or stop previewing button
			if let (Some(&node_id), None) = (selection.next(), selection.next()) {
				// Is this node the current output
				let is_output = network.outputs_contain(node_id);
				let output_button = TextButton::new(if is_output { "End Preview" } else { "Preview" })
					.icon(Some("Rescale".to_string()))
					.tooltip(if is_output { "Restore preview to the graph output" } else { "Preview selected node/layer" }.to_string() + " (Shortcut: Alt-click node/layer)")
					.on_update(move |_| NodeGraphMessage::TogglePreview { node_id }.into())
					.widget_holder();
				widgets.push(output_button);
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
		for node_id in selected_nodes.selected_nodes() {
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
			});
		}
		// Connect rest of exports to their actual export field since they are not affected by previewing
		for (i, export) in network.exports.iter().enumerate().skip(1) {
			if let NodeInput::Node { node_id, output_index, .. } = export {
				links.push(FrontendNodeLink {
					link_start: *node_id,
					link_start_output_index: *output_index,
					link_end: NodeId(0),
					link_end_input_index: i,
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
			let node_path = vec![node_id];
			// TODO: This should be based on the graph runtime type inference system in order to change the colors of node connectors to match the data type in use
			if let Some(document_node_definition) = document_node_types::resolve_document_node_type(&node.name) {
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
				let mut outputs = document_node_definition.outputs.iter().enumerate().map(|(index, output_type)| {
					let (connected, connected_index) = connected_node_to_output_lookup.get(&(node_id, index)).unwrap_or(&(Vec::new(), Vec::new())).clone();
					FrontendGraphOutput {
						data_type: output_type.data_type,
						name: output_type.name.to_string(),
						resolved_type: self.resolved_types.outputs.get(&Source { node: node_path.clone(), index }).map(|output| format!("{output:?}")),
						connected,
						connected_index,
					}
				});
				let primary_output = node.has_primary_output.then(|| outputs.next()).flatten();
				let exposed_outputs = outputs.collect::<Vec<_>>();

				// Errors
				let errors = self.node_graph_errors.iter().find(|error| error.node_path.starts_with(&node_path)).map(|error| error.error.clone());
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
					errors: errors.map(|e| format!("{e:?}")),
					ui_only: false,
				});
			} else {
				//TODO: Display nodes without definition by using compiled network
			}
		}
		let mut export_node_inputs = Vec::new();
		for (i, export) in network.exports.iter().enumerate() {
			//First export should visually connect to the root node
			if i == 0 {
				export_node_inputs.push(FrontendGraphInput {
					data_type: super::utility_types::FrontendGraphDataType::General,
					name: format!("Export {}", i + 1),
					resolved_type: None,
					connected: network.root_node.map(|root_node| root_node.id),
				});
			} else {
				let export_input_node = if let NodeInput::Node { node_id, .. } = export { Some(*node_id) } else { None };

				export_node_inputs.push(FrontendGraphInput {
					data_type: super::utility_types::FrontendGraphDataType::General,
					name: format!("Export {}", i + 1),
					resolved_type: None,
					connected: export_input_node,
				});
			}
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
		let mut import_node_outputs = Vec::new();
		for i in 0..number_of_imports {
			let (connected, connected_index) = connected_node_to_output_lookup.get(&(network.imports_metadata.0, i)).unwrap_or(&(Vec::new(), Vec::new())).clone();

			import_node_outputs.push(FrontendGraphOutput {
				data_type: super::utility_types::FrontendGraphDataType::General,
				name: format!("Import {}", i + 1),
				resolved_type: None,
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
	fn update_layer_panel(network: &NodeNetwork, metadata: &DocumentMetadata, collapsed: &CollapsedLayers, responses: &mut VecDeque<Message>) {
		for (&node_id, node) in &network.nodes {
			if node.is_layer {
				let layer = LayerNodeIdentifier::new(node_id, network);

				let parents_visible = layer
					.ancestors(metadata)
					.filter(|&ancestor| ancestor != layer)
					.all(|layer| network.nodes.get(&layer.to_node()).map(|node| node.visible).unwrap_or_default());

				let parents_unlocked = layer
					.ancestors(metadata)
					.filter(|&ancestor| ancestor != layer)
					.all(|layer| network.nodes.get(&layer.to_node()).map(|node| !node.locked).unwrap_or_default());

				let data = LayerPanelEntry {
					id: node_id,
					children_allowed:
						// The layer has other layers as children along the secondary input's horizontal flow
						layer.has_children(metadata)
						|| (
							// At least one secondary input is exposed on this layer node
							node.inputs.iter().skip(1).any(|input| input.is_exposed()) &&
							// But nothing is connected to it, since we only get 1 item (ourself) when we ask for the flow from the secondary input
							network.upstream_flow_back_from_nodes(vec![node_id], FlowType::HorizontalFlow).count() == 1
						),
					children_present: layer.has_children(metadata),
					expanded: layer.has_children(metadata) && !collapsed.0.contains(&layer),
					depth: layer.ancestors(metadata).count() - 1,
					parent_id: layer.parent(metadata).map(|parent| parent.to_node()),
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

	fn send_graph(
		&self,
		document_network: &NodeNetwork,
		metadata: &mut DocumentMetadata,
		selected_nodes: &mut SelectedNodes,
		collapsed: &CollapsedLayers,
		graph_open: bool,
		responses: &mut VecDeque<Message>,
	) {
		let nested_path = Self::collect_subgraph_names(&self.network, document_network);

		let Some(network) = document_network.nested_network(&self.network) else {
			log::error!("Could not send graph since nested network does not exist");
			return;
		};
		//let network = &crate::messages::portfolio::document::node_graph::document_node_types::wrap_network_in_scope(document_network.clone(), generate_uuid());

		responses.add(DocumentMessage::DocumentStructureChanged);
		responses.add(PropertiesPanelMessage::Refresh);
		metadata.load_structure(network, selected_nodes);
		Self::update_layer_panel(network, metadata, collapsed, responses);
		if graph_open {
			let links = Self::collect_links(network);
			let nodes = self.collect_nodes(document_network, network, &links);
			responses.add(FrontendMessage::UpdateNodeGraph { nodes, links });
			responses.add(FrontendMessage::UpdateSubgraphPath { subgraph_path: nested_path })
		}
	}

	/// Updates the frontend's selection state in line with the backend
	fn update_selected(&mut self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, selected_nodes: &SelectedNodes, responses: &mut VecDeque<Message>) {
		self.update_selection_action_buttons(document_network, document_metadata, selected_nodes, responses);
		responses.add(FrontendMessage::UpdateNodeGraphSelection {
			selected: selected_nodes.selected_nodes_ref().clone(),
		});
	}

	fn remove_references_from_network(network: &mut NodeNetwork, deleting_node_id: NodeId, reconnect: bool) -> bool {
		let mut reconnect_to_input: Option<NodeInput> = None;

		if reconnect {
			// Check whether the being-deleted node's first (primary) input is a node
			if let Some(node) = network.nodes.get(&deleting_node_id) {
				// Reconnect to the node below when deleting a layer node.
				if matches!(&node.inputs.get(0), Some(NodeInput::Node { .. })) {
					reconnect_to_input = Some(node.inputs[0].clone());
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
		if !Self::remove_references_from_network(document_network, node_id, reconnect) {
			log::debug!("could not remove_references_from_network");
			return false;
		}
		document_network.nodes.remove(&node_id);
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

	pub fn eligible_to_be_layer(&self, document_network: &NodeNetwork, node_id: NodeId) -> bool {
		if document_network.imports_metadata.0 == node_id || document_network.exports_metadata.0 == node_id {
			return false;
		}

		let Some(node) = document_network.nodes.get(&node_id) else { return false };
		let Some(definition) = resolve_document_node_type(&node.name) else { return false };

		let exposed_value_count = node.inputs.iter().filter(|input| if let NodeInput::Value { exposed, .. } = input { *exposed } else { false }).count();
		let node_input_count = node.inputs.iter().filter(|input| if let NodeInput::Node { .. } = input { true } else { false }).count();
		let input_count = node_input_count + exposed_value_count;
		let output_count = definition.outputs.len();

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
