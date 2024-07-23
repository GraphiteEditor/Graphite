use super::utility_types::{BoxSelection, ContextMenuInformation, DragStart, FrontendGraphInput, FrontendGraphOutput, FrontendNode, FrontendNodeWire, NodeMetadata, WirePath};
use super::{document_node_types, node_properties};
use crate::application::generate_uuid;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::graph_operation::load_network_structure;
use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::node_graph::document_node_types::NodePropertiesContext;
use crate::messages::portfolio::document::node_graph::utility_types::{ContextMenuData, FrontendGraphDataType};
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, LayerPanelEntry, SelectedNodes};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;

use bezier_rs::Subpath;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, FlowType, NodeId, NodeInput, NodeNetwork, Previewing, Source};
use graph_craft::proto::GraphErrors;
use graphene_core::*;
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypes;
use renderer::{ClickTarget, Quad};
use vector::PointId;

use glam::{DAffine2, DVec2, IVec2, UVec2};

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
	pub node_graph_to_viewport: &'a DAffine2,
}

#[derive(Debug, Clone)]
pub struct NodeGraphMessageHandler {
	pub network: Vec<NodeId>,
	pub resolved_types: ResolvedDocumentNodeTypes,
	pub node_graph_errors: GraphErrors,
	has_selection: bool,
	widgets: [LayoutGroup; 2],
	drag_start: Option<DragStart>,
	/// Used to add a transaction for the first node move when dragging.
	begin_dragging: bool,
	/// Stored in pixel coordinates.
	box_selection_start: Option<UVec2>,
	disconnecting: Option<(NodeId, usize)>,
	initial_disconnecting: bool,
	/// Node to select on pointer up if multiple nodes are selected and they were not dragged.
	select_if_not_dragged: Option<NodeId>,
	/// The start of the dragged line that cannot be moved. The bool represents if it is a vertical output.
	wire_in_progress_from_connector: Option<(DVec2, bool)>,
	/// The end point of the dragged line that can be moved. The bool represents if it is a vertical input.
	wire_in_progress_to_connector: Option<(DVec2, bool)>,
	/// State for the context menu popups.
	context_menu: Option<ContextMenuInformation>,
	/// Click targets for every node in the network by using the path to that node.
	pub node_metadata: HashMap<NodeId, NodeMetadata>,
	/// Cache for the bounding box around all nodes in node graph space.
	pub bounding_box_subpath: Option<Subpath<PointId>>,
	/// Index of selected node to be deselected on pointer up when shift clicking an already selected node
	pub deselect_on_pointer_up: Option<usize>,
	/// Adds the auto panning functionality to the node graph when dragging a node or selection box to the edge of the viewport.
	auto_panning: AutoPanning,
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
			node_graph_to_viewport,
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
				responses.add(DocumentMessage::DocumentStructureChanged);
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
			NodeGraphMessage::CloseCreateNodeMenu => {
				self.context_menu = None;
				responses.add(FrontendMessage::UpdateContextMenuInformation {
					context_menu_information: self.context_menu.clone(),
				});
				self.wire_in_progress_from_connector = None;
				self.wire_in_progress_to_connector = None;
				responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: None });
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

				let document_node = document_node_type.to_document_node(
					document_node_type.inputs.iter().map(|input| input.default.clone()),
					graph_craft::document::DocumentNodeMetadata::position((x / 24, y / 24)),
				);
				self.context_menu = None;

				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::InsertNode { node_id, document_node });

				if let Some(wire_in_progress) = self.wire_in_progress_from_connector {
					let Some((from_node, output_index)) = self.get_connector_from_point(wire_in_progress.0, |metadata| &metadata.output_click_targets) else {
						log::error!("Could not get output form connector start");
						return;
					};
					responses.add(NodeGraphMessage::ConnectNodesByWire {
						output_node: from_node,
						output_node_connector_index: output_index,
						input_node: node_id,
						input_node_connector_index: 0,
					});
					self.wire_in_progress_from_connector = None;
					self.wire_in_progress_to_connector = None;
				}

				responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: None });
				responses.add(FrontendMessage::UpdateContextMenuInformation {
					context_menu_information: self.context_menu.clone(),
				});
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::Cut => {
				responses.add(NodeGraphMessage::Copy);
				responses.add(NodeGraphMessage::DeleteSelectedNodes { reconnect: true });
			}
			NodeGraphMessage::DeleteNodes { node_ids, reconnect } => {
				ModifyInputsContext::delete_nodes(self, document_network, selected_nodes, node_ids, reconnect, responses, self.network.clone());

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
					selected_nodes.add_selected_nodes(copied_nodes.iter().map(|(node_id, _)| *node_id).collect(), document_network, &self.network);
					responses.add(BroadcastEvent::SelectionChanged);

					for (node_id, mut document_node) in copied_nodes {
						// Shift duplicated node
						document_node.metadata.position += IVec2::splat(2);

						// Insert new node into graph
						responses.add(NodeGraphMessage::InsertNode { node_id, document_node });
					}

					self.update_selected(document_network, selected_nodes, responses);
					responses.add(NodeGraphMessage::SendGraph);
				}
			}
			NodeGraphMessage::EnforceLayerHasNoMultiParams { node_id } => {
				let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, std::iter::once(&node_id)) else {
					return;
				};

				if !self.eligible_to_be_layer(network, node_id) {
					responses.add(NodeGraphMessage::SetToNodeOrLayer { node_id, is_layer: false })
				}
			}
			NodeGraphMessage::EnterNestedNetwork => {
				let Some(network) = document_network.nested_network_mut(&self.network) else { return };

				let viewport_location = ipp.mouse.position;
				let point = node_graph_to_viewport.inverse().transform_point2(viewport_location);
				let Some(node_id) = self.get_node_from_point(point) else { return };

				if self.get_visibility_from_point(point).is_some() {
					return;
				};
				if network.imports_metadata.0 == node_id || network.exports_metadata.0 == node_id {
					return;
				}

				let Some(node) = network.nodes.get_mut(&node_id) else { return };
				if let DocumentNodeImplementation::Network(_) = node.implementation {
					self.network.push(node_id);
					self.node_metadata.clear();

					self.update_all_click_targets(document_network, self.network.clone());

					responses.add(DocumentMessage::ZoomCanvasToFitAll);
				}

				responses.add(DocumentMessage::ResetTransform);
				responses.add(NodeGraphMessage::SendGraph);

				self.update_selected(document_network, selected_nodes, responses);
			}
			NodeGraphMessage::ExitNestedNetwork { steps_back } => {
				selected_nodes.clear_selected_nodes();
				responses.add(BroadcastEvent::SelectionChanged);

				for _ in 0..steps_back {
					self.network.pop();
				}
				self.node_metadata.clear();
				self.update_all_click_targets(document_network, self.network.clone());
				responses.add(DocumentMessage::ResetTransform);
				responses.add(NodeGraphMessage::SendGraph);
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
				let network_path = if document_network.nodes.contains_key(&node_id) { Vec::new() } else { self.network.clone() };
				self.insert_node(node_id, document_node, document_network, &network_path);
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
			NodeGraphMessage::MoveSelectedNodes {
				displacement_x,
				displacement_y,
				move_upstream,
			} => {
				let network_path = if selected_nodes.selected_nodes_ref().iter().any(|node_id| document_network.nodes.contains_key(node_id)) {
					Vec::new()
				} else {
					self.network.clone()
				};

				let Some(network) = document_network.nested_network(&network_path) else {
					warn!("No network");
					return;
				};
				let mut nodes_to_move = selected_nodes.selected_nodes(network).cloned().collect::<HashSet<_>>();
				if move_upstream {
					for selected_node_id in selected_nodes.selected_nodes(network) {
						let Some(selected_node) = network.nodes.get(selected_node_id) else {
							log::error!("Could not get selected node from network");
							continue;
						};
						// Only drag nodes that are children of the selected layer
						if let Some(NodeInput::Node { node_id, .. }) = selected_node.inputs.get(1) {
							nodes_to_move.extend(network.upstream_flow_back_from_nodes(vec![*node_id], FlowType::UpstreamFlow).map(|(_, node_id)| node_id))
						};
					}
				}
				for node_id in nodes_to_move {
					if document_network.nested_network(&network_path).unwrap().exports_metadata.0 == node_id {
						let network = document_network.nested_network_mut(&network_path).unwrap();
						network.exports_metadata.1 += IVec2::new(displacement_x, displacement_y);
					} else if document_network.nested_network(&network_path).unwrap().imports_metadata.0 == node_id {
						let network = document_network.nested_network_mut(&network_path).unwrap();
						network.imports_metadata.1 += IVec2::new(displacement_x, displacement_y);
					} else if let Some(node) = document_network.nested_network_mut(&network_path).unwrap().nodes.get_mut(&node_id) {
						node.metadata.position += IVec2::new(displacement_x, displacement_y)
					}
					self.update_click_target(node_id, document_network, network_path.clone());
				}

				// TODO: Cache all nodes and wires in the network, only update the moved node/connected wires, and send all nodes to the front end.
				// Since document structure doesn't change, just update the nodes
				if graph_view_overlay_open {
					let Some(network) = document_network.nested_network_for_selected_nodes(&self.network, selected_nodes.selected_nodes_ref().iter()) else {
						warn!("No network");
						return;
					};
					let wires = Self::collect_wires(network);
					let nodes = self.collect_nodes(document_network, network, &wires);
					responses.add(FrontendMessage::UpdateNodeGraph { nodes, wires });
					responses.add(DocumentMessage::RenderRulers);
					responses.add(DocumentMessage::RenderScrollbars);
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
			NodeGraphMessage::PointerDown {
				shift_click,
				control_click,
				alt_click,
				right_click,
			} => {
				let Some(network) = document_network.nested_network(&self.network) else {
					return;
				};

				let viewport_location = ipp.mouse.position;
				let point = node_graph_to_viewport.inverse().transform_point2(viewport_location);

				if let Some(clicked_visibility) = self.get_visibility_from_point(point) {
					responses.add(NodeGraphMessage::ToggleVisibility { node_id: clicked_visibility });
					return;
				}

				let clicked_id = self.get_node_from_point(point);
				let clicked_input = self.get_connector_from_point(point, |metadata| &metadata.input_click_targets);
				let clicked_output = self.get_connector_from_point(point, |metadata| &metadata.output_click_targets);

				// Create the add node popup on right click, then exit
				if right_click {
					let context_menu_data = if let Some((node_id, node)) = clicked_id.and_then(|node_id| network.nodes.get(&node_id).map(|node| (node_id, node))) {
						ContextMenuData::ToggleLayer {
							node_id,
							currently_is_node: !node.is_layer,
						}
					} else {
						ContextMenuData::CreateNode
					};

					// TODO: Create function
					let node_graph_shift = if matches!(context_menu_data, ContextMenuData::CreateNode) {
						let appear_right_of_mouse = if viewport_location.x > ipp.viewport_bounds.size().x - 180. { -180. } else { 0. };
						let appear_above_mouse = if viewport_location.y > ipp.viewport_bounds.size().y - 200. { -200. } else { 0. };
						DVec2::new(appear_right_of_mouse, appear_above_mouse) / node_graph_to_viewport.matrix2.x_axis.x
					} else {
						let appear_right_of_mouse = if viewport_location.x > ipp.viewport_bounds.size().x - 173. { -173. } else { 0. };
						let appear_above_mouse = if viewport_location.y > ipp.viewport_bounds.size().y - 34. { -34. } else { 0. };
						DVec2::new(appear_right_of_mouse, appear_above_mouse) / node_graph_to_viewport.matrix2.x_axis.x
					};

					let context_menu_coordinates = ((point.x + node_graph_shift.x) as i32, (point.y + node_graph_shift.y) as i32);

					self.context_menu = Some(ContextMenuInformation {
						context_menu_coordinates,
						context_menu_data,
					});

					responses.add(FrontendMessage::UpdateContextMenuInformation {
						context_menu_information: self.context_menu.clone(),
					});

					return;
				}

				// If the user is clicking on the create nodes list or context menu, break here
				if let Some(context_menu) = &self.context_menu {
					let context_menu_viewport = node_graph_to_viewport.transform_point2(DVec2::new(context_menu.context_menu_coordinates.0 as f64, context_menu.context_menu_coordinates.1 as f64));
					let (width, height) = if matches!(context_menu.context_menu_data, ContextMenuData::ToggleLayer { .. }) {
						// Height and width for toggle layer menu
						(173., 34.)
					} else {
						// Height and width for create node menu
						(180., 200.)
					};
					let context_menu_subpath = bezier_rs::Subpath::new_rounded_rect(
						DVec2::new(context_menu_viewport.x, context_menu_viewport.y),
						DVec2::new(context_menu_viewport.x + width, context_menu_viewport.y + height),
						[5.; 4],
					);
					let context_menu_click_target = ClickTarget {
						subpath: context_menu_subpath,
						stroke_width: 1.,
					};
					if context_menu_click_target.intersect_point(viewport_location, DAffine2::IDENTITY) {
						return;
					}
				}

				// Since the user is clicking elsewhere in the graph, ensure the add nodes list is closed
				if !right_click && self.context_menu.is_some() {
					self.context_menu = None;
					self.wire_in_progress_from_connector = None;
					self.wire_in_progress_to_connector = None;
					responses.add(FrontendMessage::UpdateContextMenuInformation {
						context_menu_information: self.context_menu.clone(),
					});
					responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: None });
				}

				// Alt-click sets the clicked node as previewed
				if alt_click {
					if let Some(clicked_node) = clicked_id {
						responses.add(NodeGraphMessage::TogglePreview { node_id: clicked_node });
						return;
					}
				}

				// Input: Begin moving an existing wire
				if let Some(clicked_input) = clicked_input {
					self.initial_disconnecting = true;
					let input_index = NodeGraphMessageHandler::get_input_index(network, clicked_input.0, clicked_input.1);
					if let Some(NodeInput::Node { node_id, output_index, .. }) = network
						.nodes
						.get(&clicked_input.0)
						.and_then(|clicked_node| clicked_node.inputs.get(input_index))
						.or(network.exports.get(input_index))
					{
						self.disconnecting = Some((clicked_input.0, clicked_input.1));
						let Some(output_node) = network.nodes.get(node_id) else {
							log::error!("Could not find node {}", node_id);
							return;
						};
						self.wire_in_progress_from_connector = if output_node.is_layer {
							Some((
								DVec2::new(output_node.metadata.position.x as f64 * 24. + 2. * 24., output_node.metadata.position.y as f64 * 24. - 24. / 2.),
								true,
							))
						} else {
							// The 4.95 is to ensure wire generated here aligns with the frontend wire when the mouse is moved within a node connector, but the wire is not disconnected yet. Eventually all wires should be generated in Rust so that all positions will be aligned.
							Some((
								DVec2::new(
									output_node.metadata.position.x as f64 * 24. + 5. * 24. + 4.95,
									output_node.metadata.position.y as f64 * 24. + 24. + 24. * *output_index as f64,
								),
								false,
							))
						};
					} else if let Some(NodeInput::Network { import_index, .. }) = network.nodes.get(&clicked_input.0).and_then(|clicked_node| clicked_node.inputs.get(input_index)) {
						self.disconnecting = Some((clicked_input.0, clicked_input.1));

						self.wire_in_progress_from_connector =
							// The 4.95 is to ensure wire generated here aligns with the frontend wire when the mouse is moved within a node connector, but the wire is not disconnected yet. Eventually all wires should be generated in Rust so that all positions will be aligned.
							Some((
								DVec2::new(
									network.imports_metadata.1.x as f64 * 24. + 5. * 24. + 4.95,
									network.imports_metadata.1.y as f64 * 24. + 48. + 24. * *import_index as f64,
								),
								false,
							))
					}

					return;
				}

				if let Some(clicked_output) = clicked_output {
					self.initial_disconnecting = false;

					if let Some(clicked_output_node) = network.nodes.get(&clicked_output.0) {
						// Disallow creating additional vertical output wires from an already-connected layer
						if clicked_output_node.is_layer && clicked_output_node.has_primary_output {
							for node in network.nodes.values() {
								if node
									.inputs
									.iter()
									.chain(network.exports.iter())
									.any(|node_input| node_input.as_node().is_some_and(|node_id| node_id == clicked_output.0))
								{
									return;
								}
							}
						}
						self.wire_in_progress_from_connector = if clicked_output_node.is_layer {
							Some((
								DVec2::new(
									clicked_output_node.metadata.position.x as f64 * 24. + 2. * 24.,
									clicked_output_node.metadata.position.y as f64 * 24. - 12.,
								),
								true,
							))
						} else {
							Some((
								DVec2::new(
									clicked_output_node.metadata.position.x as f64 * 24. + 5. * 24.,
									clicked_output_node.metadata.position.y as f64 * 24. + 24. + 24. * clicked_output.1 as f64,
								),
								false,
							))
						};
					} else {
						// Imports node is clicked
						self.wire_in_progress_from_connector = Some((
							DVec2::new(
								network.imports_metadata.1.x as f64 * 24. + 5. * 24.,
								network.imports_metadata.1.y as f64 * 24. + 48. + 24. * clicked_output.1 as f64,
							),
							false,
						));
					};

					return;
				}

				if let Some(clicked_id) = clicked_id {
					let mut updated_selected = selected_nodes.selected_nodes(network).cloned().collect::<Vec<_>>();
					let mut modified_selected = false;

					// Add to/remove from selection if holding Shift or Ctrl
					if shift_click || control_click {
						modified_selected = true;

						let index = updated_selected.iter().enumerate().find_map(|(i, node_id)| if *node_id == clicked_id { Some(i) } else { None });
						// Remove from selection (on PointerUp) if already selected
						self.deselect_on_pointer_up = index;

						// Add to selection if not already selected. Necessary in order to drag multiple nodes
						if index.is_none() {
							updated_selected.push(clicked_id);
						};
					}
					// Replace selection with a non-selected node
					else if !updated_selected.contains(&clicked_id) {
						modified_selected = true;
						updated_selected = vec![clicked_id];
					}
					// Replace selection (of multiple nodes including this one) with just this one, but only upon pointer up if the user didn't drag the selected nodes
					else {
						self.select_if_not_dragged = Some(clicked_id);
					}

					// If this node is selected (whether from before or just now), prepare it for dragging
					if updated_selected.contains(&clicked_id) {
						let drag_start = DragStart {
							start_x: point.x,
							start_y: point.y,
							round_x: 0,
							round_y: 0,
						};

						self.drag_start = Some(drag_start);
						self.begin_dragging = true;
					}

					// Update the selection if it was modified
					if modified_selected {
						responses.add(NodeGraphMessage::SelectedNodesSet { nodes: updated_selected })
					}

					return;
				}

				// Clicked on the graph background so we box select
				if !shift_click {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() })
				}
				self.box_selection_start = Some(UVec2::new(viewport_location.x.round().abs() as u32, viewport_location.y.round().abs() as u32));
			}
			NodeGraphMessage::PointerMove { shift } => {
				let Some(network) = document_network.nested_network(&self.network) else {
					return;
				};

				// Auto-panning
				let messages = [NodeGraphMessage::PointerOutsideViewport { shift }.into(), NodeGraphMessage::PointerMove { shift }.into()];
				self.auto_panning.setup_by_mouse_position(ipp, &messages, responses);

				let viewport_location = ipp.mouse.position;
				let point = node_graph_to_viewport.inverse().transform_point2(viewport_location);

				if self.wire_in_progress_from_connector.is_some() && self.context_menu.is_none() {
					if let Some((to_connector_node_position, is_layer, input_index)) =
						self.get_connector_from_point(point, |metadata| &metadata.input_click_targets).and_then(|(node_id, input_index)| {
							network.nodes.get(&node_id).map(|node| (node.metadata.position, node.is_layer, input_index)).or_else(|| {
								if node_id == network.exports_metadata.0 {
									Some((network.exports_metadata.1 + IVec2::new(0, 1), false, input_index))
								} else if node_id == network.imports_metadata.0 {
									Some((network.imports_metadata.1 + IVec2::new(0, 1), false, input_index))
								} else {
									None
								}
							})
						}) {
						let to_connector_position = if is_layer {
							if input_index == 0 {
								DVec2::new(to_connector_node_position.x as f64 * 24. + 2. * 24., to_connector_node_position.y as f64 * 24. + 2. * 24. + 12.)
							} else {
								DVec2::new(to_connector_node_position.x as f64 * 24., to_connector_node_position.y as f64 * 24. + 24.)
							}
						} else {
							DVec2::new(to_connector_node_position.x as f64 * 24., to_connector_node_position.y as f64 * 24. + input_index as f64 * 24. + 24.)
						};
						self.wire_in_progress_to_connector = Some((to_connector_position, input_index == 0 && is_layer));
					} else if let Some((to_connector_node_position, is_layer, output_index)) =
						self.get_connector_from_point(point, |metadata| &metadata.output_click_targets).and_then(|(node_id, output_index)| {
							network
								.nodes
								.get(&node_id)
								.map(|node| (node.metadata.position, node.is_layer, output_index + if node.has_primary_output { 0 } else { 1 }))
						}) {
						let to_connector_position = if is_layer {
							DVec2::new(to_connector_node_position.x as f64 * 24. + 2. * 24., to_connector_node_position.y as f64 * 24. - 12.)
						} else {
							DVec2::new(
								to_connector_node_position.x as f64 * 24. + 5. * 24.,
								to_connector_node_position.y as f64 * 24. + output_index as f64 * 24. + 24.,
							)
						};
						self.wire_in_progress_to_connector = Some((to_connector_position, is_layer));
					}
					// Not hovering over a node input or node output, update with the mouse position.
					else {
						self.wire_in_progress_to_connector = Some((point, false));
						// Disconnect if the wire was previously connected to an input
						if let Some(disconnecting) = self.disconnecting {
							responses.add(NodeGraphMessage::DisconnectInput {
								node_id: disconnecting.0,
								input_index: disconnecting.1,
							});
							// Update the front end that the node is disconnected
							responses.add(NodeGraphMessage::SendGraph);
							self.disconnecting = None;
						}
					}

					if let (Some(wire_in_progress_from_connector), Some(wire_in_progress_to_connector)) = (self.wire_in_progress_from_connector, self.wire_in_progress_to_connector) {
						let wire_path = WirePath {
							path_string: Self::build_wire_path_string(
								wire_in_progress_from_connector.0,
								wire_in_progress_to_connector.0,
								wire_in_progress_from_connector.1,
								wire_in_progress_to_connector.1,
							),
							data_type: FrontendGraphDataType::General,
							thick: false,
							dashed: false,
						};
						responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: Some(wire_path) });
					}
				} else if let Some(drag_start) = &mut self.drag_start {
					if self.begin_dragging {
						responses.add(DocumentMessage::StartTransaction);
						self.begin_dragging = false;
					}
					let graph_delta = IVec2::new(((point.x - drag_start.start_x) / 24.).round() as i32, ((point.y - drag_start.start_y) / 24.).round() as i32);
					if drag_start.round_x != graph_delta.x || drag_start.round_y != graph_delta.y {
						responses.add(NodeGraphMessage::MoveSelectedNodes {
							displacement_x: graph_delta.x - drag_start.round_x,
							displacement_y: graph_delta.y - drag_start.round_y,
							move_upstream: ipp.keyboard.get(shift as usize),
						});
						drag_start.round_x = graph_delta.x;
						drag_start.round_y = graph_delta.y;
					}
				} else if let Some(box_selection_start) = self.box_selection_start {
					// The mouse button was released but we missed the pointer up event
					// if ((e.buttons & 1) === 0) {
					// 	completeBoxSelection();
					// 	boxSelection = undefined;
					// } else if ((e.buttons & 2) !== 0) {
					// 	editor.handle.selectNodes(new BigUint64Array(previousSelection));
					// 	boxSelection = undefined;
					// }

					let box_selection = Some(BoxSelection {
						start_x: box_selection_start.x,
						start_y: box_selection_start.y,
						end_x: viewport_location.x.max(0.) as u32,
						end_y: viewport_location.y.max(0.) as u32,
					});

					let graph_start = node_graph_to_viewport.inverse().transform_point2(box_selection_start.into());

					// TODO: Only loop through visible nodes
					let shift = ipp.keyboard.get(shift as usize);
					let mut nodes = if shift { selected_nodes.selected_nodes_ref().clone() } else { Vec::new() };
					for node_id in network.nodes.keys().chain([network.exports_metadata.0, network.imports_metadata.0].iter()) {
						if self
							.node_metadata
							.get(node_id)
							.is_some_and(|node_metadata| node_metadata.node_click_target.intersect_rectangle(Quad::from_box([graph_start, point]), DAffine2::IDENTITY))
						{
							nodes.push(*node_id);
						}
					}
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
					responses.add(FrontendMessage::UpdateBox { box_selection })
				}
			}
			NodeGraphMessage::PointerUp => {
				let Some(network) = document_network.nested_network(&self.network) else {
					warn!("No network");
					return;
				};
				if let Some(node_to_deselect) = self.deselect_on_pointer_up {
					let mut new_selected_nodes = selected_nodes.selected_nodes_ref().clone();
					new_selected_nodes.remove(node_to_deselect);
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: new_selected_nodes });
					self.deselect_on_pointer_up = None;
				}

				// Disconnect if the wire was previously connected to an input
				let viewport_location = ipp.mouse.position;
				let point = node_graph_to_viewport.inverse().transform_point2(viewport_location);

				if let (Some(wire_in_progress_from_connector), Some(wire_in_progress_to_connector)) = (self.wire_in_progress_from_connector, self.wire_in_progress_to_connector) {
					// Check if dragged connector is reconnected to another input
					let node_from = self.get_connector_from_point(wire_in_progress_from_connector.0, |metadata| &metadata.output_click_targets);
					let node_to = self.get_connector_from_point(wire_in_progress_to_connector.0, |metadata| &metadata.input_click_targets);

					if let (Some(node_from), Some(node_to)) = (node_from, node_to) {
						responses.add(NodeGraphMessage::ConnectNodesByWire {
							output_node: node_from.0,
							output_node_connector_index: node_from.1,
							input_node: node_to.0,
							input_node_connector_index: node_to.1,
						})
					} else if node_from.is_some() && node_to.is_none() && !self.initial_disconnecting {
						// If the add node menu is already open, we don't want to open it again
						if self.context_menu.is_some() {
							return;
						}

						let appear_right_of_mouse = if viewport_location.x > ipp.viewport_bounds.size().x - 173. { -173. } else { 0. };
						let appear_above_mouse = if viewport_location.y > ipp.viewport_bounds.size().y - 34. { -34. } else { 0. };
						let node_graph_shift = DVec2::new(appear_right_of_mouse, appear_above_mouse) / node_graph_to_viewport.matrix2.x_axis.x;

						self.context_menu = Some(ContextMenuInformation {
							context_menu_coordinates: ((point.x + node_graph_shift.x) as i32, (point.y + node_graph_shift.y) as i32),
							context_menu_data: ContextMenuData::CreateNode,
						});

						responses.add(FrontendMessage::UpdateContextMenuInformation {
							context_menu_information: self.context_menu.clone(),
						});
						return;
					}
				} else if let Some(drag_start) = &self.drag_start {
					// Only select clicked node if multiple are selected and they were not dragged
					if let Some(select_if_not_dragged) = self.select_if_not_dragged {
						if drag_start.start_x == point.x
							&& drag_start.start_y == point.y
							&& (selected_nodes.selected_nodes_ref().len() != 1
								|| selected_nodes
									.selected_nodes_ref()
									.first()
									.is_some_and(|first_selected_node| *first_selected_node != select_if_not_dragged))
						{
							responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![select_if_not_dragged] })
						}
					}

					// Check if a single node was dragged onto a wire
					if selected_nodes.selected_nodes_ref().len() == 1 {
						let selected_node_id = selected_nodes.selected_nodes_ref()[0];
						// Check that neither the primary input or output of the selected node are already connected.
						let (selected_node_input, selected_node_is_layer) = network
							.nodes
							.get(&selected_node_id)
							.map(|selected_node| (selected_node.inputs.first(), selected_node.is_layer))
							.unwrap_or((network.exports.first(), false));

						// Check if primary input is disconnected
						if selected_node_input.is_some_and(|first_input| first_input.as_value().is_some()) {
							let has_primary_output_connection = network.nodes.iter().flat_map(|(_, node)| node.inputs.iter()).any(|input| {
								if let NodeInput::Node { node_id, output_index, .. } = input {
									*node_id == selected_node_id && *output_index == 0
								} else {
									false
								}
							});
							// Check if primary output is disconnected
							if !has_primary_output_connection {
								// TODO: Cache all wire locations. This will be difficult since there are many ways for an input to changes, and each change will have to update the cache
								let Some(bounding_box) = self
									.node_metadata
									.get(&selected_node_id)
									.and_then(|node_metadata| node_metadata.node_click_target.subpath.bounding_box())
								else {
									log::error!("Could not get bounding box for node: {selected_node_id}");
									return;
								};
								let overlapping_wire = Self::collect_wires(network).into_iter().find(|frontend_wire| {
									let (end_node_position, end_node_is_layer) = network.nodes.get(&frontend_wire.wire_end).map_or(
										(DVec2::new(network.exports_metadata.1.x as f64 * 24., network.exports_metadata.1.y as f64 * 24. + 24.), false),
										|node| (DVec2::new(node.metadata.position.x as f64 * 24., node.metadata.position.y as f64 * 24.), node.is_layer),
									);
									let (start_node_position, start_node_is_layer) = network.nodes.get(&frontend_wire.wire_start).map_or(
										(DVec2::new(network.imports_metadata.1.x as f64 * 24., network.imports_metadata.1.y as f64 * 24. + 24.), false),
										|node| (DVec2::new(node.metadata.position.x as f64 * 24., node.metadata.position.y as f64 * 24.), node.is_layer),
									);

									let input_position = if end_node_is_layer {
										DVec2::new(end_node_position.x + 2. * 24., end_node_position.y + 2. * 24. + 12.)
									} else {
										DVec2::new(end_node_position.x, end_node_position.y + 24. + 24. * frontend_wire.wire_end_input_index as f64)
									};

									let output_position = if start_node_is_layer {
										DVec2::new(start_node_position.x + 2. * 24., start_node_position.y - 12.)
									} else {
										DVec2::new(start_node_position.x + 5. * 24., start_node_position.y + 24. + 24. * frontend_wire.wire_start_output_index as f64)
									};

									let locations = Self::build_wire_path_locations(output_position, input_position, start_node_is_layer, end_node_is_layer);
									let bezier = bezier_rs::Bezier::from_cubic_dvec2(
										(locations[0].x, locations[0].y).into(),
										(locations[1].x, locations[1].y).into(),
										(locations[2].x, locations[2].y).into(),
										(locations[3].x, locations[3].y).into(),
									);

									!bezier.rectangle_intersections(bounding_box[0], bounding_box[1]).is_empty() || bezier.is_contained_within(bounding_box[0], bounding_box[1])
								});
								if let Some(overlapping_wire) = overlapping_wire {
									// Prevent inserting on a link that is connected to the selected node
									if overlapping_wire.wire_end != selected_node_id && overlapping_wire.wire_start != selected_node_id {
										responses.add(NodeGraphMessage::InsertNodeBetween {
											post_node_id: overlapping_wire.wire_end,
											post_node_input_index: overlapping_wire.wire_end_input_index,
											insert_node_output_index: 0,
											insert_node_id: selected_node_id,
											insert_node_input_index: 0,
											pre_node_output_index: overlapping_wire.wire_start_output_index,
											pre_node_id: overlapping_wire.wire_start,
										});
										if !selected_node_is_layer {
											responses.add(NodeGraphMessage::ShiftNode { node_id: selected_node_id });
										}
									}
								}
							}
						}
					}
					self.select_if_not_dragged = None
				}
				self.drag_start = None;
				self.begin_dragging = false;
				self.box_selection_start = None;
				self.wire_in_progress_from_connector = None;
				self.wire_in_progress_to_connector = None;
				responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: None });
				responses.add(FrontendMessage::UpdateBox { box_selection: None })
			}
			NodeGraphMessage::PointerOutsideViewport { shift } => {
				if self.drag_start.is_some() || self.box_selection_start.is_some() {
					let _ = self.auto_panning.shift_viewport(ipp, responses);
				} else {
					// Auto-panning
					let messages = [NodeGraphMessage::PointerOutsideViewport { shift }.into(), NodeGraphMessage::PointerMove { shift }.into()];
					self.auto_panning.stop(&messages, responses);
				}
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
				let network_path = if document_network.nodes.contains_key(&node_id) { Vec::new() } else { self.network.clone() };

				if ModifyInputsContext::set_input(self, document_network, &network_path, node_id, input_index, input, self.network.is_empty()) {
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
				let network_path = if document_network.nodes.contains_key(&node_id) { Vec::new() } else { self.network.clone() };

				let Some(network) = document_network.nested_network(&network_path) else {
					return;
				};
				debug_assert!(network.is_acyclic(), "Not acyclic. Network: {network:#?}");
				let outwards_wires = network.collect_outwards_wires();
				let required_shift = |left: NodeId, right: NodeId, document_network: &NodeNetwork| {
					let Some(network) = document_network.nested_network(&network_path) else {
						return 0;
					};
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

				let mut shift_node = |node_id: NodeId, shift: i32, document_network: &mut NodeNetwork| {
					let Some(network) = document_network.nested_network_mut(&network_path) else {
						return;
					};
					if let Some(node) = network.nodes.get_mut(&node_id) {
						node.metadata.position.x += shift
					}
					self.update_click_target(node_id, document_network, network_path.clone());
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
					let shift = required_shift(input_node, node_id, document_network);
					shift_node(node_id, shift, document_network);
				}

				// Shift nodes connected to the output port of the specified node
				for &descendant in outwards_wires.get(&node_id).unwrap_or(&Vec::new()) {
					let shift = required_shift(node_id, descendant, document_network);
					let mut stack = vec![descendant];
					while let Some(id) = stack.pop() {
						shift_node(id, shift, document_network);
						stack.extend(outwards_wires.get(&id).unwrap_or(&Vec::new()).iter().copied())
					}
				}

				self.send_graph(document_network, document_metadata, collapsed, graph_view_overlay_open, responses);
				responses.add(DocumentMessage::RenderRulers);
				responses.add(DocumentMessage::RenderScrollbars);
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
						responses.add(NodeGraphMessage::SetToNodeOrLayer { node_id, is_layer: !node.is_layer });
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
				self.update_click_target(node_id, document_network, self.network.clone());

				self.context_menu = None;
				responses.add(FrontendMessage::UpdateContextMenuInformation {
					context_menu_information: self.context_menu.clone(),
				});
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
					if let Some(node_metadata) = self.node_metadata.get_mut(&node_id) {
						if node.is_layer {
							node_metadata.layer_width = Some(NodeGraphMessageHandler::layer_width_cells(node));
						} else {
							node_metadata.layer_width = None;
						}
					};
					self.update_click_target(node_id, document_network, self.network.clone());
					responses.add(DocumentMessage::RenderRulers);
					responses.add(DocumentMessage::RenderScrollbars);
					responses.add(NodeGraphMessage::SendGraph);
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
		let mut common = vec![];

		if self
			.context_menu
			.as_ref()
			.is_some_and(|context_menu| matches!(context_menu.context_menu_data, ContextMenuData::CreateNode))
		{
			common.extend(actions!(NodeGraphMessageDiscriminant; CloseCreateNodeMenu));
		}

		common
	}
}

impl NodeGraphMessageHandler {
	/// Similar to [`NodeGraphMessageHandler::actions`], but this provides additional actions if the node graph is open and should only be called in that circumstance.
	pub fn actions_additional_if_node_graph_is_open(&self) -> ActionList {
		let mut common = actions!(NodeGraphMessageDiscriminant; EnterNestedNetwork, PointerDown, PointerMove, PointerUp);

		if self.has_selection {
			common.extend(actions!(NodeGraphMessageDiscriminant;
				Copy,
				Cut,
				DeleteSelectedNodes,
				DuplicateSelectedNodes,
				ToggleSelectedAsLayersOrNodes,
				PrintSelectedNodeCoordinates,
			));
		}

		common
	}

	#[cfg(not(target_arch = "wasm32"))]
	fn get_text_width(node: &DocumentNode) -> Option<f64> {
		warn!("Failed to find width of {node:#?} due to non-wasm arch");
		None
	}

	#[cfg(target_arch = "wasm32")]
	fn get_text_width(node: &DocumentNode) -> Option<f64> {
		let document = web_sys::window().unwrap().document().unwrap();
		let div = match document.create_element("div") {
			Ok(div) => div,
			Err(err) => {
				log::error!("Error creating div: {:?}", err);
				return None;
			}
		};

		// Set the div's style to make it offscreen and single line
		match div.set_attribute("style", "position: absolute; top: -9999px; left: -9999px; white-space: nowrap;") {
			Err(err) => {
				log::error!("Error setting attribute: {:?}", err);
				return None;
			}
			_ => {}
		};

		// From NodeGraphMessageHandler::untitled_layer_label(node)
		let name = (node.alias != "")
			.then_some(node.alias.to_string())
			.unwrap_or(if node.is_layer && node.name == "Merge" { "Untitled Layer".to_string() } else { node.name.clone() });

		div.set_text_content(Some(&name));

		// Append the div to the document body
		match document.body().unwrap().append_child(&div) {
			Err(err) => {
				log::error!("Error setting adding child to document {:?}", err);
				return None;
			}
			_ => {}
		};

		// Measure the width
		let text_width = div.get_bounding_client_rect().width();

		// Remove the div from the document
		match document.body().unwrap().remove_child(&div) {
			Err(_) => log::error!("Could not remove child when rendering text"),
			_ => {}
		};

		Some(text_width)
	}

	pub fn layer_width_cells(node: &DocumentNode) -> u32 {
		let half_grid_cell_offset = 24. / 2.;
		let thumbnail_width = 3. * 24.;
		let gap_width = 8.;
		let text_width = Self::get_text_width(node).unwrap_or_default();
		let icon_width = 24.;
		let icon_overhang_width = icon_width / 2.;

		let text_right = half_grid_cell_offset + thumbnail_width + gap_width + text_width;
		let layer_width_pixels = text_right + gap_width + icon_width - icon_overhang_width;
		((layer_width_pixels / 24.) as u32).max(8)
	}

	// Inserts a node into the network and updates the click target
	pub fn insert_node(&mut self, node_id: NodeId, node: DocumentNode, document_network: &mut NodeNetwork, network_path: &[NodeId]) {
		let Some(network) = document_network.nested_network_mut(network_path) else {
			log::error!("Network not found in update_click_target");
			return;
		};
		assert!(
			node_id != network.imports_metadata.0 && node_id != network.exports_metadata.0,
			"Cannot insert import/export node into network.nodes"
		);
		network.nodes.insert(node_id, node);
		self.update_click_target(node_id, document_network, network_path.to_owned());
	}

	/// Update the click targets when a DocumentNode's click target changes. network_path is the path to the encapsulating network
	pub fn update_click_target(&mut self, node_id: NodeId, document_network: &NodeNetwork, network_path: Vec<NodeId>) {
		let Some(network) = document_network.nested_network(&network_path) else {
			log::error!("Network not found in update_click_target");
			return;
		};

		let grid_size = 24; // Number of pixels per grid unit at 100% zoom

		if let Some(node) = network.nodes.get(&node_id) {
			let mut layer_width = None;
			let width = if node.is_layer {
				let layer_width_cells = self
					.node_metadata
					.get(&node_id)
					.and_then(|node_metadata| node_metadata.layer_width)
					.unwrap_or_else(|| Self::layer_width_cells(node));

				layer_width = Some(layer_width_cells);

				layer_width_cells * grid_size
			} else {
				5 * grid_size
			};
			let height = if node.is_layer {
				2 * grid_size
			} else {
				let inputs_count = node.inputs.iter().filter(|input| input.is_exposed()).count();
				let outputs_count = if let DocumentNodeImplementation::Network(network) = &node.implementation {
					network.exports.len()
				} else {
					1
				};
				std::cmp::max(inputs_count, outputs_count) as u32 * grid_size
			};
			let mut corner1 = DVec2::new((node.metadata.position.x * grid_size as i32) as f64, (node.metadata.position.y * grid_size as i32) as f64);
			let radius = if !node.is_layer {
				corner1 += DVec2::new(0., (grid_size / 2) as f64);
				3.
			} else {
				10.
			};

			let corner2 = corner1 + DVec2::new(width as f64, height as f64);
			let mut click_target_corner_1 = corner1;
			if node.is_layer && node.inputs.iter().filter(|input| input.is_exposed()).count() > 1 {
				click_target_corner_1 -= DVec2::new(24., 0.)
			}

			let subpath = bezier_rs::Subpath::new_rounded_rect(click_target_corner_1, corner2, [radius; 4]);
			let stroke_width = 1.;
			let node_click_target = ClickTarget { subpath, stroke_width };

			// Create input/output click targets
			let mut input_click_targets = Vec::new();
			let mut output_click_targets = Vec::new();
			let mut visibility_click_target = None;

			if !node.is_layer {
				let mut node_top_right: DVec2 = corner1 + DVec2::new(5. * 24., 0.);

				let number_of_inputs = node.inputs.iter().filter(|input| input.is_exposed()).count();
				let number_of_outputs = if let DocumentNodeImplementation::Network(network) = &node.implementation {
					network.exports.len()
				} else {
					1
				};

				if !node.has_primary_output {
					node_top_right.y += 24.;
				}

				let input_top_left = DVec2::new(-8., 4.);
				let input_bottom_right = DVec2::new(8., 20.);

				for node_row_index in 0..number_of_inputs {
					let stroke_width = 1.;
					let subpath = Subpath::new_ellipse(
						input_top_left + corner1 + DVec2::new(0., node_row_index as f64 * 24.),
						input_bottom_right + corner1 + DVec2::new(0., node_row_index as f64 * 24.),
					);
					let input_click_target = ClickTarget { subpath, stroke_width };
					input_click_targets.push(input_click_target);
				}

				for node_row_index in 0..number_of_outputs {
					let stroke_width = 1.;
					let subpath = Subpath::new_ellipse(
						input_top_left + node_top_right + DVec2::new(0., node_row_index as f64 * 24.),
						input_bottom_right + node_top_right + DVec2::new(0., node_row_index as f64 * 24.),
					);
					let output_click_target = ClickTarget { subpath, stroke_width };
					output_click_targets.push(output_click_target);
				}
			} else {
				let input_top_left = DVec2::new(-8., -8.);
				let input_bottom_right = DVec2::new(8., 8.);
				let layer_input_offset = corner1 + DVec2::new(2. * 24., 2. * 24. + 8.);

				let stroke_width = 1.;
				let subpath = Subpath::new_ellipse(input_top_left + layer_input_offset, input_bottom_right + layer_input_offset);
				let layer_input_click_target = ClickTarget { subpath, stroke_width };
				input_click_targets.push(layer_input_click_target);

				if node.inputs.iter().filter(|input| input.is_exposed()).count() > 1 {
					let layer_input_offset = corner1 + DVec2::new(0., 24.);
					let stroke_width = 1.;
					let subpath = Subpath::new_ellipse(input_top_left + layer_input_offset, input_bottom_right + layer_input_offset);
					let input_click_target = ClickTarget { subpath, stroke_width };
					input_click_targets.push(input_click_target);
				}

				// Output
				let layer_output_offset = corner1 + DVec2::new(2. * 24., -8.);
				let stroke_width = 1.;
				let subpath = Subpath::new_ellipse(input_top_left + layer_output_offset, input_bottom_right + layer_output_offset);
				let layer_output_click_target = ClickTarget { subpath, stroke_width };
				output_click_targets.push(layer_output_click_target);

				// Update visibility button click target
				let visibility_offset = corner1 + DVec2::new(width as f64, 24.);
				let subpath = Subpath::new_rounded_rect(DVec2::new(-12., -12.) + visibility_offset, DVec2::new(12., 12.) + visibility_offset, [3.; 4]);
				let stroke_width = 1.;
				let layer_visibility_click_target = ClickTarget { subpath, stroke_width };
				visibility_click_target = Some(layer_visibility_click_target);
			}
			let node_metadata = NodeMetadata {
				node_click_target,
				input_click_targets,
				output_click_targets,
				visibility_click_target,
				layer_width,
			};
			self.node_metadata.insert(node_id, node_metadata);
		} else if node_id == network.exports_metadata.0 {
			let width = 5 * grid_size;
			// 1 is added since the first row is reserved for the "Exports" name
			let height = (network.exports.len() as u32 + 1) * grid_size;

			let corner1 = IVec2::new(network.exports_metadata.1.x * grid_size as i32, network.exports_metadata.1.y * grid_size as i32 + grid_size as i32 / 2);
			let corner2 = corner1 + IVec2::new(width as i32, height as i32);
			let radius = 3.;
			let subpath = bezier_rs::Subpath::new_rounded_rect(corner1.into(), corner2.into(), [radius; 4]);
			let stroke_width = 1.;
			let node_click_target = ClickTarget { subpath, stroke_width };

			let node_top_left = network.exports_metadata.1 * grid_size as i32;
			let mut node_top_left = DVec2::new(node_top_left.x as f64, node_top_left.y as f64);
			// Offset 12px due to nodes being centered, and another 24px since the first export is on the second line
			node_top_left.y += 36.;
			let input_top_left = DVec2::new(-8., 4.);
			let input_bottom_right = DVec2::new(8., 20.);

			// Create input/output click targets
			let mut input_click_targets = Vec::new();
			let output_click_targets = Vec::new();
			let visibility_click_target = None;

			for _ in 0..network.exports.len() {
				let stroke_width = 1.;
				let subpath = Subpath::new_ellipse(input_top_left + node_top_left, input_bottom_right + node_top_left);
				let top_left_input = ClickTarget { subpath, stroke_width };
				input_click_targets.push(top_left_input);

				node_top_left += 24.;
			}

			let node_metadata = NodeMetadata {
				node_click_target,
				input_click_targets,
				output_click_targets,
				visibility_click_target,
				layer_width: None,
			};

			self.node_metadata.insert(node_id, node_metadata);
		}
		// The number of imports is from the parent node, which is passed as a parameter. The number of exports is available from self.
		else if node_id == network.imports_metadata.0 {
			let mut encapsulating_path = self.network.clone();
			// Import count is based on the number of inputs to the encapsulating node. If the current network is the document network, there is no import node
			if let Some(encapsulating_node) = encapsulating_path.pop() {
				let parent_node = document_network
					.nested_network(&encapsulating_path)
					.expect("Encapsulating path should always exist")
					.nodes
					.get(&encapsulating_node)
					.expect("Last path node should always exist in encapsulating network");
				let import_count = parent_node.inputs.len();

				let width = 5 * grid_size;
				// 1 is added since the first row is reserved for the "Exports" name
				let height = (import_count + 1) as u32 * grid_size;

				let corner1 = IVec2::new(network.imports_metadata.1.x * grid_size as i32, network.imports_metadata.1.y * grid_size as i32 + grid_size as i32 / 2);
				let corner2 = corner1 + IVec2::new(width as i32, height as i32);
				let radius = 3.;
				let subpath = bezier_rs::Subpath::new_rounded_rect(corner1.into(), corner2.into(), [radius; 4]);
				let stroke_width = 1.;
				let node_click_target = ClickTarget { subpath, stroke_width };

				let node_top_right = network.imports_metadata.1 * grid_size as i32;
				let mut node_top_right = DVec2::new(node_top_right.x as f64 + width as f64, node_top_right.y as f64);
				// Offset 12px due to nodes being centered, and another 24px since the first import is on the second line
				node_top_right.y += 36.;
				let input_top_left = DVec2::new(-8., 4.);
				let input_bottom_right = DVec2::new(8., 20.);

				// Create input/output click targets
				let input_click_targets = Vec::new();
				let mut output_click_targets = Vec::new();
				let visibility_click_target = None;
				for _ in 0..import_count {
					let stroke_width = 1.;
					let subpath = Subpath::new_ellipse(input_top_left + node_top_right, input_bottom_right + node_top_right);
					let top_left_input = ClickTarget { subpath, stroke_width };
					output_click_targets.push(top_left_input);

					node_top_right.y += 24.;
				}
				let node_metadata = NodeMetadata {
					node_click_target,
					input_click_targets,
					output_click_targets,
					visibility_click_target,
					layer_width: None,
				};
				self.node_metadata.insert(node_id, node_metadata);
			}
		} else {
			self.node_metadata.remove(&node_id);
		}
		let bounds = self
			.node_metadata
			.iter()
			.filter_map(|(_, node_metadata)| node_metadata.node_click_target.subpath.bounding_box())
			.reduce(Quad::combine_bounds);
		self.bounding_box_subpath = bounds.map(|bounds| bezier_rs::Subpath::new_rect(bounds[0], bounds[1]));
	}

	// Updates all click targets in a certain network
	pub fn update_all_click_targets(&mut self, document_network: &NodeNetwork, network_path: Vec<NodeId>) {
		let Some(network) = document_network.nested_network(&network_path) else {
			log::error!("Network not found in update_all_click_targets");
			return;
		};
		let export_id = network.exports_metadata.0;
		let import_id = network.imports_metadata.0;
		for (node_id, _) in network.nodes.iter() {
			self.update_click_target(*node_id, document_network, network_path.clone());
		}
		self.update_click_target(export_id, document_network, network_path.clone());
		self.update_click_target(import_id, document_network, network_path.clone())
	}

	/// Gets the bounding box in viewport coordinates for each node in the node graph
	pub fn graph_bounds_viewport_space(&self, node_graph_to_viewport: DAffine2) -> Option<[DVec2; 2]> {
		self.bounding_box_subpath
			.as_ref()
			.and_then(|bounding_box| bounding_box.bounding_box_with_transform(node_graph_to_viewport))
	}

	fn get_node_from_point(&self, point: DVec2) -> Option<NodeId> {
		self.node_metadata
			.iter()
			.map(|(node_id, node_metadata)| (node_id, &node_metadata.node_click_target))
			.find_map(|(node_id, click_target)| if click_target.intersect_point(point, DAffine2::IDENTITY) { Some(*node_id) } else { None })
	}

	fn get_connector_from_point<F>(&self, point: DVec2, click_target_selector: F) -> Option<(NodeId, usize)>
	where
		F: Fn(&NodeMetadata) -> &Vec<ClickTarget>,
	{
		self.node_metadata
			.iter()
			.map(|(node_id, node_metadata)| (node_id, click_target_selector(node_metadata)))
			.find_map(|(node_id, click_targets)| {
				for (index, click_target) in click_targets.iter().enumerate() {
					if click_target.intersect_point(point, DAffine2::IDENTITY) {
						return Some((*node_id, index));
					}
				}
				None
			})
	}

	fn get_visibility_from_point(&self, point: DVec2) -> Option<NodeId> {
		self.node_metadata
			.iter()
			.filter_map(|(node_id, node_metadata)| node_metadata.visibility_click_target.as_ref().map(|click_target| (node_id, click_target)))
			.find_map(|(node_id, click_target)| if click_target.intersect_point(point, DAffine2::IDENTITY) { Some(*node_id) } else { None })
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

			let subgraph_path_names = Self::collect_subgraph_names(&mut self.network, document_network);
			let breadcrumb_trail = subgraph_path_names.and_then(|subgraph_path_names| {
				let subgraph_path_names_length = subgraph_path_names.len();
				if subgraph_path_names_length < 2 {
					return None;
				}

				Some(BreadcrumbTrailButtons::new(subgraph_path_names).on_update(move |index| {
					NodeGraphMessage::ExitNestedNetwork {
						steps_back: subgraph_path_names_length - (*index as usize) - 1,
					}
					.into()
				}))
			});
			let mut widgets = breadcrumb_trail
				.map(|breadcrumb_trail| vec![breadcrumb_trail.widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()])
				.unwrap_or_default();

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
			.nested_network_for_selected_nodes(context.nested_path, selected_nodes.selected_nodes(context.document_network))
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

			let output_types = Self::get_output_types(node, &self.resolved_types, node_id_path);
			let primary_output_type = output_types.first().expect("Primary output should always exist");
			let frontend_data_type = if let Some(output_type) = primary_output_type {
				FrontendGraphDataType::with_type(output_type)
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
			let is_export = network.exports.first().is_some_and(|export| export.as_node().is_some_and(|export_node_id| node_id == export_node_id));
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
				errors,
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
				let output_types = Self::get_output_types(node, &self.resolved_types, node_id_path);

				if let Some(output_type) = output_types.get(*output_index).cloned().flatten() {
					(FrontendGraphDataType::with_type(&output_type), Some(output_type.clone()))
				} else {
					(FrontendGraphDataType::General, None)
				}
			} else if let NodeInput::Value { tagged_value, .. } = export {
				(FrontendGraphDataType::with_type(&tagged_value.ty()), Some(tagged_value.ty()))
			// TODO: Get type from parent node input when <https://github.com/GraphiteEditor/Graphite/issues/1762> is possible
			// else if let NodeInput::Network { import_type, .. } = export {
			// 	(FrontendGraphDataType::with_type(import_type), Some(import_type.clone()))
			// }
			} else {
				(FrontendGraphDataType::General, None)
			};

			// First import index is visually connected to the root node instead of its actual export input so previewing does not change the connection
			let connected = if index == 0 {
				network.get_root_node().map(|root_node| root_node.id)
			} else if let NodeInput::Node { node_id, .. } = export {
				Some(*node_id)
			} else {
				None
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
		let mut subgraph_path_names = vec!["Document".to_string()];
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
			subgraph_path_names.push(node.name.clone());
		}
		Some(subgraph_path_names)
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
			let layer_widths = self
				.node_metadata
				.iter()
				.filter_map(|(node_id, node_metadata)| node_metadata.layer_width.map(|layer_width| (*node_id, layer_width)))
				.collect::<HashMap<NodeId, u32>>();
			responses.add(FrontendMessage::UpdateLayerWidths { layer_widths });
		}
	}

	pub fn get_output_types(node: &DocumentNode, resolved_types: &ResolvedDocumentNodeTypes, node_id_path: &[NodeId]) -> Vec<Option<Type>> {
		let mut output_types = Vec::new();

		let primary_output_type = resolved_types
			.outputs
			.get(&Source {
				node: node_id_path.to_owned(),
				index: 0,
			})
			.cloned();
		output_types.push(primary_output_type);

		// If the node is not a protonode, get types by traversing across exports until a proto node is reached.
		if let graph_craft::document::DocumentNodeImplementation::Network(internal_network) = &node.implementation {
			for export in internal_network.exports.iter().skip(1) {
				let mut current_export = export;
				let mut current_network = internal_network;
				let mut current_path = node_id_path.to_owned();

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
				} else if let NodeInput::Network { import_index, .. } = current_export {
					resolved_types
						.outputs
						.get(&Source {
							node: node_id_path.to_owned(),
							index: *import_index,
						})
						.cloned()
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
		network_path: &'a [NodeId],
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

	pub fn get_default_inputs(document_network: &NodeNetwork, network_path: &[NodeId], node_id: NodeId, resolved_types: &ResolvedDocumentNodeTypes, node: &DocumentNode) -> Vec<NodeInput> {
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
		if !node.alias.is_empty() {
			node.alias.to_string()
		} else if node.is_layer && node.name == "Merge" {
			"Untitled Layer".to_string()
		} else {
			node.name.clone()
		}
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

	fn build_wire_path_string(output_position: DVec2, input_position: DVec2, vertical_out: bool, vertical_in: bool) -> String {
		let locations = Self::build_wire_path_locations(output_position, input_position, vertical_out, vertical_in);
		let smoothing = 0.5;
		let delta01 = DVec2::new((locations[1].x - locations[0].x) * smoothing, (locations[1].y - locations[0].y) * smoothing);
		let delta23 = DVec2::new((locations[3].x - locations[2].x) * smoothing, (locations[3].y - locations[2].y) * smoothing);
		format!(
			"M{},{} L{},{} C{},{} {},{} {},{} L{},{}",
			locations[0].x,
			locations[0].y,
			locations[1].x,
			locations[1].y,
			locations[1].x + delta01.x,
			locations[1].y + delta01.y,
			locations[2].x - delta23.x,
			locations[2].y - delta23.y,
			locations[2].x,
			locations[2].y,
			locations[3].x,
			locations[3].y
		)
	}

	fn build_wire_path_locations(output_position: DVec2, input_position: DVec2, vertical_out: bool, vertical_in: bool) -> Vec<DVec2> {
		let horizontal_gap = (output_position.x - input_position.x).abs();
		let vertical_gap = (output_position.y - input_position.y).abs();
		// TODO: Finish this commented out code replacement for the code below it based on this diagram: <https://files.keavon.com/-/InsubstantialElegantQueenant/capture.png>
		// // Straight: stacking lines which are always straight, or a straight horizontal wire between two aligned nodes
		// if ((verticalOut && vertical_in) || (!verticalOut && !vertical_in && vertical_gap === 0)) {
		// 	return [
		// 		{ x: output_position.x, y: output_position.y },
		// 		{ x: input_position.x, y: input_position.y },
		// 	];
		// }

		// // L-shape bend
		// if (verticalOut !== vertical_in) {
		// }

		let curve_length = 24.;
		let curve_falloff_rate = curve_length * std::f64::consts::PI * 2.0;

		let horizontal_curve_amount = -(2.0f64.powf((-10. * horizontal_gap) / curve_falloff_rate)) + 1.;
		let vertical_curve_amount = -(2.0f64.powf((-10. * vertical_gap) / curve_falloff_rate)) + 1.;
		let horizontal_curve = horizontal_curve_amount * curve_length;
		let vertical_curve = vertical_curve_amount * curve_length;

		vec![
			output_position,
			DVec2::new(
				if vertical_out { output_position.x } else { output_position.x + horizontal_curve },
				if vertical_out { output_position.y - vertical_curve } else { output_position.y },
			),
			DVec2::new(
				if vertical_in { input_position.x } else { input_position.x - horizontal_curve },
				if vertical_in { input_position.y + vertical_curve } else { input_position.y },
			),
			DVec2::new(input_position.x, input_position.y),
		]
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
			begin_dragging: false,
			box_selection_start: None,
			disconnecting: None,
			initial_disconnecting: false,
			select_if_not_dragged: None,
			wire_in_progress_from_connector: None,
			wire_in_progress_to_connector: None,
			context_menu: None,
			node_metadata: HashMap::new(),
			bounding_box_subpath: None,
			deselect_on_pointer_up: None,
			auto_panning: Default::default(),
		}
	}
}

impl PartialEq for NodeGraphMessageHandler {
	fn eq(&self, other: &Self) -> bool {
		self.network == other.network
			&& self.resolved_types == other.resolved_types
			&& self.node_graph_errors == other.node_graph_errors
			&& self.has_selection == other.has_selection
			&& self.widgets == other.widgets
			&& self.drag_start == other.drag_start
			&& self.begin_dragging == other.begin_dragging
			&& self.box_selection_start == other.box_selection_start
			&& self.disconnecting == other.disconnecting
			&& self.initial_disconnecting == other.initial_disconnecting
			&& self.select_if_not_dragged == other.select_if_not_dragged
			&& self.wire_in_progress_from_connector == other.wire_in_progress_from_connector
			&& self.wire_in_progress_to_connector == other.wire_in_progress_to_connector
			&& self.context_menu == other.context_menu
	}
}
