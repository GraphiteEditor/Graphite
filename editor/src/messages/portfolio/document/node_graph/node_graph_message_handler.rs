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
use crate::messages::portfolio::document::utility_types::network_interface::{self, Connector, InputConnector, NodeNetworkInterface, NodeNetworkMetadata, OutputConnector, Port};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, LayerPanelEntry, SelectedNodes};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;

use bezier_rs::Subpath;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, FlowType, NodeId, NodeInput, NodeNetwork, Previewing, Source};
use graph_craft::proto::GraphErrors;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::*;
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypes;

use glam::{DAffine2, DVec2, IVec2, UVec2};
use renderer::{ClickTarget, Quad};
use specta::reference;
use usvg::filter::Input;
use usvg::Node;
use web_sys::window;

#[derive(Debug)]
pub struct NodeGraphHandlerData<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub document_metadata: &'a mut DocumentMetadata,
	pub selected_nodes: &'a mut SelectedNodes,
	pub document_id: DocumentId,
	pub collapsed: &'a mut CollapsedLayers,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub graph_view_overlay_open: bool,
}

#[derive(Debug, Clone)]
pub struct NodeGraphMessageHandler {
	//TODO: Remove network and move to NodeNetworkInterface
	pub network: Vec<NodeId>,
	pub node_graph_errors: GraphErrors,
	has_selection: bool,
	widgets: [LayoutGroup; 2],
	drag_start: Option<DragStart>,
	/// Used to add a transaction for the first node move when dragging.
	begin_dragging: bool,
	/// Stored in pixel coordinates.
	box_selection_start: Option<UVec2>,
	disconnecting: Option<InputConnector>,
	initial_disconnecting: bool,
	/// Node to select on pointer up if multiple nodes are selected and they were not dragged.
	select_if_not_dragged: Option<NodeId>,
	/// The start of the dragged line that cannot be moved
	wire_in_progress_from_connector: Option<DVec2>,
	/// The end point of the dragged line that can be moved
	wire_in_progress_to_connector: Option<DVec2>,
	/// State for the context menu popups.
	context_menu: Option<ContextMenuInformation>,
	auto_panning: AutoPanning,
}

/// NodeGraphMessageHandler always modifies the network which the selected nodes are in. No GraphOperationMessages should be added here, since those messages will always affect the document network.
impl<'a> MessageHandler<NodeGraphMessage, NodeGraphHandlerData<'a>> for NodeGraphMessageHandler {
	fn process_message(&mut self, message: NodeGraphMessage, responses: &mut VecDeque<Message>, data: NodeGraphHandlerData<'a>) {
		let NodeGraphHandlerData {
			network_interface,
			document_metadata,
			selected_nodes,
			document_id,
			collapsed,
			graph_view_overlay_open,
			ipp,
		} = data;

		match message {
			// TODO: automatically remove broadcast messages.
			NodeGraphMessage::Init => {
				responses.add(BroadcastMessage::SubscribeEvent {
					on: BroadcastEvent::SelectionChanged,
					send: Box::new(NodeGraphMessage::SelectedNodesUpdated.into()),
				});
				load_network_structure(network_interface.document_network(), document_metadata, collapsed);
			}
			NodeGraphMessage::SelectedNodesUpdated => {
				self.update_selected(network_interface, selected_nodes, responses);
				if selected_nodes.selected_layers(document_metadata).count() <= 1 {
					responses.add(DocumentMessage::SetRangeSelectionLayer {
						new_layer: selected_nodes.selected_layers(document_metadata).next(),
					});
				}
				responses.add(ArtboardToolMessage::UpdateSelectedArtboard);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::CreateWire {
				output_connector,
				input_connector,
				use_document_network,
			} => {
				network_interface.create_wire(output_connector, input_connector, use_document_network);
			}
			NodeGraphMessage::Copy => {
				// If the selected nodes are in the document network, copy from the document network. Otherwise, use currently opened network
				let use_document_network = network_interface.selected_nodes_in_document_network(selected_nodes.selected_nodes_ref().iter());

				// Collect the selected nodes
				let new_ids = &selected_nodes.selected_nodes().copied().enumerate().map(|(new, old)| (old, NodeId(new as u64))).collect();
				let copied_nodes = network_interface.copy_nodes(new_ids, use_document_network).collect::<Vec<_>>();

				// Prefix to show that these are nodes
				let mut copy_text = String::from("graphite/nodes: ");
				copy_text += &serde_json::to_string(&copied_nodes).expect("Could not serialize copy");

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
			NodeGraphMessage::CreateNode { node_id, node_type, input_override, use_document_network } => {
				let node_id = node_id.unwrap_or_else(|| NodeId(generate_uuid()));

				let Some(document_node_type) = document_node_types::resolve_document_node_type(&node_type) else {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Cannot insert node".to_string(),
						description: format!("The document node '{node_type}' does not exist in the document node list"),
					});
					return;
				};

				let node_template = document_node_type.node_template_input_override(input_override);
				self.context_menu = None;

				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::InsertNode { node_id, node_template, use_document_network });

				// Only auto connect to the dragged wire if the node is being added to the currently opened network
				if !use_document_network {
					if let Some(output_connector_position) = self.wire_in_progress_from_connector {
						let Some(output_connector) = network_interface.get_output_connector_from_click(output_connector_position) else {
							log::error!("Could not get output from connector start");
							return;
						};
	
						// Ensure connection is to correct input of new node. If it does not have an input then do not connect
						if let Some((input_index, _)) = node_template
							.document_node
							.inputs
							.iter()
							.enumerate()
							.find(|(input_index, input)| input.is_exposed_to_frontend(network_interface.is_document_network()))
						{
							responses.add(NodeGraphMessage::CreateWire {
								output_connector,
								input_connector: InputConnector::node(node_id, input_index),
								use_document_network: false,
							});
							if let OutputConnector::Node(node_id, _) = output_connector {
								if network_interface.connected_to_output(&node_id) {
									responses.add(NodeGraphMessage::RunDocumentGraph);
								}
							} else {
								// Creating wire to export node, always run graph
								responses.add(NodeGraphMessage::RunDocumentGraph);
							}
							responses.add(NodeGraphMessage::SendGraph);
						}
	
						self.wire_in_progress_from_connector = None;
						self.wire_in_progress_to_connector = None;
					}
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
			NodeGraphMessage::DeleteNodes {
				node_ids,
				reconnect,
				use_document_network,
			} => {
				network_interface.delete_nodes(node_ids, reconnect, selected_nodes, responses, use_document_network);
			}
			// Deletes selected_nodes. If `reconnect` is true, then all children nodes (secondary input) of the selected nodes are deleted and the siblings (primary input/output) are reconnected.
			// If `reconnect` is false, then only the selected nodes are deleted and not reconnected.
			NodeGraphMessage::DeleteSelectedNodes { reconnect } => {
				let is_document_network = network_interface.selected_nodes_in_document_network(selected_nodes.selected_nodes_ref().iter());
				let Some(network) = network_interface.network(is_document_network) else {
					warn!("No network");
					return;
				};

				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::DeleteNodes {
					node_ids: selected_nodes.selected_nodes().copied().collect(),
					reconnect,
					use_document_network: is_document_network,
				});
			}
			NodeGraphMessage::DisconnectInput {
				input_connector: input,
				use_document_network,
			} => {
				let Some(network) = network_interface.network(use_document_network) else {
					return;
				};

				let existing_input = match input {
					InputConnector::Node { node_id, input_index } => network.nodes.get(&node_id).and_then(|node| node.inputs.get(input_index)),
					InputConnector::Export(input_index) => network.exports.get(input_index),
				};

				let Some(existing_input) = existing_input else {
					warn!("Could not find input for {node_id} at index {input_index} when disconnecting");
					return;
				};

				let tagged_value = TaggedValue::from_type(&network_interface.get_input_type(node_id, input.input_index(), use_document_network));

				let mut value_input = NodeInput::value(tagged_value, true);
				if let NodeInput::Value { exposed, .. } = &mut value_input {
					*exposed = existing_input.is_exposed();
				}
				if let InputConnector::Node { node_id, .. } = input {
					responses.add(NodeGraphMessage::SetNodeInput {
						input_connector: input,
						input: value_input,
						use_document_network,
					});
					if network_interface.connected_to_output(&node_id) {
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				} else {
					// Since it is only possible to drag the solid line, if previewing then there must be a dashed connection, which becomes the new export
					if matches!(network_interface.previewing(use_document_network), Previewing::Yes { .. }) {
						network_interface.start_previewing_without_restore();
					}
					// If there is no preview, then disconnect
					else {
						responses.add(NodeGraphMessage::SetNodeInput {
							input_connector: input,
							input: value_input,
							use_document_network,
						});
					}
				}

				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::DuplicateSelectedNodes => {
				let use_document_network = network_interface.selected_nodes_in_document_network(selected_nodes.selected_nodes_ref().iter());
				// If the selected nodes are in the document network, use the document network. Otherwise, use the nested network
				let Some(network) = network_interface.network(use_document_network) else {
					warn!("No network in NodeGraphMessage::Copy ");
					return;
				};
				responses.add(DocumentMessage::StartTransaction);

				let new_ids = &selected_nodes.selected_nodes().map(|&id| (id, NodeId(generate_uuid()))).collect();

				// Copy the selected nodes
				let copied_nodes = network_interface.copy_nodes(new_ids, use_document_network).collect::<Vec<_>>();

				// Select the new nodes. Duplicated nodes are always pasted into the current network
				responses.add(NodeGraphMessage::SelectedNodesSet {
					nodes: copied_nodes.iter().map(|(node_id, _)| *node_id).collect(),
				});
				responses.add(BroadcastEvent::SelectionChanged);

				for (node_id, node_template) in copied_nodes {
					// Shift duplicated node
					// document_node.metadata.position += IVec2::splat(2);

					// Insert new node into graph
					responses.add(NodeGraphMessage::InsertNode { node_id, node_template, false });
				}

				self.update_selected(network_interface, selected_nodes, responses);
			}
			NodeGraphMessage::EnforceLayerHasNoMultiParams { node_id } => {
				if !network_interface.is_eligible_to_be_layer(&node_id) {
					responses.add(NodeGraphMessage::SetToNodeOrLayer { node_id: node_id, is_layer: false })
				}
			}
			NodeGraphMessage::EnterNestedNetwork => {
				let Some(node_id) = network_interface.get_node_from_click(ipp.mouse.position) else {
					return;
				};
				if network_interface.get_visibility_from_click(ipp.mouse.position).is_some() {
					return;
				};
				let Some(network) = network_interface.network(false) else {
					log::error!("Could not get network in EnterNestedNetwork");
					return;
				};
				if network.imports_metadata.0 == node_id || network.exports_metadata.0 == node_id {
					return;
				};

				let Some(node) = network.nodes.get(&node_id) else { return };
				if let DocumentNodeImplementation::Network(_) = node.implementation {
					network_interface.enter_nested_network(node_id);
					responses.add(DocumentMessage::ZoomCanvasToFitAll);
				}

				responses.add(NodeGraphMessage::SendGraph);

				self.update_selected(document_network, selected_nodes, responses);
			}
			NodeGraphMessage::ExitNestedNetwork { steps_back } => {
				selected_nodes.clear_selected_nodes();
				responses.add(BroadcastEvent::SelectionChanged);

				for _ in 0..steps_back {
					network_interface.exit_nested_network();
				}
				responses.add(NodeGraphMessage::SendGraph);
				self.update_selected(document_network, selected_nodes, responses);
			}
			NodeGraphMessage::ExposeInput { node_id, input_index, new_exposed } => {
				let use_document_network = network_interface.selected_nodes_in_document_network(std::iter::once(&node_id));
				let Some(network) = network_interface.network(use_document_network) else {
					return;
				};

				let Some(node) = network.nodes.get(&node_id) else {
					log::error!("Could not find node {node_id} in NodeGraphMessage::ExposeInput");
					return;
				};

				responses.add(DocumentMessage::StartTransaction);

				let Some(mut input) = node.inputs.get(input_index).cloned() else {
					log::error!("Could not find input {input_index} in NodeGraphMessage::ExposeInput");
					return;
				};
				if let NodeInput::Value { exposed, .. } = &mut input {
					*exposed = new_exposed;
				} else {
					// TODO: Should network and node inputs be able to be hidden?
					log::error!("Could not hide/show input: {:?} since it is not NodeInput::Value", input);
					return;
				}

				responses.add(NodeGraphMessage::SetNodeInput {
					input_connector: InputConnector::node(node_id, input_index),
					input,
					use_document_network,
				});
				responses.add(NodeGraphMessage::EnforceLayerHasNoMultiParams { node_id });
				responses.add(PropertiesPanelMessage::Refresh);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::InsertNode { node_id, node_template, use_document_network } => {
				network_interface.insert_node(node_id, node_template, use_document_network);
			}
			NodeGraphMessage::InsertNodeBetween {
				post_node_id,
				post_node_input_index,
				insert_node_output_index,
				insert_node_id,
				insert_node_input_index,
				pre_node_output_index,
				pre_node_id,
				use_document_network,
			} => {
				// let post_node = document_network.nodes.get(&post_node_id);
				// let Some((post_node_input_index, _)) = post_node
				// 	.map_or(&document_network.exports, |post_node| &post_node.inputs)
				// 	.iter()
				// 	.enumerate()
				// 	.filter(|input| input.1.is_exposed())
				// 	.nth(post_node_input_index)
				// else {
				// 	error!("Failed to find input index {post_node_input_index} on node {post_node_id:#?}");
				// 	return;
				// };
				// let Some(insert_node) = document_network.nodes.get(&insert_node_id) else {
				// 	error!("Insert node not found");
				// 	return;
				// };
				// let Some((insert_node_input_index, _)) = insert_node.inputs.iter().enumerate().filter(|input| input.1.is_exposed()).nth(insert_node_input_index) else {
				// 	error!("Failed to find input index {insert_node_input_index} on node {insert_node_id:#?}");
				// 	return;
				// };

				// let post_input = NodeInput::node(insert_node_id, insert_node_output_index);
				// responses.add(GraphOperationMessage::SetNodeInput {
				// 	node_id: post_node_id,
				// 	input_index: post_node_input_index,
				// 	input: post_input,
				// });

				// let insert_input = NodeInput::node(pre_node_id, pre_node_output_index);
				// responses.add(GraphOperationMessage::SetNodeInput {
				// 	node_id: insert_node_id,
				// 	input_index: insert_node_input_index,
				// 	input: insert_input,
				// });
			}
			NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index } => {
				network_interface.move_layer_to_stack(layer, parent, insert_index);
			}
			NodeGraphMessage::PasteNodes { serialized_nodes } => {
				let data = match serde_json::from_str::<Vec<(NodeId, NodeTemplate)>>(&serialized_nodes) {
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
				// let mut shift = IVec2::ZERO;
				// while data
				// 	.iter()
				// 	.all(|(_, node)| network.nodes.values().any(|existing_node| node.metadata.position + shift == existing_node.metadata.position))
				// {
				// 	shift += IVec2::splat(2);
				// }

				responses.add(DocumentMessage::StartTransaction);

				let new_ids: HashMap<_, _> = data.iter().map(|&(id, _)| (id, NodeId(generate_uuid()))).collect();
				for (old_id, node_template) in data {
					// Shift copied node
					// document_node.metadata.position += shift;

					// Get the new, non-conflicting id
					let node_id = *new_ids.get(&old_id).unwrap();
					// When pasting the default inputs are never used, so use_document_network does not matter
					node_template = network_interface.map_ids(node_template, &new_ids, false);

					// Insert node into network
					responses.add(NodeGraphMessage::InsertNode { node_id, node_template });
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
				let Some(network_metadata) = network_interface.network_metadata(false) else {
					log::error!("Could not get network metadata in PointerDown");
					return;
				};

				let click = ipp.mouse.position;

				let node_graph_point = network_metadata
					.persistent_metadata
					.navigation_metadata
					.node_graph_to_viewport
					.inverse()
					.transform_point2(viewport_location);

				// Toggle visibility of clicked node and return
				if let Some(clicked_visibility) = network_interface.get_visibility_from_click(click) {
					responses.add(NodeGraphMessage::ToggleVisibility { node_id: clicked_visibility });
					return;
				}

				let clicked_id = network_interface.get_node_from_click(click);
				let clicked_input = network_interface.get_input_connector_from_click(click);
				let clicked_output = network_interface.get_output_connector_from_click(click);

				// Create the add node popup on right click, then exit
				if right_click {
					let context_menu_data = if let Some(node_id) = clicked_id {
						ContextMenuData::ToggleLayer {
							node_id: node_id,
							currently_is_node: !network_interface.is_layer(&node_id),
						}
					} else {
						ContextMenuData::CreateNode
					};

					// TODO: Create function
					let node_graph_shift = if matches!(context_menu_data, ContextMenuData::CreateNode) {
						let appear_right_of_mouse = if click.x > ipp.viewport_bounds.size().x - 180. { -180. } else { 0. };
						let appear_above_mouse = if click.y > ipp.viewport_bounds.size().y - 200. { -200. } else { 0. };
						DVec2::new(appear_right_of_mouse, appear_above_mouse) / network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.matrix2.x_axis.x
					} else {
						let appear_right_of_mouse = if click.x > ipp.viewport_bounds.size().x - 173. { -173. } else { 0. };
						let appear_above_mouse = if click.y > ipp.viewport_bounds.size().y - 34. { -34. } else { 0. };
						DVec2::new(appear_right_of_mouse, appear_above_mouse) / network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.matrix2.x_axis.x
					};

					let context_menu_coordinates = ((node_graph_point.x + node_graph_shift.x) as i32, (node_graph_point.y + node_graph_shift.y) as i32);

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
					let context_menu_viewport = network_metadata
						.persistent_metadata
						.navigation_metadata
						.node_graph_to_viewport
						.transform_point2(DVec2::new(context_menu.context_menu_coordinates.0 as f64, context_menu.context_menu_coordinates.1 as f64));
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
					if context_menu_click_target.intersect_point(click, DAffine2::IDENTITY) {
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

				// Begin moving an existing wire
				if let Some(clicked_input) = clicked_input {
					self.initial_disconnecting = true;
					self.disconnecting = Some(clicked_input);

					let Some(output_connector) = network_interface.get_output_connector_from_input_connector(clicked_input) else {
						log::error!("Could not upstream find node {node_id} when moving existing wire");
						return;
					};
					self.wire_in_progress_from_connector = Some(network_interface.get_output_position(output_connector));
					return;
				}

				// Begin creating a new wire
				if let Some(clicked_output) = clicked_output {
					self.initial_disconnecting = false;
					// Disallow creating additional vertical output wires from an already-connected layer
					if let OutputConnector::Node { node_id, .. } = clicked_output {
						if network_interface.is_layer(&node_id) && network_interface.collect_outward_wires(false).get(&clicked_output).is_some() {
							return;
						}
					}
					self.wire_in_progress_from_connector = Some(network_interface.get_output_position(output_connector));
					return;
				}

				if let Some(clicked_id) = clicked_id {
					let mut updated_selected = selected_nodes.selected_nodes().cloned().collect::<Vec<_>>();
					let mut modified_selected = false;

					// Add to/remove from selection if holding Shift or Ctrl
					if shift_click || control_click {
						modified_selected = true;

						let index = updated_selected.iter().enumerate().find_map(|(i, node_id)| if *node_id == clicked_id { Some(i) } else { None });
						// Remove from selection if already selected
						if let Some(index) = index {
							updated_selected.remove(index);
						}
						// Add to selection if not already selected. Necessary in order to drag multiple nodes
						else {
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
				self.box_selection_start = Some(UVec2::new(node_graph_point.x.round().abs() as u32, node_graph_point.y.round().abs() as u32));
			}
			// TODO: Alt+drag should move all upstream nodes as well
			NodeGraphMessage::PointerMove { shift } => {
				let Some(network) = document_network.nested_network(&self.network) else {
					return;
				};

				// Auto-panning
				let messages = [NodeGraphMessage::PointerOutsideViewport { shift }.into(), NodeGraphMessage::PointerMove { shift }.into()];
				self.auto_panning.setup_by_mouse_position(ipp, &messages, responses);

				let viewport_location = ipp.mouse.position;
				let point = network_interface.navigation_metadata().node_graph_to_viewport.inverse().transform_point2(viewport_location);

				if self.wire_in_progress_from_connector.is_some() && self.context_menu.is_none() {
					if let Some(to_connector) = network_interface.get_input_connector_from_click(ipp.mouse.position) {
						let input_position = network_interface.get_input_position(input_connector);
						if input_position.is_none() {
							log::error!("Could not get input position for connector: {to_connector}");
						}
						self.wire_in_progress_to_connector = input_position;
					}
					// Not hovering over a node input or node output, update with the mouse position.
					else {
						self.wire_in_progress_to_connector = Some(point);
						// Disconnect if the wire was previously connected to an input
						if let Some(disconnecting) = self.disconnecting {
							responses.add(DocumentMessage::StartTransaction);
							responses.add(NodeGraphMessage::DisconnectInput {
								input_connector: disconnecting,
								use_document_network: false,
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
						responses.add(NodeGraphMessage::ShiftNodes {
							node_ids: selected_nodes.selected_nodes().cloned().collect(),
							displacement_x: graph_delta.x - drag_start.round_x,
							displacement_y: graph_delta.y - drag_start.round_y,
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
						end_x: ipp.mouse.position.x.max(0.) as u32,
						end_y: ipp.mouse.position.y.max(0.) as u32,
					});

					let Some(network_metadata) = network_interface.network_metadata(false) else {
						log::error!("Could not get network metadata in PointerMove");
						return;
					};

					let box_selection_start_graph = network_metadata
						.persistent_metadata
						.navigation_metadata
						.node_graph_to_viewport
						.inverse()
						.transform_point2(box_selection_start.into());
					let box_selection_end_graph = network_metadata
						.persistent_metadata
						.navigation_metadata
						.node_graph_to_viewport
						.inverse()
						.transform_point2(ipp.mouse.position);

					let shift = ipp.keyboard.get(shift as usize);
					let mut nodes = if shift { selected_nodes.selected_nodes_ref().clone() } else { Vec::new() };
					for click_target in network_metadata.persistent_metadata.node_metadata.iter().filter_map(|(node_id, _)| {
						let Some(transient_metadata) = network_interface.get_transient_node_metadata(node_id, false) else {
							log::error!("Could not get transient metadata for node {node_id}");
							return None;
						};
						Some(transient_metadata.node_click_target)
					}) {
						if node_click_target.intersect_rectangle(Quad::from_box([box_selection_start_graph, box_selection_end_graph]), DAffine2::IDENTITY) {
							nodes.push(*node_id);
						}
					}
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
					responses.add(FrontendMessage::UpdateBox { box_selection })
				}
			}
			NodeGraphMessage::PointerUp => {
				let Some(network_metadata) = network_interface.network_metadata(false) else {
					warn!("No network_metadata");
					return;
				};
				let point = network_metadata
					.persistent_metadata
					.navigation_metadata
					.node_graph_to_viewport
					.inverse()
					.transform_point2(ipp.mouse.position);
				// Disconnect if the wire was previously connected to an input
				if let (Some(wire_in_progress_from_connector), Some(wire_in_progress_to_connector)) = (self.wire_in_progress_from_connector, self.wire_in_progress_to_connector) {
					// Check if dragged connector is reconnected to another input
					let output_connector = network_interface.get_output_connector_from_click(wire_in_progress_from_connector);
					let input_connector = network_interface.get_input_connector_from_click(wire_in_progress_to_connector);

					if let (Some(input_connector), Some(output_connector)) = (input_connector, output_connector) {
						responses.add(NodeGraphMessage::CreateWire {
							input_connector,
							output_connector,
							use_document_network: false,
						});
						if let OutputConnector::Node(node_id, _) = output_connector {
							if network_interface.connected_to_output(&node_id) {
								responses.add(NodeGraphMessage::RunDocumentGraph);
							}
						} else {
							// Creating wire to export, always run graph
							responses.add(NodeGraphMessage::RunDocumentGraph);
						}
						responses.add(NodeGraphMessage::SendGraph);
					} else if node_from.is_some() && node_to.is_none() && !self.initial_disconnecting {
						// If the add node menu is already open, we don't want to open it again
						if self.context_menu.is_some() {
							return;
						}
						let Some(network_metadata) = network_interface.network_metadata(false) else {
							warn!("No network_metadata");
							return;
						};

						let appear_right_of_mouse = if ipp.mouse.position.x > ipp.viewport_bounds.size().x - 173. { -173. } else { 0. };
						let appear_above_mouse = if ipp.mouse.position.y > ipp.viewport_bounds.size().y - 34. { -34. } else { 0. };
						let node_graph_shift = DVec2::new(appear_right_of_mouse, appear_above_mouse) / network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.matrix2.x_axis.x;

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
									.get(0)
									.is_some_and(|first_selected_node| *first_selected_node != select_if_not_dragged))
						{
							responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![select_if_not_dragged] })
						}
					}

					// Check if a single node was dragged onto a wire
					if selected_nodes.selected_nodes_ref().len() == 1 {
						let selected_node_id = selected_nodes.selected_nodes_ref()[0];
						let Some(network) = network_interface.network(false) else {
							return;
						};
						// Ensure selected_node is not the exports/imports node
						if let Some(selected_node) = network.nodes.get(&selected_node_id) {
							// Check if any downstream node has any input that feeds into the primary export of the selected node
							let node = outward_wires.get(&selected_node_id).or_else(|| network.nodes.get(&selected_node_id)) else {
								log::error!("Could not get node");
								return;
							};
							let has_primary_output_connection = network_interface.collect_outward_wires(false).get(&selected_node_id).iter().any(|downstream_node_id| {
								let Some(downstream_node) = network.nodes.get(downstream_node_id) else {
									log::error!("Could not get downstream node");
									return;
								};
								downstream_node.inputs.iter().any(|input| {
									if let NodeInput::Node { node_id, output_index, .. } = input {
										if *node_id == selected_node_id && output_index == 0 {
											true
										} else {
											false
										}
									} else {
										false
									}
								})
							});
							let has_primary_input_connection = selected_node.inputs.get(0).is_some_and(|first_input| first_input.as_value().is_some());
							// Check that neither the primary input or output of the selected node are already connected.
							if !has_primary_output_connection && !has_primary_input_connection {
								let Some(bounding_box) = network_interface.node_bounding_box(selected_node_id) else {
									log::error!("Could not get bounding box for node: {selected_node_id}");
									return;
								};
								// TODO: Cache all wire locations if this is a performance issue
								let overlapping_wire = Self::collect_wires(&network_interface).into_iter().find(|frontend_wire| {
									let Some(input_position) = network_metadata.get_input_position(frontend_wire.wire_end, frontend_wire.wire_end_input_index) else {
										log::error!("Could not get input port position for {}", frontend_wire.wire_end);
										return false;
									};

									let Some(output_position) = network_metadata.get_output_position(frontend_wire.wire_start, frontend_wire.wire_start_output_index) else {
										log::error!("Could not get output port position for {}", frontend_wire.wire_start);
										return false;
									};

									let start_node_is_layer = network_interface.is_layer(&frontend_wire.wire_start).unwrap_or(false);
									let end_node_is_layer = network_interface.is_layer(&frontend_wire.wire_end).unwrap_or(false);

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
										// Ensure connection is to first visible input of selected node. If it does not have an input then do not connect
										if let Some((selected_node_input_index, _)) = selected_node
											.inputs
											.iter()
											.enumerate()
											.find(|(input_index, input)| input.is_exposed_to_frontend(network_interface.is_document_network()))
										{
											// Insert node between overlapping wire
											let input_connector = if let Some(post_node) = network.nodes.get(&overlapping_wire.wire_end) {
												let Some((input_index, _)) = post_node
													.inputs
													.iter()
													.enumerate()
													.filter(|(index, input)| input.is_exposed_to_frontend(network_interface.is_document_network()))
													.nth(overlapping_wire.wire_end_input_index)
												else {
													log::error!("Could not get input_index when inserting node between");
													return;
												};
												InputConnector::node(overlapping_wire.wire_end, input_index)
											}
											// Post node is the Exports node, so use InputConnector::Export
											else {
												// Exports cannot be hidden, so the visible input index will be correct
												InputConnector::Export(overlapping_wire.wire_end_input_index)
											};

											let output_connector = OutputConnector::node(overlapping_wire.wire_start, overlapping_wire.wire_start_output_index);
											responses.add(DocumentMessage::StartTransaction);
											// Create wire between the inserted node and the post node
											responses.add(NodeGraphMessage::CreateWire {
												output_connector: OutputConnector::node(selected_node_id, 0),
												input_connector,
												use_document_network: false,
											});
											// Create wire between the pre node and the inserted node
											responses.add(NodeGraphMessage::CreateWire {
												output_connector,
												input_connector: InputConnector::node(selected_node_id, 0),
												use_document_network: false,
											});

											if let OutputConnector::Node(node_id, _) = output_connector {
												if network_interface.connected_to_output(&node_id) {
													responses.add(NodeGraphMessage::RunDocumentGraph);
												}
											} else {
												// Creating wire to export node, always run graph
												responses.add(NodeGraphMessage::RunDocumentGraph);
											}
											responses.add(NodeGraphMessage::SendGraph);
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
				let Some(network) = network_interface.network_for_selected_nodes(selected_nodes.selected_nodes_ref().iter()) else {
					warn!("No network");
					return;
				};

				// TODO: This will also have to print all metadata
				// for (_, node_to_print) in network
				// 	.nodes
				// 	.iter()
				// 	.filter(|node_id| selected_nodes.selected_nodes().any(|selected_id| selected_id == node_id.0))
				// {
				// 	if let DocumentNodeImplementation::Network(network) = &node_to_print.implementation {
				// 		let mut output = "\r\n\r\n".to_string();
				// 		output += &node_to_print.name;
				// 		output += ":\r\n\r\n";
				// 		let mut nodes = network.nodes.iter().collect::<Vec<_>>();
				// 		nodes.sort_by_key(|(a, _)| a.0);
				// 		output += &nodes
				// 			.iter()
				// 			.map(|(_, node)| {
				// 				format!(
				// 					"metadata: DocumentNodeMetadata {{ position: glam::IVec2::new({}, {}) }}, // {}",
				// 					node.metadata.position.x, node.metadata.position.y, node.name
				// 				)
				// 			})
				// 			.collect::<Vec<_>>()
				// 			.join("\r\n");
				// 		output += "\r\n";
				// 		output += &format!(
				// 			"imports_metadata: (NodeId(generate_uuid()), ({}, {}).into()),\r\n",
				// 			network.imports_metadata.1.x, network.imports_metadata.1.y
				// 		);
				// 		output += &format!(
				// 			"exports_metadata: (NodeId(generate_uuid()), ({}, {}).into()),",
				// 			network.exports_metadata.1.x, network.exports_metadata.1.y
				// 		);
				// 		output += "\r\n\r\n";
				// 		// KEEP THIS `debug!()` - Someday we can remove this once this development utility is no longer needed
				// 		log::debug!("{output}");
				// 	}
				// }
			}
			NodeGraphMessage::RunDocumentGraph => {
				responses.add(PortfolioMessage::SubmitGraphRender { document_id });
			}
			NodeGraphMessage::SelectedNodesAdd { nodes } => {
				selected_nodes.add_selected_nodes(nodes, network_interface);
				responses.add(BroadcastEvent::SelectionChanged);
			}
			NodeGraphMessage::SelectedNodesRemove { nodes } => {
				selected_nodes.retain_selected_nodes(|node| !nodes.contains(node));
				responses.add(BroadcastEvent::SelectionChanged);
			}
			NodeGraphMessage::SelectedNodesSet { nodes } => {
				selected_nodes.set_selected_nodes(nodes, network_interface);
				responses.add(BroadcastEvent::SelectionChanged);
				responses.add(PropertiesPanelMessage::Refresh);
			}
			NodeGraphMessage::SendGraph => {
				self.send_graph(network_interface, document_metadata, collapsed, graph_view_overlay_open, responses);
			}
			NodeGraphMessage::SetInputValue { node_id, input_index, value } => {
				let use_document_network = network_interface.selected_nodes_in_document_network(std::iter::once(&node_id));
				let input = NodeInput::Value { tagged_value: value, exposed: false };
				responses.add(NodeGraphMessage::SetNodeInput {
					input_connector: InputConnector::node(node_id, input_index),
					input,
					use_document_network,
				});
				responses.add(PropertiesPanelMessage::Refresh);
				if (network_interface.get_reference(&node_id, use_document_network) != "Imaginate" || input_index == 0) && network_interface.connected_to_output(&node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
			}
			NodeGraphMessage::SetNodeInput {
				input_connector,
				input,
				use_document_network,
			} => {
				network_interface.set_input(input_connector, input, use_document_network);
				load_network_structure(document_network, document_metadata, collapsed);
			}
			NodeGraphMessage::ShiftNodes {
				node_ids,
				displacement_x,
				displacement_y,
			} => {
				for node_id in node_ids {
					network_interface.shift_node(node_id, IVec2::new(displacement_x, displacement_y));
				}
				if graph_view_overlay_open {
					responses.add(NodeGraphMessage::SendGraph);
					responses.add(DocumentMessage::RenderRulers);
					responses.add(DocumentMessage::RenderScrollbars);
				}
			}
			NodeGraphMessage::ToggleSelectedVisibility => {
				let Some(network) = network_interface.network_for_selected_nodes(selected_nodes.selected_nodes_ref().iter()) else {
					return;
				};
				responses.add(DocumentMessage::StartTransaction);

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let visible = !selected_nodes.selected_nodes().all(|&node_id| network.nodes.get(&node_id).is_some_and(|node| node.visible));

				for &node_id in selected_nodes.selected_nodes() {
					responses.add(NodeGraphMessage::SetVisibility { node_id, visible });
				}
			}
			NodeGraphMessage::ToggleVisibility { node_id } => {
				let Some(network) = network_interface.network_for_selected_nodes(std::iter::once(&node_id)) else {
					return;
				};

				let Some(node) = network.nodes.get(&node_id) else {
					log::error!("Cannot get node {node_id} in NodeGraphMessage::ToggleVisibility");
					return;
				};

				let visible = !node.visible;

				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetVisibility { node_id, visible });
			}
			NodeGraphMessage::SetVisibility { node_id, visible } => {
				network_interface.set_visibility(node_id, is_layer);

				// Only generate node graph if one of the selected nodes is connected to the output
				if network_interface.connected_to_output(&node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				// If change has been made to document_network
				if network_interface.selected_nodes_in_document_network(std::iter::once(&node_id)) {
					document_metadata.load_structure(document_network);
				}

				self.update_selection_action_buttons(document_network, selected_nodes, responses);
				responses.add(PropertiesPanelMessage::Refresh);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::SetLocked { node_id, locked } => {
				network_interface.set_locked(node_id, locked);

				if network_interface.connected_to_output(&node_id) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				// If change has been made to document_network
				if network_interface.selected_nodes_in_document_network(std::iter::once(&node_id)) {
					document_metadata.load_structure(document_network);
				}
				self.update_selection_action_buttons(document_network, selected_nodes, responses);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::ToggleSelectedAsLayersOrNodes => {
				for node_id in selected_nodes.selected_nodes() {
					responses.add(NodeGraphMessage::SetToNodeOrLayer {
						node_id: *node_id,
						is_layer: !network_interface.is_layer(node_id),
					});
				}
				if selected_nodes.selected_nodes().any(|node_id| network_interface.connected_to_output(node_id)) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
			}
			NodeGraphMessage::SetToNodeOrLayer { node_id, is_layer } => {
				if is_layer && !network_interface.is_eligible_to_be_layer(&node_id) {
					return;
				}

				network_interface.set_to_node_or_layer(node_id, is_layer);

				self.context_menu = None;
				responses.add(FrontendMessage::UpdateContextMenuInformation {
					context_menu_information: self.context_menu.clone(),
				});
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(DocumentMessage::DocumentStructureChanged);
			}
			NodeGraphMessage::SetAlias { node_id, alias } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetAliasImpl { node_id, alias });
			}
			NodeGraphMessage::SetAliasImpl { node_id, alias } => {
				network_interface.set_alias(node_id, alias);

				responses.add(DocumentMessage::RenderRulers);
				responses.add(DocumentMessage::RenderScrollbars);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::TogglePreview { node_id } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::TogglePreviewImpl { node_id });
			}
			NodeGraphMessage::TogglePreviewImpl { node_id } => {
				network_interface.toggle_preview(node_id);

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
				network_interface.resolved_types = resolved_types;
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

	/// Send the cached layout to the frontend for the options bar at the top of the node panel
	fn send_node_bar_layout(&self, responses: &mut VecDeque<Message>) {
		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout::new(self.widgets.to_vec())),
			layout_target: LayoutTarget::NodeGraphBar,
		});
	}

	/// Updates the buttons for visibility, locked, and preview
	fn update_selection_action_buttons(&mut self, network_interface: &NodeNetworkInterface, selected_nodes: &SelectedNodes, responses: &mut VecDeque<Message>) {
		let Some(network) = network_interface.network_for_selected_nodes(selected_nodes.selected_nodes_ref().iter()) else {
			warn!("No network in update_selection_action_buttons");
			return;
		};
		let mut widgets = Vec::new();

		// Don't allow disabling input or output nodes
		let mut selection = selected_nodes
			.selected_nodes()
			.filter(|node_id| **node_id != network.imports_metadata.0 && **node_id != network.exports_metadata.0);

		// If there is at least one other selected node then show the hide or show button
		if selection.next().is_some() {
			// Check if any of the selected nodes are disabled
			let all_visible = selected_nodes.selected_nodes().all(|id| {
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

		let mut selection = selected_nodes.selected_nodes();
		// If only one node is selected then show the preview or stop previewing button
		if let (Some(&node_id), None) = (selection.next(), selection.next()) {
			// Is this node the current output
			let is_output = network.outputs_contain(node_id);
			let is_previewing = matches!(network.previewing, Previewing::Yes { .. });

			// Prevent showing "End Preview"/"Preview" if the root node is the output, or the import/export node
			let is_import_or_export = node_id == network.imports_metadata.0 || node_id == network.exports_metadata.0;
			if !is_import_or_export && network == network {
				let output_button = TextButton::new(if is_output && is_previewing { "End Preview" } else { "Preview" })
					.icon(Some("Rescale".to_string()))
					.tooltip(if is_output { "Restore preview to the graph output" } else { "Preview selected node/layer" }.to_string() + " (Shortcut: Alt-click node/layer)")
					.on_update(move |_| NodeGraphMessage::TogglePreview { node_id }.into())
					.widget_holder();
				widgets.push(output_button);
			}
		}

		self.widgets[0] = LayoutGroup::Row { widgets };
		self.send_node_bar_layout(responses);
	}

	/// Collate the properties panel sections for a node graph
	pub fn collate_properties(context: &mut NodePropertiesContext, selected_nodes: &SelectedNodes) -> Vec<LayoutGroup> {
		// If the selected nodes are in the document network, use the document network. Otherwise, use the nested network
		let Some(network) = context.network_interface.network_for_selected_nodes(selected_nodes.selected_nodes_ref().iter()) else {
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
					.filter(|&selected_node_id| !context.network_interface.is_node_upstream_of_another_by_horizontal_flow(layers[0], selected_node_id));
				if nodes_not_upstream_of_layer.count() > 0 {
					return Vec::new();
				}

				// Iterate through all the upstream nodes, but stop when we reach another layer (since that's a point where we switch from horizontal to vertical flow)
				context
					.network_interface
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

	fn collect_wires(network_interface: &NodeNetworkInterface) -> Vec<FrontendNodeWire> {
		let Some(network) = network_interface.network(false) else {
			log::error!("Could not get network when collecting wires");
			return Vec::new();
		};
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
		if let Some((root_node_id, root_node_output_index)) = network_interface.get_root_node(false).and_then(|output_connector| {
			if let OutputConnector::Node { node_id, output_index } = output_connector {
				Some((node_id, output_index))
			} else {
				None
			}
		}) {
			wires.push(FrontendNodeWire {
				wire_start: root_node_id,
				wire_start_output_index: root_node_output_index,
				wire_end: network.exports_metadata.0,
				wire_end_input_index: 0,
				dashed: false,
			});
		}

		// Connect rest of exports to their actual export field since they are not affected by previewing. Only connect the primary export if it is dashed
		for (i, export) in network.exports.iter().enumerate() {
			if let NodeInput::Node { node_id, output_index, .. } = export {
				let dashed = matches!(network_interface.previewing(false), Previewing::Yes { .. }) && i == 0;
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

	fn collect_nodes(&self, network_interface: &NodeNetworkInterface, wires: &[FrontendNodeWire]) -> Vec<FrontendNode> {
		let Some(network) = network_interface.network(false) else {
			log::error!("Could not get nested network when collecting nodes");
			return Vec::new();
		};
		let Some(network_metadata) = network_interface.network_metadata(false) else {
			log::error!("Could not get network_metadata when collecting nodes");
			return Vec::new();
		};

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
			let node_id_path = &[network_interface.network_path().clone(), &[node_id]].concat();
			let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(&node_id) else {
				log::error!("Could not get node_metadata for {node_id_path}");
				continue;
			};

			let frontend_graph_inputs = node.inputs.iter().enumerate().map(|(index, _)| {
				// Convert the index in all inputs to the index in only the exposed inputs
				// TODO: Only display input type if potential inputs in node_registry are all the same type
				let input_type = network_interface.resolved_types.inputs.get(&Source { node: node_id_path.clone(), index }).cloned();

				// TODO: Should display the color of the "most commonly relevant" (we'd need some sort of precedence) data type it allows given the current generic form that's constrained by the other present connections.
				let frontend_data_type = if let Some(ref input_type) = input_type {
					FrontendGraphDataType::with_type(input_type)
				} else {
					FrontendGraphDataType::General
				};

				let input_name = node_metadata
					.persistent_metadata
					.input_names
					.get(index)
					.unwrap_or(network_interface.get_input_type(node_id, index, false).nested_type());

				FrontendGraphInput {
					data_type: frontend_data_type,
					name: input_name.to_string(),
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
					input.is_exposed_to_frontend(network_interface.is_document_network())
				})
				.map(|(_, input_type)| input_type);
			let exposed_inputs = inputs
				.filter(|(input, _)| input.is_exposed_to_frontend(network_interface.is_document_network()))
				.map(|(_, input_type)| input_type)
				.collect();

			let output_types = Self::get_output_types(node, &network_interface.resolved_types, &node_id_path);
			let primary_output_type = output_types.get(0).expect("Primary output should always exist");
			let frontend_data_type = if let Some(output_type) = primary_output_type {
				FrontendGraphDataType::with_type(&output_type)
			} else {
				FrontendGraphDataType::General
			};
			let (connected, connected_index) = connected_node_to_output_lookup.get(&(node_id, 0)).unwrap_or(&(Vec::new(), Vec::new())).clone();
			let primary_output = if network_interface.has_primary_output(&node_id) {
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
				if index == 0 && network_interface.has_primary_output(&node_id) {
					continue;
				}
				let frontend_data_type = if let Some(output_type) = &exposed_output {
					FrontendGraphDataType::with_type(output_type)
				} else {
					FrontendGraphDataType::General
				};
				let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(&node_id) else {
					log::error!("Could not get node_metadata when getting output for {node_id}");
					continue;
				};
				let output_name = node_metadata
					.persistent_metadata
					.output_names
					.get(index)
					.map(|output_name| output_name.to_string())
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
			let reference = network_interface.get_reference(&node_id, false);

			let Some(position) = network_interface.get_position(&node_id, network_interface.collect_outward_wires(false), false) else {
				log::error!("Could not get position for node: {node_id}");
				continue;
			};
			let previewed = is_export && !is_root_node;

			let locked = network_interface.is_locked(&node_id, false);

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
				is_layer: network_interface.persistent_node_metadata(&node_id).is_some_and(|node_metadata| node_metadata.is_layer()),
				can_be_layer: network_interface.is_eligible_to_be_layer(&node_id),
				alias: network_interface.untitled_layer_label(&node_id, false),
				reference,
				primary_input,
				exposed_inputs,
				primary_output,
				exposed_outputs,
				position,
				previewed,
				visible: node.visible,
				locked: locked,
				errors: errors,
				ui_only: false,
			});
		}

		// Get import/export names from parent node metadata input/outputs. None means to use type, or "Import/Export + index" if type can't be determined
		let mut import_names = Vec::new();
		let mut export_names = vec![None; network.exports.len()];

		if let Some(encapsulating_metadata) = network_interface.encapsulating_node_metadata() {
			// Get all import/export names from encapsulating node metadata
			import_names = encapsulating_metadata.persistent_metadata.input_names;
			export_names = encapsulating_metadata.persistent_metadata.output_names;
		}

		// Add "Export" UI-only node
		let mut export_node_inputs = Vec::new();
		for (index, export) in network.exports.iter().enumerate() {
			let (frontend_data_type, input_type) = if let NodeInput::Node { node_id, output_index, .. } = export {
				let node = network.nodes.get(node_id).expect("Node should always exist");
				let node_id_path: &Vec<NodeId> = &[&self.network[..], &[*node_id]].concat();
				let output_types = Self::get_output_types(node, &network_interface.resolved_types, &node_id_path);

				if let Some(output_type) = output_types.get(*output_index).cloned().flatten() {
					(FrontendGraphDataType::with_type(&output_type), Some(output_type.clone()))
				} else {
					(FrontendGraphDataType::General, None)
				}
			} else if let NodeInput::Value { tagged_value, .. } = export {
				(FrontendGraphDataType::with_type(&tagged_value.ty()), Some(tagged_value.ty()))
			}
			// TODO: Get type from parent node input when <https://github.com/GraphiteEditor/Graphite/issues/1762> is possible
			// else if let NodeInput::Network { import_type, .. } = export {
			// 	(FrontendGraphDataType::with_type(import_type), Some(import_type.clone()))
			// }
			else {
				(FrontendGraphDataType::General, None)
			};

			// First import index is visually connected to the root node instead of its actual export input so previewing does not change the connection
			let connected = if index == 0 {
				network_interface.get_root_node(false).and_then(|root_node| root_node.node_id())
			} else {
				if let NodeInput::Node { node_id, .. } = export {
					Some(*node_id)
				} else {
					None
				}
			};

			// `export_names` is pre-initialized with None, so this is safe
			let export_name = export_names[index]
				.clone()
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
			reference: None,
			alias: "Exports".to_string(),
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
		if !network_interface.is_document_network() {
			let mut import_node_outputs = Vec::new();
			for (index, import_name) in import_names.into_iter().enumerate() {
				let (connected, connected_index) = connected_node_to_output_lookup.get(&(network.imports_metadata.0, index)).unwrap_or(&(Vec::new(), Vec::new())).clone();
				// TODO: https://github.com/GraphiteEditor/Graphite/issues/1767
				// TODO: Non exposed inputs are not added to the inputs_source_map, fix `pub fn document_node_types(&self) -> ResolvedDocumentNodeTypes`
				let input_type = network_interface.resolved_types.inputs.get(&Source { node: self.network.clone(), index }).cloned();

				let frontend_data_type = if let Some(input_type) = input_type.clone() {
					FrontendGraphDataType::with_type(&input_type)
				} else {
					FrontendGraphDataType::General
				};

				let import_name = import_name
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
				reference: None,
				alias: "Imports".to_string(),
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

	fn collect_subgraph_names(network_interface: &mut NodeNetworkInterface) -> Option<Vec<String>> {
		let mut current_network = network_interface.document_network();
		let mut subgraph_names = Vec::new();
		for node_id in subgraph_path.iter() {
			if let Some(node) = current_network.nodes.get(node_id) {
				if let Some(network) = node.implementation.get_network() {
					current_network = network;
				}

				// TODO: Maybe replace with alias and default to name if it does not exist
				subgraph_names.push(node.name.clone());
			} else {
				// If node cannot be found and we are in a nested network, set subgraph_path to document network and return None, which runs send_graph again on the document network
				if !subgraph_path.is_empty() {
					network_interface.exit_all_nested_networks();
					return None;
				} else {
					return Some(Vec::new());
				}
			};
		}
		Some(subgraph_names)
	}

	fn update_layer_panel(network_interface: &NodeNetworkInterface, metadata: &DocumentMetadata, collapsed: &CollapsedLayers, responses: &mut VecDeque<Message>) {
		for (&node_id, node_metadata) in &network_interface.document_network_metadata().persistent_metadata.node_metadata {
			if node_metadata.persistent_metadata.is_layer() {
				let layer = LayerNodeIdentifier::new(node_id, network_interface.document_network());

				let parents_visible = layer.ancestors(metadata).filter(|&ancestor| ancestor != layer).all(|layer| {
					if layer != LayerNodeIdentifier::ROOT_PARENT {
						network_interface.document_network().nodes.get(&layer.to_node()).map(|node| node.visible).unwrap_or_default()
					} else {
						true
					}
				});

				let parents_unlocked = layer.ancestors(metadata).filter(|&ancestor| ancestor != layer).all(|layer| {
					if layer != LayerNodeIdentifier::ROOT_PARENT {
						network_interface.document_network().nodes.get(&layer.to_node()).map(|node| !node.locked).unwrap_or_default()
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
							network_interface.document_network().nodes.get(&node_id).map_or_else(||{log::error!("Could not get node {node_id} in update_layer_panel"); false}, |node_id| node_id.inputs.iter().skip(1).any(|input| input.is_exposed())) &&
							// But nothing is connected to it, since we only get 1 item (ourself) when we ask for the flow from the secondary input
							network_interface.upstream_flow_back_from_nodes(vec![node_id], FlowType::HorizontalFlow).count() == 1
						),
					children_present: layer.has_children(metadata),
					expanded: layer.has_children(metadata) && !collapsed.0.contains(&layer),
					depth: layer.ancestors(metadata).count() - 1,
					parent_id: layer.parent(metadata).and_then(|parent| if parent != LayerNodeIdentifier::ROOT_PARENT { Some(parent.to_node()) } else { None }),
					//reference: network_interface.get_reference(&node_id, true),
					alias: network_interface.untitled_layer_label(&node_id, true),
					tooltip: if cfg!(debug_assertions) { format!("Layer ID: {node_id}") } else { "".into() },
					visible: network_interface.is_visible(&node_id, true),
					parents_visible,
					unlocked: !network_interface.is_locked(&node_id, true),
					parents_unlocked,
				};
				responses.add(FrontendMessage::UpdateDocumentLayerDetails { data });
			}
		}
	}

	fn send_graph(&mut self, network_interface: &mut NodeNetworkInterface, metadata: &mut DocumentMetadata, collapsed: &CollapsedLayers, graph_open: bool, responses: &mut VecDeque<Message>) {
		// If a node cannot be found in collect_subgraph_names, for example when the nested node is deleted while it is entered, and we are in a nested network, set self.network to empty (document network), and call send_graph again to send the document network
		let Some(nested_path) = Self::collect_subgraph_names(network_interface) else {
			self.send_graph(network_interface, metadata, collapsed, graph_open, responses);
			return;
		};

		let Some(network) = network_interface.network(false) else {
			log::error!("Could not send graph since nested network does not exist");
			return;
		};

		responses.add(DocumentMessage::DocumentStructureChanged);
		responses.add(PropertiesPanelMessage::Refresh);

		metadata.load_structure(document_network);

		Self::update_layer_panel(document_network, metadata, collapsed, responses);

		if graph_open {
			let wires = Self::collect_wires(network_interface);
			let nodes = self.collect_nodes(network_interface, &wires);

			responses.add(FrontendMessage::UpdateNodeGraph { nodes, wires });
			responses.add(FrontendMessage::UpdateSubgraphPath { subgraph_path: nested_path });
			let layer_widths = self
				.node_metadata
				.iter()
				.filter_map(|(node_id, node_metadata)| node_metadata.layer_width.map(|layer_width| (*node_id, layer_width)))
				.collect::<HashMap<NodeId, u32>>();
			responses.add(FrontendMessage::UpdateLayerWidths { layer_widths });
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
				} else if let NodeInput::Network { import_index, .. } = current_export {
					resolved_types
						.outputs
						.get(&Source {
							node: node_id_path.clone(),
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
	fn update_selected(&mut self, network_interface: &NodeNetworkInterface, selected_nodes: &SelectedNodes, responses: &mut VecDeque<Message>) {
		self.update_selection_action_buttons(network_interface, selected_nodes, responses);

		responses.add(FrontendMessage::UpdateNodeGraphSelection {
			selected: selected_nodes.selected_nodes_ref().clone(),
		});
	}

	fn untitled_layer_label(node: &DocumentNode) -> String {}

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

		return vec![
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
		];
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
			auto_panning: Default::default(),
		}
	}
}

impl PartialEq for NodeGraphMessageHandler {
	fn eq(&self, other: &Self) -> bool {
		self.network == other.network
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
