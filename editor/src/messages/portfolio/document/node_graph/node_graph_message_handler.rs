use super::utility_types::{BoxSelection, ContextMenuInformation, DragStart, FrontendGraphInput, FrontendGraphOutput, FrontendNode, FrontendNodeWire, WirePath};
use super::{document_node_definitions, node_properties};
use crate::application::generate_uuid;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::NodePropertiesContext;
use crate::messages::portfolio::document::node_graph::utility_types::{ContextMenuData, Direction, FrontendGraphDataType};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{self, InputConnector, NodeNetworkInterface, NodeTemplate, OutputConnector, Previewing};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, LayerPanelEntry};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;

use graph_craft::document::{DocumentNodeImplementation, NodeId, NodeInput};
use graph_craft::proto::GraphErrors;
use graphene_core::*;
use renderer::{ClickTarget, Quad};

use glam::{DAffine2, DVec2, IVec2};
use std::cmp::Ordering;

#[derive(Debug)]
pub struct NodeGraphHandlerData<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub selection_network_path: &'a [NodeId],
	pub breadcrumb_network_path: &'a [NodeId],
	pub document_id: DocumentId,
	pub collapsed: &'a mut CollapsedLayers,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub graph_view_overlay_open: bool,
}

#[derive(Debug, Clone)]
pub struct NodeGraphMessageHandler {
	// TODO: Remove network and move to NodeNetworkInterface
	pub network: Vec<NodeId>,
	pub node_graph_errors: GraphErrors,
	has_selection: bool,
	widgets: [LayoutGroup; 2],
	pub drag_start: Option<DragStart>,
	/// Used to add a transaction for the first node move when dragging.
	begin_dragging: bool,
	/// Stored in node graph coordinates
	box_selection_start: Option<DVec2>,
	/// If the grip icon is held during a drag, then shift without pushing other nodes
	shift_without_push: bool,
	disconnecting: Option<InputConnector>,
	initial_disconnecting: bool,
	/// Node to select on pointer up if multiple nodes are selected and they were not dragged.
	select_if_not_dragged: Option<NodeId>,
	/// The start of the dragged line (cannot be moved), stored in node graph coordinates
	pub wire_in_progress_from_connector: Option<DVec2>,
	/// The end point of the dragged line (cannot be moved), stored in node graph coordinates
	pub wire_in_progress_to_connector: Option<DVec2>,
	/// State for the context menu popups.
	pub context_menu: Option<ContextMenuInformation>,
	/// Index of selected node to be deselected on pointer up when shift clicking an already selected node
	pub deselect_on_pointer_up: Option<usize>,
	/// Adds the auto panning functionality to the node graph when dragging a node or selection box to the edge of the viewport.
	auto_panning: AutoPanning,
	/// The node to preview on mouse up if alt-clicked
	preview_on_mouse_up: Option<NodeId>,
}

/// NodeGraphMessageHandler always modifies the network which the selected nodes are in. No GraphOperationMessages should be added here, since those messages will always affect the document network.
impl<'a> MessageHandler<NodeGraphMessage, NodeGraphHandlerData<'a>> for NodeGraphMessageHandler {
	fn process_message(&mut self, message: NodeGraphMessage, responses: &mut VecDeque<Message>, data: NodeGraphHandlerData<'a>) {
		let NodeGraphHandlerData {
			network_interface,
			selection_network_path,
			breadcrumb_network_path,
			document_id,
			collapsed,
			graph_view_overlay_open,
			ipp,
		} = data;

		match message {
			// TODO: automatically remove broadcast messages.
			NodeGraphMessage::AddNodes { nodes, new_ids } => {
				let Some(new_layer_id) = new_ids.get(&NodeId(0)).cloned().or_else(|| nodes.first().map(|(node_id, _)| *node_id)) else {
					log::error!("No nodes to add in AddNodes");
					return;
				};
				network_interface.insert_node_group(nodes, new_ids, selection_network_path);

				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![new_layer_id] });
			}
			NodeGraphMessage::Init => {
				responses.add(BroadcastMessage::SubscribeEvent {
					on: BroadcastEvent::SelectionChanged,
					send: Box::new(NodeGraphMessage::SelectedNodesUpdated.into()),
				});
				network_interface.load_structure();
				collapsed.0.retain(|&layer| network_interface.document_metadata().layer_exists(layer));
			}
			NodeGraphMessage::SelectedNodesUpdated => {
				let selected_layers = network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_layers(network_interface.document_metadata())
					.collect::<Vec<_>>();
				if selected_layers.len() <= 1 {
					responses.add(DocumentMessage::SetRangeSelectionLayer {
						new_layer: selected_layers.first().cloned(),
					});
				}
				responses.add(NodeGraphMessage::UpdateLayerPanel);
				responses.add(NodeGraphMessage::SendSelectedNodes);
				responses.add(ArtboardToolMessage::UpdateSelectedArtboard);
				responses.add(DocumentMessage::DocumentStructureChanged);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::CreateWire { output_connector, input_connector } => {
				// TODO: Add support for flattening NodeInput::Network exports in flatten_with_fns https://github.com/GraphiteEditor/Graphite/issues/1762
				if matches!(input_connector, InputConnector::Export(_)) && matches!(output_connector, OutputConnector::Import { .. }) {
					responses.add(DialogMessage::RequestComingSoonDialog { issue: Some(1762) });
					return;
				}
				network_interface.create_wire(&output_connector, &input_connector, selection_network_path);
			}
			NodeGraphMessage::Copy => {
				let all_selected_nodes = network_interface.upstream_chain_nodes(selection_network_path);
				// Collect the selected nodes
				let new_ids = &all_selected_nodes.iter().enumerate().map(|(new, old)| (*old, NodeId(new as u64))).collect();
				let copied_nodes = network_interface.copy_nodes(new_ids, selection_network_path).collect::<Vec<_>>();

				// Prefix to show that these are nodes
				let mut copy_text = String::from("graphite/nodes: ");
				copy_text += &serde_json::to_string(&copied_nodes).expect("Could not serialize copy");

				responses.add(FrontendMessage::TriggerTextCopy { copy_text });
			}
			NodeGraphMessage::CreateNodeFromContextMenu { node_id, node_type, x, y } => {
				let node_id = node_id.unwrap_or_else(|| NodeId(generate_uuid()));

				let Some(document_node_type) = document_node_definitions::resolve_document_node_type(&node_type) else {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Cannot insert node".to_string(),
						description: format!("The document node '{node_type}' does not exist in the document node list"),
					});
					return;
				};

				let node_template = document_node_type.default_node_template();
				self.context_menu = None;

				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::InsertNode {
					node_id,
					node_template: node_template.clone(),
				});
				responses.add(NodeGraphMessage::ShiftNodePosition { node_id, x, y });
				// Only auto connect to the dragged wire if the node is being added to the currently opened network
				if let Some(output_connector_position) = self.wire_in_progress_from_connector {
					let Some(network_metadata) = network_interface.network_metadata(selection_network_path) else {
						log::error!("Could not get network metadata in CreateNodeFromContextMenu");
						return;
					};
					let output_connector_position_viewport = network_metadata
						.persistent_metadata
						.navigation_metadata
						.node_graph_to_viewport
						.transform_point2(output_connector_position);
					let Some(output_connector) = &network_interface.output_connector_from_click(output_connector_position_viewport, breadcrumb_network_path) else {
						log::error!("Could not get output from connector start");
						return;
					};

					// Ensure connection is to correct input of new node. If it does not have an input then do not connect
					if let Some((input_index, _)) = node_template
						.document_node
						.inputs
						.iter()
						.enumerate()
						.find(|(_, input)| input.is_exposed_to_frontend(selection_network_path.is_empty()))
					{
						responses.add(NodeGraphMessage::CreateWire {
							output_connector: output_connector.clone(),
							input_connector: InputConnector::node(node_id, input_index),
						});

						responses.add(NodeGraphMessage::RunDocumentGraph);
					}

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
				responses.add(NodeGraphMessage::DeleteSelectedNodes { delete_children: true });
			}
			NodeGraphMessage::DeleteNodes { node_ids, delete_children } => {
				network_interface.delete_nodes(node_ids, delete_children, selection_network_path);
				responses.add(NodeGraphMessage::SelectedNodesUpdated);
				responses.add(NodeGraphMessage::SendGraph);
			}
			// Deletes selected_nodes. If `reconnect` is true, then all children nodes (secondary input) of the selected nodes are deleted and the siblings (primary input/output) are reconnected.
			// If `reconnect` is false, then only the selected nodes are deleted and not reconnected.
			NodeGraphMessage::DeleteSelectedNodes { delete_children } => {
				let Some(selected_nodes) = network_interface.selected_nodes(selection_network_path) else {
					log::error!("Could not get selected nodes in DeleteSelectedNodes");
					return;
				};
				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::DeleteNodes {
					node_ids: selected_nodes.selected_nodes().cloned().collect::<Vec<_>>(),
					delete_children,
				})
			}
			NodeGraphMessage::DisconnectInput { input_connector } => {
				network_interface.disconnect_input(&input_connector, selection_network_path);
			}
			NodeGraphMessage::DisconnectRootNode => {
				network_interface.start_previewing_without_restore(selection_network_path);
			}
			NodeGraphMessage::DuplicateSelectedNodes => {
				let all_selected_nodes = network_interface.upstream_chain_nodes(selection_network_path);

				let copy_ids = all_selected_nodes.iter().enumerate().map(|(new, id)| (*id, NodeId(new as u64))).collect::<HashMap<NodeId, NodeId>>();

				// Copy the selected nodes
				let nodes = network_interface.copy_nodes(&copy_ids, selection_network_path).collect::<Vec<_>>();

				let new_ids = nodes.iter().map(|(id, _)| (*id, NodeId(generate_uuid()))).collect::<HashMap<_, _>>();
				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::AddNodes { nodes, new_ids: new_ids.clone() });
				responses.add(NodeGraphMessage::SelectedNodesSet {
					nodes: new_ids.values().cloned().collect(),
				});
			}
			NodeGraphMessage::EnterNestedNetwork => {
				let Some(node_id) = network_interface.node_from_click(ipp.mouse.position, selection_network_path) else {
					return;
				};
				if network_interface.visibility_from_click(ipp.mouse.position, selection_network_path).is_some() {
					return;
				};
				let Some(network) = network_interface.network(selection_network_path) else {
					log::error!("Could not get network in EnterNestedNetwork");
					return;
				};

				let Some(node) = network.nodes.get(&node_id) else { return };
				if let DocumentNodeImplementation::Network(_) = node.implementation {
					responses.add(DocumentMessage::EnterNestedNetwork { node_id });
				}
			}
			NodeGraphMessage::ExposeInput { node_id, input_index, new_exposed } => {
				let Some(network) = network_interface.network(selection_network_path) else {
					return;
				};

				let Some(node) = network.nodes.get(&node_id) else {
					log::error!("Could not find node {node_id} in NodeGraphMessage::ExposeInput");
					return;
				};

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

				responses.add(DocumentMessage::AddTransaction);

				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, input_index),
					input,
				});
				responses.add(PropertiesPanelMessage::Refresh);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::InsertNode { node_id, node_template } => {
				network_interface.insert_node(node_id, node_template, selection_network_path);
			}
			NodeGraphMessage::InsertNodeBetween {
				node_id,
				input_connector,
				insert_node_input_index,
			} => {
				network_interface.insert_node_between(&node_id, &input_connector, insert_node_input_index, selection_network_path);
			}
			NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index } => {
				network_interface.move_layer_to_stack(layer, parent, insert_index, selection_network_path);
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

				responses.add(DocumentMessage::AddTransaction);

				let new_ids: HashMap<_, _> = data.iter().map(|(id, _)| (*id, NodeId(generate_uuid()))).collect();
				responses.add(NodeGraphMessage::AddNodes {
					nodes: data,
					new_ids: new_ids.clone(),
				});
			}
			NodeGraphMessage::PointerDown {
				shift_click,
				control_click,
				alt_click,
				right_click,
			} => {
				if selection_network_path != breadcrumb_network_path {
					log::error!("Selection network path does not match breadcrumb network path in PointerDown");
					return;
				}
				let Some(network_metadata) = network_interface.network_metadata(selection_network_path) else {
					log::error!("Could not get network metadata in PointerDown");
					return;
				};

				let click = ipp.mouse.position;

				let node_graph_point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);

				if network_interface.grip_from_click(click, selection_network_path).is_some() {
					self.shift_without_push = true;
				}

				let clicked_id = network_interface.node_from_click(click, selection_network_path);
				let clicked_input = network_interface.input_connector_from_click(click, selection_network_path);
				let clicked_output = network_interface.output_connector_from_click(click, selection_network_path);
				let network_metadata = network_interface.network_metadata(selection_network_path).unwrap();

				// Create the add node popup on right click, then exit
				if right_click {
					// Abort dragging a node
					if self.drag_start.is_some() {
						responses.add(DocumentMessage::AbortTransaction);
						self.drag_start = None;
						return;
					}
					// Abort a box selection
					if self.box_selection_start.is_some() {
						responses.add(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() });
						self.box_selection_start = None;
						return;
					}
					// Abort dragging a wire
					if self.wire_in_progress_from_connector.is_some() {
						responses.add(DocumentMessage::AbortTransaction);
						self.wire_in_progress_from_connector = None;
						self.wire_in_progress_to_connector = None;
						responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: None });
						return;
					}

					let context_menu_data = if let Some(node_id) = clicked_id {
						ContextMenuData::ToggleLayer {
							node_id,
							currently_is_node: !network_interface.is_layer(&node_id, selection_network_path),
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
					let context_menu_click_target = ClickTarget::new(context_menu_subpath, 0.);
					if context_menu_click_target.intersect_point(click, DAffine2::IDENTITY) {
						return;
					}
				}

				// Since the user is clicking elsewhere in the graph, ensure the add nodes list is closed
				if self.context_menu.is_some() {
					self.context_menu = None;
					self.wire_in_progress_from_connector = None;
					self.wire_in_progress_to_connector = None;
					responses.add(FrontendMessage::UpdateContextMenuInformation {
						context_menu_information: self.context_menu.clone(),
					});
					responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: None });
				}

				// Toggle visibility of clicked node and return
				if let Some(clicked_visibility) = network_interface.visibility_from_click(click, selection_network_path) {
					responses.add(NodeGraphMessage::ToggleVisibility { node_id: clicked_visibility });
					return;
				}

				// Alt-click sets the clicked node as previewed
				if alt_click {
					if let Some(clicked_node) = clicked_id {
						self.preview_on_mouse_up = Some(clicked_node);
					}
				}

				// Begin moving an existing wire
				if let Some(clicked_input) = &clicked_input {
					responses.add(DocumentMessage::StartTransaction);
					self.initial_disconnecting = true;
					self.disconnecting = Some(clicked_input.clone());

					let output_connector = if *clicked_input == InputConnector::Export(0) {
						network_interface.root_node(selection_network_path).map(|root_node| root_node.to_connector())
					} else {
						network_interface.upstream_output_connector(clicked_input, selection_network_path)
					};
					let Some(output_connector) = output_connector else { return };
					self.wire_in_progress_from_connector = network_interface.output_position(&output_connector, selection_network_path);
					return;
				}

				// Begin creating a new wire
				if let Some(clicked_output) = clicked_output {
					responses.add(DocumentMessage::StartTransaction);
					self.initial_disconnecting = false;
					// Disconnect vertical output wire from an already-connected layer
					if let OutputConnector::Node { node_id, .. } = clicked_output {
						if network_interface.is_layer(&node_id, selection_network_path) {
							if let Some(input_connectors) = network_interface.outward_wires(selection_network_path).and_then(|outward_wires| outward_wires.get(&clicked_output)) {
								self.disconnecting = input_connectors.first().cloned();
							}
						}
					}

					self.wire_in_progress_from_connector = network_interface.output_position(&clicked_output, selection_network_path);
					return;
				}

				if let Some(clicked_id) = clicked_id {
					let Some(selected_nodes) = network_interface.selected_nodes(selection_network_path) else {
						log::error!("Could not get selected nodes in PointerDown");
						return;
					};
					let mut updated_selected = selected_nodes.selected_nodes().cloned().collect::<Vec<_>>();
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
							start_x: node_graph_point.x,
							start_y: node_graph_point.y,
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

					responses.add(DocumentMessage::StartTransaction);
					return;
				}

				// Clicked on the graph background so we box select
				if !shift_click {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() })
				}
				self.box_selection_start = Some(node_graph_point);
			}
			NodeGraphMessage::PointerMove { shift } => {
				if selection_network_path != breadcrumb_network_path {
					log::error!("Selection network path does not match breadcrumb network path in PointerMove");
					return;
				}
				let Some(network_metadata) = network_interface.network_metadata(selection_network_path) else {
					return;
				};

				// Auto-panning
				let messages = [NodeGraphMessage::PointerOutsideViewport { shift }.into(), NodeGraphMessage::PointerMove { shift }.into()];
				self.auto_panning.setup_by_mouse_position(ipp, &messages, responses);

				let viewport_location = ipp.mouse.position;
				let point = network_metadata
					.persistent_metadata
					.navigation_metadata
					.node_graph_to_viewport
					.inverse()
					.transform_point2(viewport_location);

				if self.wire_in_progress_from_connector.is_some() && self.context_menu.is_none() {
					let to_connector = network_interface.input_connector_from_click(ipp.mouse.position, selection_network_path);
					if let Some(to_connector) = &to_connector {
						let Some(input_position) = network_interface.input_position(to_connector, selection_network_path) else {
							log::error!("Could not get input position for connector: {to_connector:?}");
							return;
						};
						self.wire_in_progress_to_connector = Some(input_position);
					}
					// Not hovering over a node input or node output, update with the mouse position.
					else {
						self.wire_in_progress_to_connector = Some(point);
						// Disconnect if the wire was previously connected to an input
						if let Some(disconnecting) = &self.disconnecting {
							let mut disconnect_root_node = false;
							if let Previewing::Yes { root_node_to_restore } = network_interface.previewing(selection_network_path) {
								if root_node_to_restore.is_some() && *disconnecting == InputConnector::Export(0) {
									disconnect_root_node = true;
								}
							}
							if disconnect_root_node {
								responses.add(NodeGraphMessage::DisconnectRootNode);
							} else {
								responses.add(NodeGraphMessage::DisconnectInput {
									input_connector: disconnecting.clone(),
								});
							}
							// Update the frontend that the node is disconnected
							responses.add(NodeGraphMessage::RunDocumentGraph);
							responses.add(NodeGraphMessage::SendGraph);
							self.disconnecting = None;
						}
					}

					if let (Some(wire_in_progress_from_connector), Some(wire_in_progress_to_connector)) = (self.wire_in_progress_from_connector, self.wire_in_progress_to_connector) {
						// If performance is a concern this can be stored as a field in the wire_in_progress_from/to_connector struct, and updated when snapping to an output
						let Some(network_metadata) = network_interface.network_metadata(selection_network_path) else {
							return;
						};
						let from_connector_viewport = network_metadata
							.persistent_metadata
							.navigation_metadata
							.node_graph_to_viewport
							.transform_point2(wire_in_progress_from_connector);
						let from_connector_is_layer = network_interface
							.output_connector_from_click(from_connector_viewport, selection_network_path)
							.is_some_and(|output_connector| {
								if let OutputConnector::Node { node_id, .. } = output_connector {
									network_interface.is_layer(&node_id, selection_network_path)
								} else {
									false
								}
							});
						let to_connector_is_layer = to_connector.is_some_and(|to_connector| {
							if let InputConnector::Node { node_id, input_index } = to_connector {
								input_index == 0 && network_interface.is_layer(&node_id, selection_network_path)
							} else {
								false
							}
						});
						let wire_path = WirePath {
							path_string: Self::build_wire_path_string(wire_in_progress_from_connector, wire_in_progress_to_connector, from_connector_is_layer, to_connector_is_layer),
							data_type: FrontendGraphDataType::General,
							thick: false,
							dashed: false,
						};
						responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: Some(wire_path) });
					}
				} else if let Some(drag_start) = &mut self.drag_start {
					if self.begin_dragging {
						self.begin_dragging = false;
						if ipp.keyboard.get(crate::messages::tool::tool_messages::tool_prelude::Key::Alt as usize) {
							responses.add(NodeGraphMessage::DuplicateSelectedNodes);
							// Duplicating sets a 2x2 offset, so shift the nodes back to the original position
							responses.add(NodeGraphMessage::ShiftSelectedNodes {
								direction: Direction::Up,
								rubber_band: false,
							});
							responses.add(NodeGraphMessage::ShiftSelectedNodes {
								direction: Direction::Up,
								rubber_band: false,
							});
							responses.add(NodeGraphMessage::ShiftSelectedNodes {
								direction: Direction::Left,
								rubber_band: false,
							});
							responses.add(NodeGraphMessage::ShiftSelectedNodes {
								direction: Direction::Left,
								rubber_band: false,
							});
							self.preview_on_mouse_up = None;
						}
					}

					let mut graph_delta = IVec2::new(((point.x - drag_start.start_x) / 24.).round() as i32, ((point.y - drag_start.start_y) / 24.).round() as i32);
					let previous_round_x = drag_start.round_x;
					let previous_round_y = drag_start.round_y;

					drag_start.round_x = graph_delta.x;
					drag_start.round_y = graph_delta.y;

					graph_delta.x -= previous_round_x;
					graph_delta.y -= previous_round_y;

					while graph_delta != IVec2::ZERO {
						match graph_delta.x.cmp(&0) {
							Ordering::Greater => {
								responses.add(NodeGraphMessage::ShiftSelectedNodes {
									direction: Direction::Right,
									rubber_band: true,
								});
								graph_delta.x -= 1;
							}
							Ordering::Less => {
								responses.add(NodeGraphMessage::ShiftSelectedNodes {
									direction: Direction::Left,
									rubber_band: true,
								});
								graph_delta.x += 1;
							}
							Ordering::Equal => {}
						}

						match graph_delta.y.cmp(&0) {
							Ordering::Greater => {
								responses.add(NodeGraphMessage::ShiftSelectedNodes {
									direction: Direction::Down,
									rubber_band: true,
								});
								graph_delta.y -= 1;
							}
							Ordering::Less => {
								responses.add(NodeGraphMessage::ShiftSelectedNodes {
									direction: Direction::Up,
									rubber_band: true,
								});
								graph_delta.y += 1;
							}
							Ordering::Equal => {}
						}
					}
				} else if self.box_selection_start.is_some() {
					responses.add(NodeGraphMessage::UpdateBoxSelection);
				}
			}
			NodeGraphMessage::PointerUp => {
				if selection_network_path != breadcrumb_network_path {
					log::error!("Selection network path does not match breadcrumb network path in PointerUp");
					return;
				}
				let Some(selected_nodes) = network_interface.selected_nodes(selection_network_path) else {
					log::error!("Could not get selected nodes in PointerUp");
					return;
				};
				let Some(network_metadata) = network_interface.network_metadata(selection_network_path) else {
					warn!("No network_metadata");
					return;
				};

				responses.add(DocumentMessage::EndTransaction);

				if let Some(preview_node) = self.preview_on_mouse_up {
					responses.add(NodeGraphMessage::TogglePreview { node_id: preview_node });
					self.preview_on_mouse_up = None;
				}
				if let Some(node_to_deselect) = self.deselect_on_pointer_up {
					let mut new_selected_nodes = selected_nodes.selected_nodes_ref().clone();
					new_selected_nodes.remove(node_to_deselect);
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: new_selected_nodes });
					self.deselect_on_pointer_up = None;
				}
				let point = network_metadata
					.persistent_metadata
					.navigation_metadata
					.node_graph_to_viewport
					.inverse()
					.transform_point2(ipp.mouse.position);
				// Disconnect if the wire was previously connected to an input
				if let (Some(wire_in_progress_from_connector), Some(wire_in_progress_to_connector)) = (self.wire_in_progress_from_connector, self.wire_in_progress_to_connector) {
					// Check if dragged connector is reconnected to another input
					let node_graph_to_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport;
					let from_connector_viewport = node_graph_to_viewport.transform_point2(wire_in_progress_from_connector);
					let to_connector_viewport = node_graph_to_viewport.transform_point2(wire_in_progress_to_connector);
					let output_connector = network_interface.output_connector_from_click(from_connector_viewport, selection_network_path);
					let input_connector = network_interface.input_connector_from_click(to_connector_viewport, selection_network_path);

					if let (Some(output_connector), Some(input_connector)) = (&output_connector, &input_connector) {
						responses.add(NodeGraphMessage::CreateWire {
							input_connector: input_connector.clone(),
							output_connector: output_connector.clone(),
						});

						responses.add(NodeGraphMessage::RunDocumentGraph);

						responses.add(NodeGraphMessage::SendGraph);
					} else if output_connector.is_some() && input_connector.is_none() && !self.initial_disconnecting {
						// If the add node menu is already open, we don't want to open it again
						if self.context_menu.is_some() {
							return;
						}
						let Some(network_metadata) = network_interface.network_metadata(selection_network_path) else {
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
				}
				// End of dragging a node
				else if let Some(drag_start) = &self.drag_start {
					self.shift_without_push = false;
					// Reset all offsets to end the rubber banding while dragging
					network_interface.unload_stack_dependents_y_offset(selection_network_path);
					let Some(selected_nodes) = network_interface.selected_nodes(selection_network_path) else { return };
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

					// Try expand the upstream chain for all layers if there is an eligible node
					let Some(network) = network_interface.network(selection_network_path) else { return };
					for layer in network
						.nodes
						.keys()
						.filter(|node_id| network_interface.is_layer(node_id, selection_network_path))
						.cloned()
						.collect::<Vec<_>>()
					{
						network_interface.try_set_upstream_to_chain(&InputConnector::node(layer, 1), selection_network_path);
					}
					responses.add(NodeGraphMessage::SendGraph);

					let Some(selected_nodes) = network_interface.selected_nodes(selection_network_path) else { return };
					// Check if a single node was dragged onto a wire and that the node was dragged onto the wire
					if selected_nodes.selected_nodes_ref().len() == 1 && !self.begin_dragging {
						let selected_node_id = selected_nodes.selected_nodes_ref()[0];
						let has_primary_output_connection = network_interface
							.outward_wires(selection_network_path)
							.is_some_and(|outward_wires| outward_wires.get(&OutputConnector::node(selected_node_id, 0)).is_some_and(|outward_wires| !outward_wires.is_empty()));
						let Some(network) = network_interface.network(selection_network_path) else {
							return;
						};
						if let Some(selected_node) = network.nodes.get(&selected_node_id) {
							// Check if any downstream node has any input that feeds into the primary export of the selected node
							let primary_input_is_value = selected_node.inputs.first().is_some_and(|first_input| first_input.as_value().is_some());
							// Check that neither the primary input or output of the selected node are already connected.
							if !has_primary_output_connection && primary_input_is_value {
								let Some(bounding_box) = network_interface.node_bounding_box(&selected_node_id, selection_network_path) else {
									log::error!("Could not get bounding box for node: {selected_node_id}");
									return;
								};
								// TODO: Cache all wire locations if this is a performance issue
								let overlapping_wires = Self::collect_wires(network_interface, selection_network_path)
									.into_iter()
									.filter(|frontend_wire| {
										// Prevent inserting on a link that is connected upstream to the selected node
										if network_interface
											.upstream_flow_back_from_nodes(vec![selected_node_id], selection_network_path, network_interface::FlowType::UpstreamFlow)
											.any(|upstream_id| {
												frontend_wire.wire_end.node_id().is_some_and(|wire_end_id| wire_end_id == upstream_id)
													|| frontend_wire.wire_start.node_id().is_some_and(|wire_start_id| wire_start_id == upstream_id)
											}) {
											return false;
										}

										// Prevent inserting a layer into a chain
										if network_interface.is_layer(&selected_node_id, selection_network_path)
											&& frontend_wire
												.wire_start
												.node_id()
												.is_some_and(|wire_start_id| network_interface.is_chain(&wire_start_id, selection_network_path))
										{
											return false;
										}

										let Some(input_position) = network_interface.input_position(&frontend_wire.wire_end, selection_network_path) else {
											log::error!("Could not get input port position for {:?}", frontend_wire.wire_end);
											return false;
										};

										let Some(output_position) = network_interface.output_position(&frontend_wire.wire_start, selection_network_path) else {
											log::error!("Could not get output port position for {:?}", frontend_wire.wire_start);
											return false;
										};

										let start_node_is_layer = frontend_wire
											.wire_end
											.node_id()
											.is_some_and(|wire_start_id| network_interface.is_layer(&wire_start_id, selection_network_path));
										let end_node_is_layer = frontend_wire
											.wire_end
											.node_id()
											.is_some_and(|wire_end_id| network_interface.is_layer(&wire_end_id, selection_network_path));

										let locations = Self::build_wire_path_locations(output_position, input_position, start_node_is_layer, end_node_is_layer);
										let bezier = bezier_rs::Bezier::from_cubic_dvec2(
											(locations[0].x, locations[0].y).into(),
											(locations[1].x, locations[1].y).into(),
											(locations[2].x, locations[2].y).into(),
											(locations[3].x, locations[3].y).into(),
										);

										!bezier.rectangle_intersections(bounding_box[0], bounding_box[1]).is_empty() || bezier.is_contained_within(bounding_box[0], bounding_box[1])
									})
									.collect::<Vec<_>>();

								let is_stack_wire = |wire: &FrontendNodeWire| match (wire.wire_start.node_id(), wire.wire_end.node_id(), wire.wire_end.input_index()) {
									(Some(start_id), Some(end_id), input_index) => {
										input_index == 0 && network_interface.is_layer(&start_id, selection_network_path) && network_interface.is_layer(&end_id, selection_network_path)
									}
									_ => false,
								};

								// Prioritize vertical thick lines and cancel if there are multiple potential wires
								let mut node_wires = Vec::new();
								let mut stack_wires = Vec::new();
								for wire in overlapping_wires {
									if is_stack_wire(&wire) {
										stack_wires.push(wire)
									} else {
										node_wires.push(wire)
									}
								}

								// Auto convert node to layer when inserting on a single stack wire
								if stack_wires.len() == 1 && node_wires.is_empty() {
									network_interface.set_to_node_or_layer(&selected_node_id, selection_network_path, true)
								}

								let overlapping_wire = if network_interface.is_layer(&selected_node_id, selection_network_path) {
									if stack_wires.len() == 1 {
										stack_wires.first()
									} else if stack_wires.is_empty() && node_wires.len() == 1 {
										node_wires.first()
									} else {
										None
									}
								} else if node_wires.len() == 1 {
									node_wires.first()
								} else {
									None
								};
								if let Some(overlapping_wire) = overlapping_wire {
									let Some(network) = network_interface.network(selection_network_path) else {
										return;
									};
									// Ensure connection is to first visible input of selected node. If it does not have an input then do not connect
									if let Some((selected_node_input_index, _)) = network
										.nodes
										.get(&selected_node_id)
										.unwrap()
										.inputs
										.iter()
										.enumerate()
										.find(|(_, input)| input.is_exposed_to_frontend(selection_network_path.is_empty()))
									{
										responses.add(NodeGraphMessage::InsertNodeBetween {
											node_id: selected_node_id,
											input_connector: overlapping_wire.wire_end.clone(),
											insert_node_input_index: selected_node_input_index,
										});

										responses.add(NodeGraphMessage::RunDocumentGraph);

										responses.add(NodeGraphMessage::SendGraph);
									}
								}
							}
						}
					}
					self.select_if_not_dragged = None;
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
				// 					node.metadata().position.x, node.metadata().position.y, node.name
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
				responses.add(PortfolioMessage::SubmitGraphRender { document_id, ignore_hash: false });
			}
			NodeGraphMessage::ForceRunDocumentGraph => {
				responses.add(PortfolioMessage::SubmitGraphRender { document_id, ignore_hash: true });
			}
			NodeGraphMessage::SelectedNodesAdd { nodes } => {
				let Some(selected_nodes) = network_interface.selected_nodes_mut(selection_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::SelectedNodesAdd");
					return;
				};
				selected_nodes.add_selected_nodes(nodes);
				responses.add(BroadcastEvent::SelectionChanged);
			}
			NodeGraphMessage::SelectedNodesRemove { nodes } => {
				let Some(selected_nodes) = network_interface.selected_nodes_mut(selection_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::SelectedNodesRemove");
					return;
				};
				selected_nodes.retain_selected_nodes(|node| !nodes.contains(node));
				responses.add(BroadcastEvent::SelectionChanged);
			}
			NodeGraphMessage::SelectedNodesSet { nodes } => {
				let Some(selected_nodes) = network_interface.selected_nodes_mut(selection_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::SelectedNodesSet");
					return;
				};
				selected_nodes.set_selected_nodes(nodes);
				responses.add(BroadcastEvent::SelectionChanged);
				responses.add(PropertiesPanelMessage::Refresh);
			}
			NodeGraphMessage::SendClickTargets => responses.add(FrontendMessage::UpdateClickTargets {
				click_targets: Some(network_interface.collect_frontend_click_targets(breadcrumb_network_path)),
			}),
			NodeGraphMessage::EndSendClickTargets => responses.add(FrontendMessage::UpdateClickTargets { click_targets: None }),
			NodeGraphMessage::SendGraph => {
				responses.add(NodeGraphMessage::UpdateLayerPanel);
				responses.add(DocumentMessage::DocumentStructureChanged);
				responses.add(PropertiesPanelMessage::Refresh);
				if breadcrumb_network_path == selection_network_path && graph_view_overlay_open {
					// TODO: Implement culling of nodes and wires whose bounding boxes are outside of the viewport
					let wires = Self::collect_wires(network_interface, breadcrumb_network_path);
					let nodes = self.collect_nodes(network_interface, breadcrumb_network_path);
					let (layer_widths, chain_widths) = network_interface.collect_layer_widths(breadcrumb_network_path);
					let imports = network_interface.frontend_imports(breadcrumb_network_path).unwrap_or_default();
					let exports = network_interface.frontend_exports(breadcrumb_network_path).unwrap_or_default();
					responses.add(FrontendMessage::UpdateImportsExports { imports, exports });
					responses.add(FrontendMessage::UpdateNodeGraph { nodes, wires });
					responses.add(FrontendMessage::UpdateLayerWidths { layer_widths, chain_widths });
					responses.add(NodeGraphMessage::SendSelectedNodes);
				}
			}
			NodeGraphMessage::SetGridAlignedEdges => {
				if graph_view_overlay_open {
					network_interface.set_grid_aligned_edges(DVec2::new(ipp.viewport_bounds.bottom_right.x - ipp.viewport_bounds.top_left.x, 0.), breadcrumb_network_path);
					// Send the new edges to the frontend
					let imports = network_interface.frontend_imports(breadcrumb_network_path).unwrap_or_default();
					let exports = network_interface.frontend_exports(breadcrumb_network_path).unwrap_or_default();
					responses.add(FrontendMessage::UpdateImportsExports { imports, exports });
				}
			}
			NodeGraphMessage::SetInputValue { node_id, input_index, value } => {
				let input = NodeInput::value(value, false);
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, input_index),
					input,
				});
				responses.add(PropertiesPanelMessage::Refresh);
				if (!network_interface.reference(&node_id, selection_network_path).is_some_and(|reference| reference == "Imaginate") || input_index == 0)
					&& network_interface.connected_to_output(&node_id, selection_network_path)
				{
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
			}
			NodeGraphMessage::SetInput { input_connector, input } => {
				network_interface.set_input(&input_connector, input, selection_network_path);
			}
			NodeGraphMessage::ShiftSelectedNodes { direction, rubber_band } => {
				network_interface.shift_selected_nodes(direction, self.shift_without_push, selection_network_path);

				if !rubber_band {
					network_interface.unload_stack_dependents_y_offset(selection_network_path);
				}

				if graph_view_overlay_open {
					responses.add(NodeGraphMessage::SendGraph);
					responses.add(DocumentMessage::RenderRulers);
					responses.add(DocumentMessage::RenderScrollbars);
				}
			}

			NodeGraphMessage::ToggleSelectedAsLayersOrNodes => {
				let Some(selected_nodes) = network_interface.selected_nodes(selection_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::ToggleSelectedAsLayersOrNodes");
					return;
				};
				responses.add(DocumentMessage::AddTransaction);
				for node_id in selected_nodes.selected_nodes() {
					responses.add(NodeGraphMessage::SetToNodeOrLayer {
						node_id: *node_id,
						is_layer: !network_interface.is_layer(node_id, selection_network_path),
					});
				}
				if selected_nodes.selected_nodes().any(|node_id| network_interface.connected_to_output(node_id, selection_network_path)) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
			}
			NodeGraphMessage::ShiftNodePosition { node_id, x, y } => {
				network_interface.shift_absolute_node_position(&node_id, IVec2::new(x, y), selection_network_path);
			}
			NodeGraphMessage::SetToNodeOrLayer { node_id, is_layer } => {
				if is_layer && !network_interface.is_eligible_to_be_layer(&node_id, selection_network_path) {
					return;
				}
				network_interface.set_to_node_or_layer(&node_id, selection_network_path, is_layer);

				self.context_menu = None;
				responses.add(FrontendMessage::UpdateContextMenuInformation {
					context_menu_information: self.context_menu.clone(),
				});
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::SetDisplayName { node_id, alias } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetDisplayNameImpl { node_id, alias });
				// Does not add a history step if the name was not changed
				responses.add(DocumentMessage::EndTransaction);
				responses.add(DocumentMessage::RenderRulers);
				responses.add(DocumentMessage::RenderScrollbars);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::SetDisplayNameImpl { node_id, alias } => {
				network_interface.set_display_name(&node_id, alias, selection_network_path);
			}
			NodeGraphMessage::TogglePreview { node_id } => {
				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::TogglePreviewImpl { node_id });
				responses.add(NodeGraphMessage::UpdateActionButtons);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::TogglePreviewImpl { node_id } => {
				network_interface.toggle_preview(node_id, selection_network_path);
			}
			NodeGraphMessage::ToggleSelectedLocked => {
				let Some(selected_nodes) = network_interface.selected_nodes(selection_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::ToggleSelectedLocked");
					return;
				};
				let node_ids = selected_nodes.selected_nodes().cloned().collect::<Vec<_>>();

				// If any of the selected layers are locked, show them all. Otherwise, hide them all.
				let locked = !node_ids.iter().all(|node_id| network_interface.is_locked(node_id, selection_network_path));

				responses.add(DocumentMessage::AddTransaction);

				for node_id in &node_ids {
					responses.add(NodeGraphMessage::SetLocked { node_id: *node_id, locked });
				}

				responses.add(NodeGraphMessage::SetLockedOrVisibilitySideEffects { node_ids })
			}
			NodeGraphMessage::ToggleLocked { node_id } => {
				let Some(node_metadata) = network_interface.network_metadata(&[]).unwrap().persistent_metadata.node_metadata.get(&node_id) else {
					log::error!("Cannot get node {:?} in NodeGraphMessage::ToggleLocked", node_id);
					return;
				};

				let locked = !node_metadata.persistent_metadata.locked;

				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::SetLocked { node_id, locked });
				responses.add(NodeGraphMessage::SetLockedOrVisibilitySideEffects { node_ids: vec![node_id] })
			}
			NodeGraphMessage::SetLocked { node_id, locked } => {
				network_interface.set_locked(&node_id, selection_network_path, locked);
			}
			NodeGraphMessage::ToggleSelectedVisibility => {
				let Some(network) = network_interface.network(selection_network_path) else {
					return;
				};
				let Some(selected_nodes) = network_interface.selected_nodes(selection_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::ToggleSelectedLocked");
					return;
				};

				let node_ids = selected_nodes.selected_nodes().cloned().collect::<Vec<_>>();

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let visible = !node_ids.iter().all(|node_id| network.nodes.get(node_id).is_some_and(|node| node.visible));

				responses.add(DocumentMessage::AddTransaction);
				for node_id in &node_ids {
					responses.add(NodeGraphMessage::SetVisibility { node_id: *node_id, visible });
				}
				responses.add(NodeGraphMessage::SetLockedOrVisibilitySideEffects { node_ids });
			}
			NodeGraphMessage::ToggleVisibility { node_id } => {
				let Some(network) = network_interface.network(selection_network_path) else {
					return;
				};

				let Some(node) = network.nodes.get(&node_id) else {
					log::error!("Cannot get node {node_id} in NodeGraphMessage::ToggleVisibility");
					return;
				};

				let visible = !node.visible;

				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::SetVisibility { node_id, visible });
				responses.add(NodeGraphMessage::SetLockedOrVisibilitySideEffects { node_ids: vec![node_id] });
			}
			NodeGraphMessage::SetVisibility { node_id, visible } => {
				network_interface.set_visibility(&node_id, selection_network_path, visible);
			}
			NodeGraphMessage::SetLockedOrVisibilitySideEffects { node_ids } => {
				if node_ids.iter().any(|node_id| network_interface.connected_to_output(node_id, selection_network_path)) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(NodeGraphMessage::UpdateActionButtons);
				responses.add(NodeGraphMessage::SendGraph);

				responses.add(PropertiesPanelMessage::Refresh);
			}
			NodeGraphMessage::UpdateBoxSelection => {
				if let Some(box_selection_start) = self.box_selection_start {
					// The mouse button was released but we missed the pointer up event
					// if ((e.buttons & 1) === 0) {
					// 	completeBoxSelection();
					// 	boxSelection = undefined;
					// } else if ((e.buttons & 2) !== 0) {
					// 	editor.handle.selectNodes(new BigUint64Array(previousSelection));
					// 	boxSelection = undefined;
					// }

					let Some(network_metadata) = network_interface.network_metadata(selection_network_path) else {
						log::error!("Could not get network metadata in PointerMove");
						return;
					};

					let box_selection_start_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.transform_point2(box_selection_start);

					let box_selection = Some(BoxSelection {
						start_x: box_selection_start_viewport.x.max(0.) as u32,
						start_y: box_selection_start_viewport.y.max(0.) as u32,
						end_x: ipp.mouse.position.x.max(0.) as u32,
						end_y: ipp.mouse.position.y.max(0.) as u32,
					});
					let box_selection_end_graph = network_metadata
						.persistent_metadata
						.navigation_metadata
						.node_graph_to_viewport
						.inverse()
						.transform_point2(ipp.mouse.position);

					let shift = ipp.keyboard.get(crate::messages::tool::tool_messages::tool_prelude::Key::Shift as usize);
					let Some(selected_nodes) = network_interface.selected_nodes(selection_network_path) else {
						log::error!("Could not get selected nodes in PointerMove");
						return;
					};
					let previous_selection = selected_nodes.selected_nodes_ref().iter().cloned().collect::<HashSet<_>>();
					let mut nodes = if shift {
						selected_nodes.selected_nodes_ref().iter().cloned().collect::<HashSet<_>>()
					} else {
						HashSet::new()
					};
					let all_nodes = network_metadata.persistent_metadata.node_metadata.keys().cloned().collect::<Vec<_>>();
					for node_id in all_nodes {
						let Some(click_targets) = network_interface.node_click_targets(&node_id, selection_network_path) else {
							log::error!("Could not get transient metadata for node {node_id}");
							continue;
						};
						if click_targets
							.node_click_target
							.intersect_rectangle(Quad::from_box([box_selection_start, box_selection_end_graph]), DAffine2::IDENTITY)
						{
							nodes.insert(node_id);
						}
					}
					if nodes != previous_selection {
						responses.add(NodeGraphMessage::SelectedNodesSet {
							nodes: nodes.into_iter().collect::<Vec<_>>(),
						});
					}
					responses.add(FrontendMessage::UpdateBox { box_selection })
				}
			}
			NodeGraphMessage::UpdateLayerPanel => {
				Self::update_layer_panel(network_interface, selection_network_path, collapsed, responses);
			}
			NodeGraphMessage::UpdateEdges => {
				// Update the import/export UI edges whenever the PTZ changes or the bounding box of all nodes changes
			}
			NodeGraphMessage::UpdateNewNodeGraph => {
				let Some(selected_nodes) = network_interface.selected_nodes_mut(selection_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::UpdateNewNodeGraph");
					return;
				};
				selected_nodes.clear_selected_nodes();
				responses.add(BroadcastEvent::SelectionChanged);

				responses.add(NodeGraphMessage::SendGraph);

				let node_types = document_node_definitions::collect_node_types();
				responses.add(FrontendMessage::UpdateNodeTypes { node_types });
			}
			NodeGraphMessage::UpdateTypes { resolved_types, node_graph_errors } => {
				for (path, node_type) in resolved_types.add {
					network_interface.resolved_types.types.insert(path.to_vec(), node_type);
				}
				for path in resolved_types.remove {
					network_interface.resolved_types.types.remove(&path.to_vec());
				}
				self.node_graph_errors = node_graph_errors;
			}
			NodeGraphMessage::UpdateActionButtons => {
				if selection_network_path == breadcrumb_network_path {
					self.update_selection_action_buttons(network_interface, breadcrumb_network_path, responses);
				}
			}
			NodeGraphMessage::UpdateInSelectedNetwork => responses.add(FrontendMessage::UpdateInSelectedNetwork {
				in_selected_network: selection_network_path == breadcrumb_network_path,
			}),
			NodeGraphMessage::SendSelectedNodes => {
				let Some(selected_nodes) = network_interface.selected_nodes(breadcrumb_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::SendSelectedNodes");
					return;
				};
				responses.add(NodeGraphMessage::UpdateActionButtons);
				responses.add(FrontendMessage::UpdateNodeGraphSelection {
					selected: selected_nodes.selected_nodes().cloned().collect::<Vec<_>>(),
				});
			}
		}
		let Some(selected_nodes) = network_interface.selected_nodes(selection_network_path) else {
			log::error!("Could not get selected nodes in NodeGraphMessageHandler");
			return;
		};
		self.has_selection = selected_nodes.has_selected_nodes();
	}

	fn actions(&self) -> ActionList {
		let common = vec![];
		common
	}
}

impl NodeGraphMessageHandler {
	/// Similar to [`NodeGraphMessageHandler::actions`], but this provides additional actions if the node graph is open and should only be called in that circumstance.
	pub fn actions_additional_if_node_graph_is_open(&self) -> ActionList {
		let mut common = actions!(NodeGraphMessageDiscriminant; EnterNestedNetwork, PointerDown, PointerMove, PointerUp, SendClickTargets, EndSendClickTargets);

		if self.has_selection {
			common.extend(actions!(NodeGraphMessageDiscriminant;
				Copy,
				Cut,
				DeleteSelectedNodes,
				DuplicateSelectedNodes,
				ToggleSelectedAsLayersOrNodes,
				ToggleSelectedLocked,
				ToggleSelectedVisibility,
				PrintSelectedNodeCoordinates,
				ShiftSelectedNodes,
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
	fn update_selection_action_buttons(&mut self, network_interface: &mut NodeNetworkInterface, breadcrumb_network_path: &[NodeId], responses: &mut VecDeque<Message>) {
		let Some(subgraph_path_names) = Self::collect_subgraph_names(network_interface, breadcrumb_network_path) else {
			// If a node in a nested network could not be found, exit the nested network
			let breadcrumb_network_path_len = breadcrumb_network_path.len();
			if breadcrumb_network_path_len > 0 {
				responses.add(DocumentMessage::ExitNestedNetwork {
					steps_back: breadcrumb_network_path_len,
				});
			}
			return;
		};

		let Some(network) = network_interface.network(breadcrumb_network_path) else {
			warn!("No network in update_selection_action_buttons");
			return;
		};

		let Some(selected_nodes) = network_interface.selected_nodes(breadcrumb_network_path) else {
			warn!("No selected nodes in update_selection_action_buttons");
			return;
		};

		let subgraph_path_names_length = subgraph_path_names.len();

		let breadcrumb_trail = BreadcrumbTrailButtons::new(subgraph_path_names).on_update(move |index| {
			DocumentMessage::ExitNestedNetwork {
				steps_back: subgraph_path_names_length - (*index as usize) - 1,
			}
			.into()
		});

		let mut widgets = if subgraph_path_names_length >= 2 {
			vec![breadcrumb_trail.widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()]
		} else {
			Vec::new()
		};

		let mut selection = selected_nodes.selected_nodes();

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
			let is_previewing = matches!(network_interface.previewing(breadcrumb_network_path), Previewing::Yes { .. });

			let output_button = TextButton::new(if is_output && is_previewing { "End Preview" } else { "Preview" })
				.icon(Some("Rescale".to_string()))
				.tooltip(if is_output { "Restore preview to the graph output" } else { "Preview selected node/layer" }.to_string() + " (Shortcut: Alt-click node/layer)")
				.on_update(move |_| NodeGraphMessage::TogglePreview { node_id }.into())
				.widget_holder();
			widgets.push(output_button);
		}

		self.widgets[0] = LayoutGroup::Row { widgets };
		self.send_node_bar_layout(responses);
	}

	/// Collate the properties panel sections for a node graph
	pub fn collate_properties(context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
		// If the selected nodes are in the document network, use the document network. Otherwise, use the nested network
		let Some(network) = context.network_interface.network(context.selection_network_path) else {
			warn!("No network in collate_properties");
			return Vec::new();
		};
		let Some(selected_nodes) = context.network_interface.selected_nodes(context.selection_network_path) else {
			warn!("No selected nodes in collate_properties");
			return Vec::new();
		};
		// We want:
		// - If only nodes (no layers) are selected: display each node's properties
		// - If one layer is selected, and zero or more of its upstream nodes: display the properties for the layer and its upstream nodes
		// - If multiple layers are selected, or one node plus other non-upstream nodes: display nothing

		// First, we filter all the selections into layers and nodes
		let (mut layers, mut nodes) = (Vec::new(), Vec::new());
		for node_id in selected_nodes.selected_nodes() {
			if context.network_interface.is_layer(node_id, context.selection_network_path) {
				layers.push(*node_id);
			} else {
				nodes.push(*node_id);
			}
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
				let nodes_not_upstream_of_layer = nodes.into_iter().filter(|&selected_node_id| {
					!context
						.network_interface
						.is_node_upstream_of_another_by_horizontal_flow(layers[0], context.selection_network_path, selected_node_id)
				});
				if nodes_not_upstream_of_layer.count() > 0 {
					return Vec::new();
				}

				// Iterate through all the upstream nodes, but stop when we reach another layer (since that's a point where we switch from horizontal to vertical flow)
				context
					.network_interface
					.upstream_flow_back_from_nodes(vec![layers[0]], context.selection_network_path, network_interface::FlowType::HorizontalFlow)
					.enumerate()
					.take_while(|(i, node_id)| {
						if *i == 0 {
							true
						} else {
							!context.network_interface.is_layer(node_id, context.selection_network_path)
						}
					})
					.filter_map(|(_, node_id)| network.nodes.get(&node_id).map(|node| (node, node_id)))
					.map(|(node, node_id)| node_properties::generate_node_properties(node, node_id, context))
					.collect()
			}
			// If multiple layers and/or nodes are selected, show nothing
			_ => Vec::new(),
		}
	}

	fn collect_wires(network_interface: &NodeNetworkInterface, breadcrumb_network_path: &[NodeId]) -> Vec<FrontendNodeWire> {
		let Some(network) = network_interface.network(breadcrumb_network_path) else {
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
						wire_start: OutputConnector::node(wire_start, wire_start_output_index),
						wire_end: InputConnector::node(wire_end, wire_end_input_index),
						dashed: false,
					})
				} else if let NodeInput::Network { import_index, .. } = *input {
					Some(FrontendNodeWire {
						wire_start: OutputConnector::Import(import_index),
						wire_end: InputConnector::node(wire_end, wire_end_input_index),
						dashed: false,
					})
				} else {
					None
				}
			})
			.collect::<Vec<_>>();

		// Connect primary export to root node, since previewing a node will change the primary export
		if let Some(root_node) = network_interface.root_node(breadcrumb_network_path) {
			wires.push(FrontendNodeWire {
				wire_start: OutputConnector::node(root_node.node_id, root_node.output_index),
				wire_end: InputConnector::Export(0),
				dashed: false,
			});
		}

		// Connect rest of exports to their actual export field since they are not affected by previewing. Only connect the primary export if it is dashed
		for (i, export) in network.exports.iter().enumerate() {
			let dashed = matches!(network_interface.previewing(breadcrumb_network_path), Previewing::Yes { .. }) && i == 0;
			if dashed || i != 0 {
				if let NodeInput::Node { node_id, output_index, .. } = export {
					wires.push(FrontendNodeWire {
						wire_start: OutputConnector::Node {
							node_id: *node_id,
							output_index: *output_index,
						},
						wire_end: InputConnector::Export(i),
						dashed,
					});
				} else if let NodeInput::Network { import_index, .. } = *export {
					wires.push(FrontendNodeWire {
						wire_start: OutputConnector::Import(import_index),
						wire_end: InputConnector::Export(i),
						dashed,
					})
				}
			}
		}
		wires
	}

	fn collect_nodes(&self, network_interface: &mut NodeNetworkInterface, breadcrumb_network_path: &[NodeId]) -> Vec<FrontendNode> {
		let Some(outward_wires) = network_interface.outward_wires(breadcrumb_network_path).cloned() else {
			return Vec::new();
		};
		let mut can_be_layer_lookup = HashSet::new();
		let mut position_lookup = HashMap::new();
		let Some(network) = network_interface.network(breadcrumb_network_path) else {
			log::error!("Could not get nested network when collecting nodes");
			return Vec::new();
		};
		for node_id in network.nodes.keys().cloned().collect::<Vec<_>>() {
			if network_interface.is_eligible_to_be_layer(&node_id, breadcrumb_network_path) {
				can_be_layer_lookup.insert(node_id);
			}
			if let Some(position) = network_interface.position(&node_id, breadcrumb_network_path) {
				position_lookup.insert(node_id, position);
			} else {
				log::error!("Could not get position for node {node_id}");
			}
		}
		let Some(network) = network_interface.network(breadcrumb_network_path) else {
			log::error!("Could not get nested network when collecting nodes");
			return Vec::new();
		};
		let Some(network_metadata) = network_interface.network_metadata(breadcrumb_network_path) else {
			log::error!("Could not get network_metadata when collecting nodes");
			return Vec::new();
		};

		let mut nodes = Vec::new();
		for (&node_id, node) in &network.nodes {
			let node_id_path = &[breadcrumb_network_path, (&[node_id])].concat();
			let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(&node_id) else {
				log::error!("Could not get node_metadata for {node_id_path:?}");
				continue;
			};

			let frontend_graph_inputs = node.inputs.iter().enumerate().map(|(index, _)| {
				// Convert the index in all inputs to the index in only the exposed inputs
				// TODO: Only display input type if potential inputs in node_registry are all the same type
				let node_type = network_interface.input_type(&InputConnector::node(node_id, index), breadcrumb_network_path);
				// TODO: Should display the color of the "most commonly relevant" (we'd need some sort of precedence) data type it allows given the current generic form that's constrained by the other present connections.
				let data_type = FrontendGraphDataType::with_type(&node_type);

				let input_name = node_metadata
					.persistent_metadata
					.input_names
					.get(index)
					.cloned()
					.unwrap_or(network_interface.input_type(&InputConnector::node(node_id, index), breadcrumb_network_path).nested_type().to_string());

				FrontendGraphInput {
					data_type,
					name: input_name,
					resolved_type: Some(format!("{:?}", node_type)),
					connected_to: None,
				}
			});

			let mut inputs = node.inputs.iter().zip(frontend_graph_inputs).map(|(node_input, mut frontend_graph_input)| {
				if let NodeInput::Node {
					node_id: connected_node_id,
					output_index,
					..
				} = node_input
				{
					frontend_graph_input.connected_to = Some(OutputConnector::node(*connected_node_id, *output_index));
				} else if let NodeInput::Network { import_index, .. } = node_input {
					frontend_graph_input.connected_to = Some(OutputConnector::Import(*import_index));
				}
				(node_input, frontend_graph_input)
			});

			let primary_input = inputs
				.next()
				.filter(|(input, _)| {
					// Don't show EditorApi input to nodes like "Text" in the document network
					input.is_exposed_to_frontend(breadcrumb_network_path.is_empty())
				})
				.map(|(_, input_type)| input_type);
			let exposed_inputs = inputs
				.filter(|(input, _)| input.is_exposed_to_frontend(breadcrumb_network_path.is_empty()))
				.map(|(_, input_type)| input_type)
				.collect();

			let output_types = network_interface.output_types(&node_id, breadcrumb_network_path);
			let primary_output_type = output_types.first().expect("Primary output should always exist");
			let frontend_data_type = if let Some(output_type) = primary_output_type {
				FrontendGraphDataType::with_type(output_type)
			} else {
				FrontendGraphDataType::General
			};
			let connected_to = outward_wires.get(&OutputConnector::node(node_id, 0)).cloned().unwrap_or_default();
			let primary_output = if network_interface.has_primary_output(&node_id, breadcrumb_network_path) {
				Some(FrontendGraphOutput {
					data_type: frontend_data_type,
					name: "Output 1".to_string(),
					resolved_type: primary_output_type.clone().map(|input| format!("{input:?}")),
					connected_to,
				})
			} else {
				None
			};

			let mut exposed_outputs = Vec::new();
			for (index, exposed_output) in output_types.iter().enumerate() {
				if index == 0 && network_interface.has_primary_output(&node_id, breadcrumb_network_path) {
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

				let connected_to = outward_wires.get(&OutputConnector::node(node_id, index)).cloned().unwrap_or_default();
				exposed_outputs.push(FrontendGraphOutput {
					data_type: frontend_data_type,
					name: output_name,
					resolved_type: exposed_output.clone().map(|input| format!("{input:?}")),
					connected_to,
				});
			}
			let Some(network) = network_interface.network(breadcrumb_network_path) else {
				log::error!("Could not get nested network when collecting nodes");
				return Vec::new();
			};
			let is_export = network.exports.first().is_some_and(|export| export.as_node().is_some_and(|export_node_id| node_id == export_node_id));
			let is_root_node = network_interface.root_node(breadcrumb_network_path).is_some_and(|root_node| root_node.node_id == node_id);

			let Some(position) = position_lookup.get(&node_id).map(|pos| (pos.x, pos.y)) else {
				log::error!("Could not get position for node: {node_id}");
				continue;
			};
			let previewed = is_export && !is_root_node;

			let locked = network_interface.is_locked(&node_id, breadcrumb_network_path);

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
				is_layer: network_interface
					.node_metadata(&node_id, breadcrumb_network_path)
					.is_some_and(|node_metadata| node_metadata.persistent_metadata.is_layer()),
				can_be_layer: can_be_layer_lookup.contains(&node_id),
				reference: None,
				display_name: network_interface.frontend_display_name(&node_id, breadcrumb_network_path),
				primary_input,
				exposed_inputs,
				primary_output,
				exposed_outputs,
				position,
				previewed,
				visible: node.visible,
				locked,
				errors,
				ui_only: false,
			});
		}

		nodes
	}

	fn collect_subgraph_names(network_interface: &mut NodeNetworkInterface, breadcrumb_network_path: &[NodeId]) -> Option<Vec<String>> {
		let mut current_network_path = vec![];
		let mut current_network = network_interface.network(&current_network_path).unwrap();
		let mut subgraph_names = vec!["Document".to_string()];
		for node_id in breadcrumb_network_path {
			if let Some(node) = current_network.nodes.get(node_id) {
				if let Some(network) = node.implementation.get_network() {
					current_network = network;
				};
				subgraph_names.push(network_interface.frontend_display_name(node_id, &current_network_path));
				current_network_path.push(*node_id)
			} else {
				// Could not get node in network in breadcrumb_network_path
				return None;
			};
		}
		Some(subgraph_names)
	}

	fn update_layer_panel(network_interface: &NodeNetworkInterface, selection_network_path: &[NodeId], collapsed: &CollapsedLayers, responses: &mut VecDeque<Message>) {
		let Some(selected_nodes) = network_interface.selected_nodes(&[]) else {
			log::error!("Could not get selected layers in update_layer_panel");
			return;
		};
		let selected_layers = selected_nodes
			.selected_layers(network_interface.document_metadata())
			.map(|layer| layer.to_node())
			.collect::<HashSet<_>>();

		let mut selected_parents = HashSet::new();
		for selected_layer in &selected_layers {
			for ancestor in LayerNodeIdentifier::new(*selected_layer, network_interface, &[]).ancestors(network_interface.document_metadata()) {
				if ancestor != LayerNodeIdentifier::ROOT_PARENT && !selected_layers.contains(&ancestor.to_node()) {
					selected_parents.insert(ancestor.to_node());
				}
			}
		}

		for (&node_id, node_metadata) in &network_interface.network_metadata(&[]).unwrap().persistent_metadata.node_metadata {
			if node_metadata.persistent_metadata.is_layer() {
				let layer = LayerNodeIdentifier::new(node_id, network_interface, &[]);

				let children_allowed =
						// The layer has other layers as children along the secondary input's horizontal flow
						layer.has_children(network_interface.document_metadata())
						|| (
							// Check if the last node in the chain has an exposed left input
							network_interface.upstream_flow_back_from_nodes(vec![node_id], &[], network_interface::FlowType::HorizontalFlow).last().is_some_and(|node_id|
								network_interface.network(&[]).unwrap().nodes.get(&node_id).map_or_else(||{log::error!("Could not get node {node_id} in update_layer_panel"); false}, |node| {
									if network_interface.is_layer(&node_id, &[]) {
										node.inputs.iter().filter(|input| input.is_exposed_to_frontend(true)).nth(1).is_some_and(|input| input.as_value().is_some())
									} else {
										node.inputs.iter().filter(|input| input.is_exposed_to_frontend(true)).nth(0).is_some_and(|input| input.as_value().is_some())
									}
								}))
						);

				let parents_visible = layer.ancestors(network_interface.document_metadata()).filter(|&ancestor| ancestor != layer).all(|layer| {
					if layer != LayerNodeIdentifier::ROOT_PARENT {
						network_interface.network(&[]).unwrap().nodes.get(&layer.to_node()).map(|node| node.visible).unwrap_or_default()
					} else {
						true
					}
				});

				let parents_unlocked: bool = layer.ancestors(network_interface.document_metadata()).filter(|&ancestor| ancestor != layer).all(|layer| {
					if layer != LayerNodeIdentifier::ROOT_PARENT {
						!network_interface.is_locked(&layer.to_node(), &[])
					} else {
						true
					}
				});

				let is_selected_parent = selected_parents.contains(&node_id);

				let data = LayerPanelEntry {
					id: node_id,
					children_allowed,
					children_present: layer.has_children(network_interface.document_metadata()),
					expanded: layer.has_children(network_interface.document_metadata()) && !collapsed.0.contains(&layer),
					depth: layer.ancestors(network_interface.document_metadata()).count() - 1,
					parent_id: layer
						.parent(network_interface.document_metadata())
						.and_then(|parent| if parent != LayerNodeIdentifier::ROOT_PARENT { Some(parent.to_node()) } else { None }),
					alias: network_interface.frontend_display_name(&node_id, &[]),
					tooltip: if cfg!(debug_assertions) { format!("Layer ID: {node_id}") } else { "".into() },
					visible: network_interface.is_visible(&node_id, &[]),
					parents_visible,
					unlocked: !network_interface.is_locked(&node_id, &[]),
					parents_unlocked,
					selected: selected_layers.contains(&node_id) || is_selected_parent,
					in_selected_network: selection_network_path.is_empty(),
					selected_parent: is_selected_parent,
				};
				responses.add(FrontendMessage::UpdateDocumentLayerDetails { data });
			}
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
		// TODO: Finish this commented out code replacement for the code below it based on this diagram: <https://files.keavon.com/-/SuperbWideFoxterrier/capture.png>
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
			node_graph_errors: Vec::new(),
			has_selection: false,
			widgets: [LayoutGroup::Row { widgets: Vec::new() }, LayoutGroup::Row { widgets: right_side_widgets }],
			drag_start: None,
			begin_dragging: false,
			shift_without_push: false,
			box_selection_start: None,
			disconnecting: None,
			initial_disconnecting: false,
			select_if_not_dragged: None,
			wire_in_progress_from_connector: None,
			wire_in_progress_to_connector: None,
			context_menu: None,
			deselect_on_pointer_up: None,
			auto_panning: Default::default(),
			preview_on_mouse_up: None,
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
			&& self.initial_disconnecting == other.initial_disconnecting
			&& self.select_if_not_dragged == other.select_if_not_dragged
			&& self.wire_in_progress_from_connector == other.wire_in_progress_from_connector
			&& self.wire_in_progress_to_connector == other.wire_in_progress_to_connector
			&& self.context_menu == other.context_menu
	}
}
