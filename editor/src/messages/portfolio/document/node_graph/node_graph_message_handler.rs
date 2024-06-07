use super::utility_types::{FrontendGraphInput, FrontendGraphOutput, FrontendNode, FrontendNodeWire};
use super::{document_node_types, node_properties};
use crate::application::generate_uuid;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::graph_operation::load_network_structure;
use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::node_graph::document_node_types::NodePropertiesContext;
use crate::messages::portfolio::document::node_graph::utility_types::FrontendGraphDataType;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, LayerPanelEntry, SelectedNodes};
use crate::messages::prelude::*;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, FlowType, NodeId, NodeInput, NodeNetwork, Previewing, Source};
use graph_craft::proto::GraphErrors;
use graphene_core::*;
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypes;

use glam::{DAffine2, DVec2, IVec2};
use renderer::ClickTarget;

#[derive(Debug)]
pub struct NodeGraphHandlerData<'a> {
	pub document_network: &'a mut NodeNetwork,
	pub document_metadata: &'a mut DocumentMetadata,
	pub selected_nodes: &'a mut SelectedNodes,
	pub document_id: DocumentId,
	pub document_name: &'a str,
	pub collapsed: &'a mut CollapsedLayers,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub graph_view_overlay_open: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeGraphMessageHandler {
	pub network: Vec<NodeId>,
	pub resolved_types: ResolvedDocumentNodeTypes,
	pub node_graph_errors: GraphErrors,
	has_selection: bool,
	widgets: [LayoutGroup; 2],
	drag_start: Option<DVec2>,
}

/// NodeGraphMessageHandler always modifies the network which the selected nodes are in. No GraphOperationMessages should be added here, since those messages will always affect the document network.
impl<'a> MessageHandler<NodeGraphMessage, NodeGraphHandlerData<'a>> for NodeGraphMessageHandler {
	fn process_message(&mut self, message: NodeGraphMessage, responses: &mut VecDeque<Message>, data: NodeGraphHandlerData<'a>) {
		let NodeGraphHandlerData {
			document_network,
			document_metadata,
			selected_nodes,
			document_id,
			collapsed,
			graph_view_overlay_open,
			ipp,
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
			NodeGraphMessage::ConnectNodesByWire {
				output_node,
				output_node_connector_index,
				input_node,
				input_node_connector_index,
			} => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, std::iter::once(&output_node)) else {
					return;
				};
				// If `output_node_id` is None, then it is the UI-only "Import" node
				let output_node_id = if network.imports_metadata.0 == output_node { None } else { Some(output_node) };
				// If `input_node_id` is None, then it is the UI-only "Export" node
				let input_node_id = if network.exports_metadata.0 == input_node { None } else { Some(input_node) };

				let input_index = NodeGraphMessageHandler::get_input_index(network, input_node, input_node_connector_index);
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
						// TODO: Add support for flattening NodeInput::Network exports in flatten_with_fns https://github.com/GraphiteEditor/Graphite/issues/1762
						responses.add(DialogMessage::RequestComingSoonDialog { issue: Some(1762) })
						// let input = NodeInput::network(generic!(T), output_node_connector_index);
						// responses.add(NodeGraphMessage::SetNodeInput {
						// 	node_id: network.exports_metadata.0,
						// 	input_index,
						// 	input,
						// });
						// responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				}
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::Copy => {
				// If the selected nodes are in the document network, use the document network. Otherwise, use the nested network
				let network_path = if selected_nodes
					.selected_nodes_ref()
					.iter()
					.any(|node_id| document_network.nodes.contains_key(node_id) || document_network.exports_metadata.0 == *node_id || document_network.imports_metadata.0 == *node_id)
				{
					Vec::new()
				} else {
					self.network.clone()
				};
				let Some(network) = document_network.nested_network(&network_path) else {
					warn!("No network in NodeGraphMessage::Copy ");
					return;
				};

				// Collect the selected nodes
				let new_ids = &selected_nodes.selected_nodes(network).copied().enumerate().map(|(new, old)| (old, NodeId(new as u64))).collect();
				let copied_nodes = Self::copy_nodes(document_network, &network_path, &self.resolved_types, new_ids).collect::<Vec<_>>();

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
				ModifyInputsContext::delete_nodes(document_network, selected_nodes, node_ids, reconnect, responses, self.network.clone(), &self.resolved_types);

				// Load structure if the selected network is the document network
				if self.network.is_empty() {
					load_network_structure(document_network, document_metadata, collapsed);
				}

				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			// Deletes selected_nodes. If `reconnect` is true, then all children nodes (secondary input) of the selected nodes are deleted and the siblings (primary input/output) are reconnected.
			// If `reconnect` is false, then only the selected nodes are deleted and not reconnected.
			NodeGraphMessage::DeleteSelectedNodes { reconnect } => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
					warn!("No network");
					return;
				};

				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::DeleteNodes {
					node_ids: selected_nodes.selected_nodes(network).copied().collect(),
					reconnect,
				});
			}
			// Input_index is the visible input index, not the actual input index
			NodeGraphMessage::DisconnectInput { node_id, input_index } => {
				responses.add(DocumentMessage::StartTransaction);

				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, std::iter::once(&node_id)) else {
					return;
				};

				let input_index = NodeGraphMessageHandler::get_input_index(network, node_id, input_index);

				let Some(existing_input) = network.nodes.get(&node_id).map_or_else(|| network.exports.get(input_index), |node| node.inputs.get(input_index)) else {
					warn!("Could not find input for {node_id} at index {input_index} when disconnecting");
					return;
				};

				let tagged_value = TaggedValue::from_type(&ModifyInputsContext::get_input_type(document_network, &self.network, node_id, &self.resolved_types, input_index));

				let mut input = NodeInput::value(tagged_value, true);
				if let NodeInput::Value { exposed, .. } = &mut input {
					*exposed = existing_input.is_exposed();
				}
				if node_id == network.exports_metadata.0 {
					// Since it is only possible to drag the solid line, there must be a root_node_to_restore
					if let Previewing::Yes { .. } = network.previewing {
						responses.add(NodeGraphMessage::StartPreviewingWithoutRestore { node_id });
					}
					// If there is no preview, then disconnect
					else {
						responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });
					}
				} else {
					responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });
				}
				if network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::EnterNestedNetwork { node } => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, std::iter::once(&node)) else {
					return;
				};
				if network.imports_metadata.0 == node || network.exports_metadata.0 == node {
					return;
				}

				if network.nodes.get(&node).and_then(|node| node.implementation.get_network()).is_some() {
					self.network.push(node);
				}

				self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);

				self.update_selected(document_network, selected_nodes, responses);
			}
			NodeGraphMessage::DuplicateSelectedNodes => {
				if let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) {
					responses.add(DocumentMessage::StartTransaction);

					let new_ids = &selected_nodes.selected_nodes(network).map(|&id| (id, NodeId(generate_uuid()))).collect();

					selected_nodes.clear_selected_nodes();
					responses.add(BroadcastEvent::SelectionChanged);

					// Copy the selected nodes
					let copied_nodes = Self::copy_nodes(document_network, &self.network, &self.resolved_types, new_ids).collect::<Vec<_>>();

					// Select the new nodes
					selected_nodes.retain_selected_nodes(|selected_node| network.nodes.contains_key(selected_node));
					selected_nodes.add_selected_nodes(copied_nodes.iter().map(|(node_id, _)| *node_id).collect(), &document_network, &self.network);
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
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, std::iter::once(&node_id)) else {
					return;
				};

				if !self.eligible_to_be_layer(network, node_id) {
					responses.add(NodeGraphMessage::SetToNodeOrLayer { node_id: node_id, is_layer: false })
				}
			}
			NodeGraphMessage::ExitNestedNetwork { steps_back } => {
				selected_nodes.clear_selected_nodes();
				responses.add(BroadcastEvent::SelectionChanged);

				for _ in 0..steps_back {
					self.network.pop();
				}

				self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);
				self.update_selected(document_network, selected_nodes, responses);
			}
			NodeGraphMessage::ExposeInput { node_id, input_index, new_exposed } => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, std::iter::once(&node_id)) else {
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
					// TODO: Should network and node inputs be able to be hidden?
					log::error!("Could not hide/show input: {:?} since it is not NodeInput::Value", input);
					return;
				}

				responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });
				responses.add(NodeGraphMessage::EnforceLayerHasNoMultiParams { node_id });
				responses.add(PropertiesPanelMessage::Refresh);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::InsertNode { node_id, document_node } => {
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, std::iter::once(&node_id)) else {
					return;
				};
				network.insert_node(node_id, document_node);
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
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, std::iter::once(&insert_node_id)) else {
					return;
				};

				let post_node = network.nodes.get(&post_node_id);
				let Some((post_node_input_index, _)) = post_node
					.map_or(&network.exports, |post_node| &post_node.inputs)
					.iter()
					.enumerate()
					.filter(|input| input.1.is_exposed())
					.nth(post_node_input_index)
				else {
					error!("Failed to find input index {post_node_input_index} on node {post_node_id:#?}");
					return;
				};
				let Some(insert_node) = network.nodes.get(&insert_node_id) else {
					error!("Insert node not found");
					return;
				};
				let Some((insert_node_input_index, _)) = insert_node.inputs.iter().enumerate().filter(|input| input.1.is_exposed()).nth(insert_node_input_index) else {
					error!("Failed to find input index {insert_node_input_index} on node {insert_node_id:#?}");
					return;
				};

				responses.add(DocumentMessage::StartTransaction);

				let post_input = NodeInput::node(insert_node_id, insert_node_output_index);
				responses.add(NodeGraphMessage::SetNodeInput {
					node_id: post_node_id,
					input_index: post_node_input_index,
					input: post_input,
				});

				let insert_input = if pre_node_id == network.imports_metadata.0 {
					NodeInput::network(generic!(T), pre_node_output_index)
				} else {
					NodeInput::node(pre_node_id, pre_node_output_index)
				};
				responses.add(NodeGraphMessage::SetNodeInput {
					node_id: insert_node_id,
					input_index: insert_node_input_index,
					input: insert_input,
				});

				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::MoveSelectedNodes { displacement_x, displacement_y } => {
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
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
					network.update_click_target(node_id);
				}

				// Since document structure doesn't change, just update the nodes
				if graph_view_overlay_open {
					let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
						warn!("No network");
						return;
					};
					let wires = Self::collect_wires(network);
					let nodes = self.collect_nodes(document_network, network, &wires);
					responses.add(FrontendMessage::UpdateNodeGraph { nodes, wires });
				}
			}
			NodeGraphMessage::PasteNodes { serialized_nodes } => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
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
					let default_inputs = NodeGraphMessageHandler::get_default_inputs(document_network, &self.network, node_id, &self.resolved_types, &document_node);
					document_node = document_node.map_ids(default_inputs, &new_ids);

					// Insert node into network
					responses.add(NodeGraphMessage::InsertNode { node_id, document_node });
				}

				let nodes = new_ids.values().copied().collect();
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
			}
			NodeGraphMessage::PointerDown { shift_click, control_click, alt_click } => {
				let Some(network) = document_network.nested_network(&self.network) else {
					return;
				};

				let viewport_location = ipp.mouse.position;
				let point = network.node_graph_to_viewport.inverse().transform_point2(viewport_location);
				//log::debug!("point: {point:?}");

				// Alt-click sets the clicked node as previewed
				if alt_click {
					if let Some(clicked_node) = NodeGraphMessageHandler::get_clicked_target(&network.node_click_targets, point) {
						responses.add(NodeGraphMessage::TogglePreview { node_id: clicked_node });
						return;
					}
				}

				if let Some(clicked_input) = NodeGraphMessageHandler::get_clicked_target(&network.input_click_targets, point) {
					log::debug!("Clicked input: {} index: {}", clicked_input.0, clicked_input.1);
				}

				if let Some(clicked_id) = NodeGraphMessageHandler::get_clicked_target(&network.node_click_targets, point) {
					let mut updated_selected = selected_nodes.selected_nodes(network).cloned().collect::<Vec<_>>();
					let mut modified_selected = false;

					// Add to/remove from selection if holding Shift or Ctrl
					if shift_click || control_click {
						modified_selected = true;

						let index = updated_selected.iter().enumerate().find_map(|(i, node_id)| if *node_id == clicked_id { Some(i) } else { None });
						// Remove from selection if already selected
						if let Some(index) = index {
							updated_selected.remove(index);
						}
						// Add to selection if not already selected
						else {
							updated_selected.push(clicked_id);
						};
					}
					// Replace selection with a non-selected node
					else if !updated_selected.contains(&clicked_id) {
						modified_selected = true;
						updated_selected = vec![clicked_id];
					}

					if modified_selected {
						responses.add(NodeGraphMessage::SelectedNodesSet { nodes: updated_selected })
					}
				} else {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() })
				}

				let grid_coordinates = DVec2::new((point.x / 24.).round(), (point.y / 24.).round());
				let rounded_graph_coordinates = grid_coordinates * 24.;

				self.drag_start = Some(rounded_graph_coordinates);
			}
			//TODO: Alt+drag should move all upstream nodes as well
			NodeGraphMessage::PointerMove => {
				if let Some(drag_start) = self.drag_start {
					let Some(network) = document_network.nested_network(&self.network) else {
						return;
					};

					let viewport_location = ipp.mouse.position;
					let mut point = network.node_graph_to_viewport.inverse().transform_point2(viewport_location);
					point = point / 24.;
					let grid_coordinates = DVec2::new(point.x.round(), point.y.round());
					let rounded_graph_coordinates = grid_coordinates * 24.;
					if drag_start != rounded_graph_coordinates {
						let displacement = (rounded_graph_coordinates - drag_start) / 24.;
						responses.add(NodeGraphMessage::MoveSelectedNodes {
							displacement_x: displacement.x as i32,
							displacement_y: displacement.y as i32,
						});
						self.drag_start = Some(rounded_graph_coordinates);
					}
				}
			}
			NodeGraphMessage::PointerUp => {
				self.drag_start = None;
				//log::debug!("pointer up");
			}
			NodeGraphMessage::PrintSelectedNodeCoordinates => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
					warn!("No network");
					return;
				};

				for (_, node_to_print) in network
					.nodes
					.iter()
					.filter(|node_id| selected_nodes.selected_nodes(network).any(|selected_id| selected_id == node_id.0))
				{
					if let DocumentNodeImplementation::Network(network) = &node_to_print.implementation {
						let mut output = "\r\n\r\n".to_string();
						output += &node_to_print.name;
						output += ":\r\n\r\n";
						let mut nodes = network.nodes.iter().collect::<Vec<_>>();
						nodes.sort_by_key(|(a, _)| a.0);
						output += &nodes
							.iter()
							.map(|(_, node)| {
								format!(
									"metadata: DocumentNodeMetadata {{ position: glam::IVec2::new({}, {}) }}, // {}",
									node.metadata.position.x, node.metadata.position.y, node.name
								)
							})
							.collect::<Vec<_>>()
							.join("\r\n");
						output += "\r\n";
						output += &format!(
							"imports_metadata: (NodeId(generate_uuid()), ({}, {}).into()),\r\n",
							network.imports_metadata.1.x, network.imports_metadata.1.y
						);
						output += &format!(
							"exports_metadata: (NodeId(generate_uuid()), ({}, {}).into()),",
							network.exports_metadata.1.x, network.exports_metadata.1.y
						);
						output += "\r\n\r\n";
						// KEEP THIS `debug!()` - Someday we can remove this once this development utility is no longer needed
						log::debug!("{output}");
					}
				}
			}
			NodeGraphMessage::RunDocumentGraph => {
				responses.add(PortfolioMessage::SubmitGraphRender { document_id });
			}
			NodeGraphMessage::SelectedNodesAdd { nodes } => {
				selected_nodes.add_selected_nodes(nodes, document_network, &self.network);
				responses.add(BroadcastEvent::SelectionChanged);
			}
			NodeGraphMessage::SelectedNodesRemove { nodes } => {
				selected_nodes.retain_selected_nodes(|node| !nodes.contains(node));
				responses.add(BroadcastEvent::SelectionChanged);
			}
			NodeGraphMessage::SelectedNodesSet { nodes } => {
				selected_nodes.set_selected_nodes(nodes, document_network, &self.network);
				responses.add(BroadcastEvent::SelectionChanged);
				responses.add(PropertiesPanelMessage::Refresh);
			}
			NodeGraphMessage::SendGraph => {
				self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);
			}
			NodeGraphMessage::SetInputValue { node_id, input_index, value } => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, std::iter::once(&node_id)) else {
					return;
				};
				if let Some(node) = network.nodes.get(&node_id) {
					let input = NodeInput::Value { tagged_value: value, exposed: false };
					responses.add(NodeGraphMessage::SetNodeInput { node_id, input_index, input });
					responses.add(PropertiesPanelMessage::Refresh);
					if (node.name != "Imaginate" || input_index == 0) && network.connected_to_output(node_id) {
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				}
			}
			NodeGraphMessage::SetNodeInput { node_id, input_index, input } => {
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, std::iter::once(&node_id)) else {
					return;
				};
				if ModifyInputsContext::set_input(network, node_id, input_index, input, self.network.is_empty()) {
					load_network_structure(document_network, document_metadata, collapsed);
				}
			}
			NodeGraphMessage::SetQualifiedInputValue { node_id, input_index, value } => {
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, std::iter::once(&node_id)) else {
					return;
				};

				if let Some(node) = network.nodes.get_mut(&node_id) {
					// Extend number of inputs if not already large enough
					if input_index >= node.inputs.len() {
						node.inputs.extend(((node.inputs.len() - 1)..input_index).map(|_| NodeInput::network(generic!(T), 0)));
					}
					node.inputs[input_index] = NodeInput::Value { tagged_value: value, exposed: false };
					if network.connected_to_output(node_id) {
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				}
			}
			// Move all the downstream nodes to the right in the graph to allow space for a newly inserted node
			NodeGraphMessage::ShiftNode { node_id } => {
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, std::iter::once(&node_id)) else {
					return;
				};
				debug_assert!(network.is_acyclic(), "Not acyclic. Network: {network:#?}");
				let outwards_wires = network.collect_outwards_wires();
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
					network.update_click_target(node_id);
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
				for &descendant in outwards_wires.get(&node_id).unwrap_or(&Vec::new()) {
					let shift = required_shift(node_id, descendant, network);
					let mut stack = vec![descendant];
					while let Some(id) = stack.pop() {
						shift_node(id, shift, network);
						stack.extend(outwards_wires.get(&id).unwrap_or(&Vec::new()).iter().copied())
					}
				}

				self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);
			}
			NodeGraphMessage::ToggleSelectedVisibility => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
					return;
				};
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let visible = !selected_nodes.selected_nodes(network).all(|&node_id| network.nodes.get(&node_id).is_some_and(|node| node.visible));

				for &node_id in selected_nodes.selected_nodes(network) {
					responses.add(NodeGraphMessage::SetVisibility { node_id, visible });
				}
			}
			NodeGraphMessage::ToggleVisibility { node_id } => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, std::iter::once(&node_id)) else {
					return;
				};

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
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, std::iter::once(&node_id)) else {
					return;
				};

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
				}

				self.update_selection_action_buttons(document_network, selected_nodes, responses);
				responses.add(PropertiesPanelMessage::Refresh);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::ToggleSelectedLocked => {
				// If node is selected in document network, then ctrl+L should lock it
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
					return;
				};
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let locked = !selected_nodes.selected_nodes(network).all(|&node_id| network.nodes.get(&node_id).is_some_and(|node| node.locked));

				for &node_id in selected_nodes.selected_nodes(network) {
					responses.add(NodeGraphMessage::SetLocked { node_id, locked });
				}
			}
			NodeGraphMessage::SetLocked { node_id, locked } => {
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, std::iter::once(&node_id)) else {
					return;
				};

				let Some(node) = network.nodes.get_mut(&node_id) else { return };
				node.locked = locked;

				if network.connected_to_output(node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				// If change has been made to document_network
				if self.network.is_empty() {
					document_metadata.load_structure(document_network);
				}
				self.update_selection_action_buttons(document_network, selected_nodes, responses);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::ToggleSelectedAsLayersOrNodes => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
					return;
				};

				for node_id in selected_nodes.selected_nodes(network).cloned().collect::<Vec<_>>() {
					let Some(network_mut) = document_network.nested_network_for_selected_nodes_mut(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
						return;
					};
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
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, std::iter::once(&node_id)) else {
					return;
				};

				if is_layer && !self.eligible_to_be_layer(network, node_id) {
					log::error!("Could not set node {node_id} to layer");
					return;
				}

				if let Some(node) = network.nodes.get_mut(&node_id) {
					node.is_layer = is_layer;
				}
				network.update_click_target(node_id);

				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(DocumentMessage::DocumentStructureChanged);
			}
			NodeGraphMessage::SetName { node_id, name } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetNameImpl { node_id, name });
			}
			NodeGraphMessage::SetNameImpl { node_id, name } => {
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, std::iter::once(&node_id)) else {
					return;
				};
				if let Some(node) = network.nodes.get_mut(&node_id) {
					node.alias = name;
					network.update_click_target(node_id);
					self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);
				}
			}
			NodeGraphMessage::StartPreviewingWithoutRestore { node_id } => {
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, std::iter::once(&node_id)) else {
					return;
				};
				network.start_previewing_without_restore();
			}
			NodeGraphMessage::TogglePreview { node_id } => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, std::iter::once(&node_id)) else {
					return;
				};

				if network.imports_metadata.0 == node_id || network.exports_metadata.0 == node_id {
					return;
				}
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::TogglePreviewImpl { node_id });
			}
			NodeGraphMessage::TogglePreviewImpl { node_id } => {
				let toggle_id = node_id;
				let Some(network) = document_network.nested_network_for_selected_nodes_mut(&self.network, std::iter::once(&toggle_id)) else {
					return;
				};

				if let Some(export) = network.exports.get_mut(0) {
					// If there currently an export
					if let NodeInput::Node { node_id, output_index, .. } = export {
						let previous_export_id = *node_id;
						let previous_output_index = *output_index;

						// The export is clicked
						if *node_id == toggle_id {
							// If the current export is clicked and is being previewed end the preview and set either export back to root node or disconnect
							if let Previewing::Yes { root_node_to_restore } = network.previewing {
								if let Some(root_node_to_restore) = root_node_to_restore {
									*export = NodeInput::node(root_node_to_restore.id, root_node_to_restore.output_index);
								} else {
									responses.add(NodeGraphMessage::DisconnectInput {
										node_id: network.exports_metadata.0,
										input_index: 0,
									});
								}
								network.stop_preview();
							}
							// The export is clicked and there is no preview
							else {
								network.start_previewing(previous_export_id, previous_output_index);
							}
						}
						// The export is not clicked
						else {
							*export = NodeInput::node(toggle_id, 0);

							// There is currently a dashed line being drawn to the export node
							if let Previewing::Yes { root_node_to_restore } = network.previewing {
								// There is also a solid line being drawn
								if let Some(root_node_to_restore) = root_node_to_restore {
									// If the node with the solid line is clicked, then end preview
									if root_node_to_restore.id == toggle_id {
										network.start_previewing(toggle_id, 0);
									}
								}
								// There is a dashed line without a solid line.
								else {
									network.start_previewing_without_restore();
								}
							}
							// There is no dashed line being drawn
							else {
								network.start_previewing(previous_export_id, previous_output_index);
							}
						}
					}
					// The primary export is disconnected
					else {
						// Set node as export and cancel any preview
						*export = NodeInput::node(toggle_id, 0);
						network.start_previewing_without_restore();
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
		let mut common = actions!(NodeGraphMessageDiscriminant; PointerDown, PointerMove, PointerUp);

		if self.has_selection {
			common.extend(actions!(NodeGraphMessageDiscriminant;
				ToggleSelectedLocked,
				ToggleSelectedVisibility,
			));
		}

		common
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
				PrintSelectedNodeCoordinates,
			)
		} else {
			actions!(NodeGraphMessageDiscriminant;)
		}
	}

	/// Get the clicked target from a mouse click
	fn get_clicked_target<T: Clone>(hashmap: &HashMap<T, ClickTarget>, point: DVec2) -> Option<T> {
		hashmap
			.iter()
			.filter(move |(_, target)| target.intersect_point(point, DAffine2::IDENTITY))
			.map(|(data, _)| data.clone())
			.next()
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
		if let Some(current_network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) {
			let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
				warn!("No network in update_selection_action_buttons");
				return;
			};
			let mut widgets = Vec::new();

			// Don't allow disabling input or output nodes
			let mut selection = selected_nodes
				.selected_nodes(network)
				.filter(|node_id| **node_id != network.imports_metadata.0 && **node_id != network.exports_metadata.0);

			// If there is at least one other selected node then show the hide or show button
			if selection.next().is_some() {
				// Check if any of the selected nodes are disabled
				let all_visible = selected_nodes.selected_nodes(network).all(|id| {
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

			let mut selection = selected_nodes.selected_nodes(network);
			// If only one node is selected then show the preview or stop previewing button
			if let (Some(&node_id), None) = (selection.next(), selection.next()) {
				// Is this node the current output
				let is_output = network.outputs_contain(node_id);
				let is_previewing = matches!(network.previewing, Previewing::Yes { .. });

				// Prevent showing "End Preview"/"Preview" if the root node is the output, or the import/export node
				let is_import_or_export = node_id == network.imports_metadata.0 || node_id == network.exports_metadata.0;
				if !is_import_or_export && network == current_network {
					let output_button = TextButton::new(if is_output && is_previewing { "End Preview" } else { "Preview" })
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
		// If the selected nodes are in the document network, use the document network. Otherwise, use the nested network
		let Some(network) = context
			.document_network
			.nested_network_for_selected_nodes(&context.nested_path.to_vec(), selected_nodes.selected_nodes(context.document_network))
		else {
			warn!("No network in collate_properties");
			return Vec::new();
		};

		// We want:
		// - If only nodes (no layers) are selected: display each node's properties
		// - If one layer is selected, and zero or more of its upstream nodes: display the properties for the layer and its upstream nodes
		// - If multiple layers are selected, or one node plus other non-upstream nodes: display nothing

		// First, we filter all the selections into layers and nodes
		let (mut layers, mut nodes) = (Vec::new(), Vec::new());
		for node_id in selected_nodes.selected_nodes(network) {
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

	fn collect_wires(network: &NodeNetwork) -> Vec<FrontendNodeWire> {
		let mut wires = network
			.nodes
			.iter()
			.flat_map(|(wire_end, node)| node.inputs.iter().filter(|input| input.is_exposed()).enumerate().map(move |(index, input)| (input, wire_end, index)))
			.filter_map(|(input, &wire_end, wire_end_input_index)| {
				if let NodeInput::Node {
					node_id: wire_start,
					output_index: wire_start_output_index,
					// TODO: add ui for lambdas
					lambda: _,
				} = *input
				{
					Some(FrontendNodeWire {
						wire_start,
						wire_start_output_index,
						wire_end,
						wire_end_input_index,
						dashed: false,
					})
				} else if let NodeInput::Network { import_index, .. } = *input {
					Some(FrontendNodeWire {
						wire_start: network.imports_metadata.0,
						wire_start_output_index: import_index,
						wire_end,
						wire_end_input_index,
						dashed: false,
					})
				} else {
					None
				}
			})
			.collect::<Vec<_>>();

		// Connect primary export to root node, since previewing a node will change the primary export
		if let Some(root_node) = network.get_root_node() {
			wires.push(FrontendNodeWire {
				wire_start: root_node.id,
				wire_start_output_index: root_node.output_index,
				wire_end: network.exports_metadata.0,
				wire_end_input_index: 0,
				dashed: false,
			});
		}

		// Connect rest of exports to their actual export field since they are not affected by previewing. Only connect the primary export if it is dashed
		for (i, export) in network.exports.iter().enumerate() {
			if let NodeInput::Node { node_id, output_index, .. } = export {
				let dashed = matches!(network.previewing, Previewing::Yes { .. }) && i == 0;
				if dashed || i != 0 {
					wires.push(FrontendNodeWire {
						wire_start: *node_id,
						wire_start_output_index: *output_index,
						wire_end: network.exports_metadata.0,
						wire_end_input_index: i,
						dashed,
					});
				}
			}
		}
		wires
	}

	fn collect_nodes(&self, document_network: &NodeNetwork, network: &NodeNetwork, wires: &[FrontendNodeWire]) -> Vec<FrontendNode> {
		let connected_node_to_output_lookup = wires
			.iter()
			.map(|wire| ((wire.wire_start, wire.wire_start_output_index), (wire.wire_end, wire.wire_end_input_index)))
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
			let node_definition = document_node_types::resolve_document_node_type(&node.name);

			let frontend_graph_inputs = node.inputs.iter().enumerate().map(|(index, _)| {
				// Convert the index in all inputs to the index in only the exposed inputs
				// TODO: Only display input type if potential inputs in node_registry are all the same type
				let input_type = self.resolved_types.inputs.get(&Source { node: node_id_path.clone(), index }).cloned();

				// TODO: Should display the color of the "most commonly relevant" (we'd need some sort of precedence) data type it allows given the current generic form that's constrained by the other present connections.
				let frontend_data_type = if let Some(ref input_type) = input_type {
					FrontendGraphDataType::with_type(input_type)
				} else {
					FrontendGraphDataType::General
				};

				let definition_name = node_definition.and_then(|node_definition| {
					let node_implementation = &node.implementation;
					let definition_implementation = &node_definition.implementation;

					// Only use definition input names if the node implementation is the same as the definition implementation
					if std::mem::discriminant(node_implementation) == std::mem::discriminant(definition_implementation) {
						node_definition.inputs.get(index).map(|input| input.name.to_string())
					} else {
						None
					}
				});

				let input_name = definition_name.unwrap_or(
					ModifyInputsContext::get_input_type(document_network, &self.network, node_id, &self.resolved_types, index)
						.nested_type()
						.to_string(),
				);

				FrontendGraphInput {
					data_type: frontend_data_type,
					name: input_name,
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

			let primary_input = inputs
				.next()
				.filter(|(input, _)| {
					// Don't show EditorApi input to nodes like "Text" in the document network
					if document_network == network && matches!(input, NodeInput::Network { .. }) {
						false
					} else {
						input.is_exposed()
					}
				})
				.map(|(_, input_type)| input_type);
			let exposed_inputs = inputs
				.filter(|(input, _)| input.is_exposed() && !(matches!(input, NodeInput::Network { .. }) && document_network == network))
				.map(|(_, input_type)| input_type)
				.collect();

			let output_types = Self::get_output_types(node, &self.resolved_types, &node_id_path);
			let primary_output_type = output_types.get(0).expect("Primary output should always exist");
			let frontend_data_type = if let Some(output_type) = primary_output_type {
				FrontendGraphDataType::with_type(&output_type)
			} else {
				FrontendGraphDataType::General
			};
			let (connected, connected_index) = connected_node_to_output_lookup.get(&(node_id, 0)).unwrap_or(&(Vec::new(), Vec::new())).clone();
			let primary_output = if node.has_primary_output {
				Some(FrontendGraphOutput {
					data_type: frontend_data_type,
					name: "Output 1".to_string(),
					resolved_type: primary_output_type.clone().map(|input| format!("{input:?}")),
					connected,
					connected_index,
				})
			} else {
				None
			};

			let mut exposed_outputs = Vec::new();
			for (index, exposed_output) in output_types.iter().enumerate() {
				if index == 0 && node.has_primary_output {
					continue;
				}
				let frontend_data_type = if let Some(output_type) = &exposed_output {
					FrontendGraphDataType::with_type(output_type)
				} else {
					FrontendGraphDataType::General
				};

				let output_name = node_definition
					.and_then(|node_definition| {
						// If a node has multiple outputs, node and definition must have Network implementations
						node_definition.outputs.get(index).map(|output| output.name.to_string())
					})
					.unwrap_or(format!("Output {}", index + 1));

				let (connected, connected_index) = connected_node_to_output_lookup.get(&(node_id, index)).unwrap_or(&(Vec::new(), Vec::new())).clone();
				exposed_outputs.push(FrontendGraphOutput {
					data_type: frontend_data_type,
					name: output_name,
					resolved_type: exposed_output.clone().map(|input| format!("{input:?}")),
					connected,
					connected_index,
				});
			}
			let is_export = network.exports.get(0).is_some_and(|export| export.as_node().is_some_and(|export_node_id| node_id == export_node_id));
			let is_root_node = network.get_root_node().is_some_and(|root_node| root_node.id == node_id);
			let previewed = is_export && !is_root_node;

			let errors = self
				.node_graph_errors
				.iter()
				.find(|error| error.node_path == *node_id_path)
				.map(|error| format!("{:?}", error.error.clone()))
				.or_else(|| {
					if self.node_graph_errors.iter().any(|error| error.node_path.starts_with(node_id_path)) {
						Some("Node graph type error within this node".to_string())
					} else {
						None
					}
				});

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
				previewed,
				visible: node.visible,
				locked: node.locked,
				errors: errors,
				ui_only: false,
			});
		}

		// Get import/export names from parent node definition input/outputs. None means to use type, or "Import/Export + index" if type can't be determined
		let mut import_names = Vec::new();
		let mut export_names = vec![None; network.exports.len()];

		let mut encapsulating_path = self.network.clone();
		if let Some(encapsulating_node) = encapsulating_path.pop() {
			let parent_node = document_network
				.nested_network(&encapsulating_path)
				.expect("Encapsulating path should always exist")
				.nodes
				.get(&encapsulating_node)
				.expect("Last path node should always exist in encapsulating network");

			let parent_definition = document_node_types::resolve_document_node_type(&parent_node.name);
			let node_implementation = &parent_node.implementation;

			// Get all import names from definition
			for (index, _) in parent_node.inputs.iter().enumerate() {
				let definition_name = parent_definition.and_then(|node_definition| {
					// Only use definition input names if the parent implementation is the same as the definition implementation
					let definition_implementation = &node_definition.implementation;
					if std::mem::discriminant(node_implementation) == std::mem::discriminant(definition_implementation) {
						node_definition.inputs.get(index).map(|input| input.name.to_string())
					} else {
						None
					}
				});

				import_names.push(definition_name);
			}

			// Get all export names from definition
			for (index, _) in network.exports.iter().enumerate() {
				let definition_name = parent_definition.and_then(|node_definition| {
					// Only use definition input names if the parent implementation is the same as the definition implementation
					let definition_implementation = &node_definition.implementation;
					if std::mem::discriminant(node_implementation) == std::mem::discriminant(definition_implementation) {
						node_definition.outputs.get(index).map(|output| output.name.to_string())
					} else {
						None
					}
				});
				export_names[index] = definition_name;
			}
		}

		// Add "Export" UI-only node
		let mut export_node_inputs = Vec::new();
		for (index, export) in network.exports.iter().enumerate() {
			let (frontend_data_type, input_type) = if let NodeInput::Node { node_id, output_index, .. } = export {
				let node = network.nodes.get(node_id).expect("Node should always exist");
				let node_id_path = &[&self.network[..], &[*node_id]].concat();
				let output_types = Self::get_output_types(node, &self.resolved_types, &node_id_path);

				if let Some(output_type) = output_types.get(*output_index).cloned().flatten() {
					(FrontendGraphDataType::with_type(&output_type), Some(output_type.clone()))
				} else {
					(FrontendGraphDataType::General, None)
				}
			}
			// If type should only be determined from resolved_types then remove this
			else if let NodeInput::Value { tagged_value, .. } = export {
				(FrontendGraphDataType::with_type(&tagged_value.ty()), Some(tagged_value.ty()))
			} else if let NodeInput::Network { import_type, .. } = export {
				(FrontendGraphDataType::with_type(import_type), Some(import_type.clone()))
			} else {
				(FrontendGraphDataType::General, None)
			};

			// First import index is visually connected to the root node instead of its actual export input so previewing does not change the connection
			let connected = if index == 0 {
				network.get_root_node().map(|root_node| root_node.id)
			} else {
				if let NodeInput::Node { node_id, .. } = export {
					Some(*node_id)
				} else {
					None
				}
			};

			let definition_name = export_names[index].clone();

			// `export_names` is pre-initialized with None, so this is safe
			let export_name = definition_name
				.or(input_type.clone().map(|input_type| TaggedValue::from_type(&input_type).ty().to_string()))
				.unwrap_or(format!("Export {}", index + 1));

			export_node_inputs.push(FrontendGraphInput {
				data_type: frontend_data_type,
				name: export_name,
				resolved_type: input_type.map(|input| format!("{input:?}")),
				connected,
			});
		}
		// Display error for document network export node
		let errors = self
			.node_graph_errors
			.iter()
			.find(|error| error.node_path.is_empty() && self.network.is_empty())
			.map(|error| format!("{:?}", error.error.clone()));

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
			errors,
			ui_only: true,
		});

		// Add "Import" UI-only node
		if document_network != network {
			let mut import_node_outputs = Vec::new();
			for (index, definition_name) in import_names.into_iter().enumerate() {
				let (connected, connected_index) = connected_node_to_output_lookup.get(&(network.imports_metadata.0, index)).unwrap_or(&(Vec::new(), Vec::new())).clone();
				// TODO: https://github.com/GraphiteEditor/Graphite/issues/1767
				// TODO: Non exposed inputs are not added to the inputs_source_map, fix `pub fn document_node_types(&self) -> ResolvedDocumentNodeTypes`
				let input_type = self.resolved_types.inputs.get(&Source { node: self.network.clone(), index }).cloned();

				let frontend_data_type = if let Some(input_type) = input_type.clone() {
					FrontendGraphDataType::with_type(&input_type)
				} else {
					FrontendGraphDataType::General
				};

				let import_name = definition_name
					.or(input_type.clone().map(|input_type| TaggedValue::from_type(&input_type).ty().to_string()))
					.unwrap_or(format!("Import {}", index + 1));

				import_node_outputs.push(FrontendGraphOutput {
					data_type: frontend_data_type,
					name: import_name,
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

	fn collect_subgraph_names(subgraph_path: &mut Vec<NodeId>, network: &NodeNetwork) -> Option<Vec<String>> {
		let mut current_network = network;
		let mut subraph_names = Vec::new();
		for node_id in subgraph_path.iter() {
			let Some(node) = current_network.nodes.get(node_id) else {
				// If node cannot be found and we are in a nested network, set subgraph_path to document network and return None, which runs send_graph again on the document network
				if !subgraph_path.is_empty() {
					subgraph_path.clear();
					return None;
				} else {
					return Some(Vec::new());
				}
			};
			if let Some(network) = node.implementation.get_network() {
				current_network = network;
			}

			// TODO: Maybe replace with alias and default to name if it does not exist
			subraph_names.push(node.name.clone());
		}
		Some(subraph_names)
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
					parent_id: layer.parent(metadata).and_then(|parent| if parent != LayerNodeIdentifier::ROOT_PARENT { Some(parent.to_node()) } else { None }),
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

	fn send_graph(&mut self, document_network: &NodeNetwork, metadata: &mut DocumentMetadata, collapsed: &CollapsedLayers, graph_open: bool, responses: &mut VecDeque<Message>) {
		// If a node cannot be found in collect_subgraph_names, and we are in a nested network, set self.network to empty (document network), and call send_graph again to send the document network
		let Some(nested_path) = Self::collect_subgraph_names(&mut self.network, document_network) else {
			self.send_graph(document_network, metadata, collapsed, graph_open, responses);
			return;
		};

		let Some(network) = document_network.nested_network(&self.network) else {
			log::error!("Could not send graph since nested network does not exist");
			return;
		};

		// View encapsulating network
		responses.add(DocumentMessage::DocumentStructureChanged);
		responses.add(PropertiesPanelMessage::Refresh);

		metadata.load_structure(document_network);

		Self::update_layer_panel(document_network, metadata, collapsed, responses);

		if graph_open {
			let wires = Self::collect_wires(network);
			let nodes = self.collect_nodes(document_network, network, &wires);

			responses.add(FrontendMessage::UpdateNodeGraph { nodes, wires });
			responses.add(FrontendMessage::UpdateSubgraphPath { subgraph_path: nested_path })
		}
	}

	pub fn get_output_types(node: &DocumentNode, resolved_types: &ResolvedDocumentNodeTypes, node_id_path: &Vec<NodeId>) -> Vec<Option<Type>> {
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
					// Current export is pointing to a proto node where type can be derived
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

	/// Returns an iterator of nodes to be copied and their ids, excluding output and input nodes
	pub fn copy_nodes<'a>(
		document_network: &'a NodeNetwork,
		network_path: &'a Vec<NodeId>,
		resolved_types: &'a ResolvedDocumentNodeTypes,
		new_ids: &'a HashMap<NodeId, NodeId>,
	) -> impl Iterator<Item = (NodeId, DocumentNode)> + 'a {
		new_ids
			.iter()
			.filter_map(|(&id, &new)| {
				document_network
					.nested_network(network_path)
					.and_then(|network| network.nodes.get(&id).map(|node| (new, id, node.clone())))
			})
			.map(move |(new, node_id, node)| {
				let default_inputs = NodeGraphMessageHandler::get_default_inputs(document_network, network_path, node_id, resolved_types, &node);
				(new, node.map_ids(default_inputs, new_ids))
			})
	}

	pub fn get_default_inputs(document_network: &NodeNetwork, network_path: &Vec<NodeId>, node_id: NodeId, resolved_types: &ResolvedDocumentNodeTypes, node: &DocumentNode) -> Vec<NodeInput> {
		let mut default_inputs = Vec::new();

		for (input_index, input) in node.inputs.iter().enumerate() {
			let tagged_value = TaggedValue::from_type(&ModifyInputsContext::get_input_type(document_network, network_path, node_id, resolved_types, input_index));
			let mut exposed = true;

			if let NodeInput::Value { exposed: input_exposed, .. } = input {
				exposed = *input_exposed;
			}

			let default_input = NodeInput::value(tagged_value, exposed);
			default_inputs.push(default_input);
		}
		default_inputs
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
			.filter(|input| matches!(input, NodeInput::Node { .. }) || matches!(input, NodeInput::Network { .. }))
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
			.then_some(node.alias.to_string())
			.unwrap_or(if node.is_layer && node.name == "Merge" { "Untitled Layer".to_string() } else { node.name.clone() })
	}

	/// Get the actual input index from the visible input index where hidden inputs are skipped
	fn get_input_index(network: &NodeNetwork, node_id: NodeId, visible_index: usize) -> usize {
		if network.exports_metadata.0 != node_id {
			let Some(input_node) = network.nodes.get(&node_id) else {
				error!("Could not get node {node_id} in get_input_index");
				return 0;
			};
			let input_index = input_node
				.inputs
				.iter()
				.enumerate()
				.filter(|input| input.1.is_exposed())
				.nth(visible_index)
				.map(|enumerated_input| enumerated_input.0);

			let Some(input_index) = input_index else {
				error!("Failed to find actual index of connector index {visible_index} on node {node_id:#?}");
				return 0;
			};
			input_index
		} else {
			visible_index
		}
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
			drag_start: None,
		}
	}
}
