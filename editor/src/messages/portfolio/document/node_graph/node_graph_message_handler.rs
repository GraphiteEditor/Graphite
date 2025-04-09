use super::utility_types::{BoxSelection, ContextMenuInformation, DragStart, FrontendGraphInput, FrontendGraphOutput, FrontendNode, FrontendNodeWire, WirePath};
use super::{document_node_definitions, node_properties};
use crate::consts::GRID_SIZE;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::document_message_handler::navigation_controls;
use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::node_graph::document_node_definitions::NodePropertiesContext;
use crate::messages::portfolio::document::node_graph::utility_types::{ContextMenuData, Direction, FrontendGraphDataType};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::GroupFolderType;
use crate::messages::portfolio::document::utility_types::network_interface::{
	self, InputConnector, NodeNetworkInterface, NodeTemplate, NodeTypePersistentMetadata, OutputConnector, Previewing, TypeSource,
};
use crate::messages::portfolio::document::utility_types::nodes::{CollapsedLayers, LayerPanelEntry};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::tool_messages::tool_prelude::{Key, MouseMotion};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::document::{DocumentNodeImplementation, NodeId, NodeInput};
use graph_craft::proto::GraphErrors;
use graphene_core::*;
use renderer::Quad;
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
	pub graph_fade_artwork_percentage: f64,
	pub navigation_handler: &'a NavigationMessageHandler,
	pub preferences: &'a PreferencesMessageHandler,
}

#[derive(Debug, Clone)]
pub struct NodeGraphMessageHandler {
	// TODO: Remove network and move to NodeNetworkInterface
	pub network: Vec<NodeId>,
	pub node_graph_errors: GraphErrors,
	has_selection: bool,
	widgets: [LayoutGroup; 2],
	/// Used to add a transaction for the first node move when dragging.
	begin_dragging: bool,
	/// Used to prevent entering a nested network if the node is dragged after double clicking
	node_has_moved_in_drag: bool,
	/// If dragging the selected nodes, this stores the starting position both in viewport and node graph coordinates,
	/// plus a flag indicating if it has been dragged since the mousedown began.
	pub drag_start: Option<(DragStart, bool)>,
	/// If dragging the background to create a box selection, this stores its starting point in node graph coordinates,
	/// plus a flag indicating if it has been dragged since the mousedown began.
	box_selection_start: Option<(DVec2, bool)>,
	/// Restore the selection before box selection if it is aborted
	selection_before_pointer_down: Vec<NodeId>,
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
	// The index of the import that is being moved
	reordering_import: Option<usize>,
	// The index of the export that is being moved
	reordering_export: Option<usize>,
	// The end index of the moved port
	end_index: Option<usize>,
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
			ipp,
			graph_view_overlay_open,
			graph_fade_artwork_percentage,
			navigation_handler,
			preferences,
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
			NodeGraphMessage::AddImport => {
				network_interface.add_import(graph_craft::document::value::TaggedValue::None, true, -1, "", breadcrumb_network_path);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::AddExport => {
				network_interface.add_export(graph_craft::document::value::TaggedValue::None, -1, "", breadcrumb_network_path);
				responses.add(NodeGraphMessage::SendGraph);
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
				let selected_layers = network_interface.selected_nodes().selected_layers(network_interface.document_metadata()).collect::<Vec<_>>();
				if selected_layers.len() <= 1 {
					responses.add(DocumentMessage::SetRangeSelectionLayer {
						new_layer: selected_layers.first().cloned(),
					});
				}
				responses.add(MenuBarMessage::SendLayout);
				responses.add(NodeGraphMessage::UpdateLayerPanel);
				responses.add(NodeGraphMessage::SendSelectedNodes);
				responses.add(ArtboardToolMessage::UpdateSelectedArtboard);
				responses.add(DocumentMessage::DocumentStructureChanged);
				responses.add(OverlaysMessage::Draw);
				responses.add(NodeGraphMessage::SendGraph);
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
			NodeGraphMessage::CreateNodeInLayerNoTransaction { node_type, layer } => {
				let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, network_interface, responses) else {
					return;
				};
				modify_inputs.create_node(&node_type);
			}
			NodeGraphMessage::CreateNodeInLayerWithTransaction { node_type, layer } => {
				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::CreateNodeInLayerNoTransaction { node_type, layer });
				responses.add(PropertiesPanelMessage::Refresh);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::CreateNodeFromContextMenu { node_id, node_type, xy } => {
				let (x, y) = if let Some((x, y)) = xy {
					(x, y)
				} else if let Some(node_graph_ptz) = network_interface.node_graph_ptz(breadcrumb_network_path) {
					((-node_graph_ptz.pan.x / GRID_SIZE as f64) as i32, (-node_graph_ptz.pan.y / GRID_SIZE as f64) as i32)
				} else {
					(0, 0)
				};

				let node_id = node_id.unwrap_or_else(NodeId::new);

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
							output_connector: *output_connector,
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
			NodeGraphMessage::ConnectUpstreamOutputToInput { downstream_input, input_connector } => {
				let Some(upstream_node) = network_interface.upstream_output_connector(&downstream_input, selection_network_path) else {
					log::error!("Failed to find upstream node for downstream_input");
					return;
				};
				responses.add(NodeGraphMessage::CreateWire {
					output_connector: upstream_node,
					input_connector,
				});
			}
			NodeGraphMessage::Cut => {
				responses.add(NodeGraphMessage::Copy);
				responses.add(NodeGraphMessage::DeleteSelectedNodes { delete_children: true });
			}
			NodeGraphMessage::DeleteNodes { node_ids, delete_children } => {
				network_interface.delete_nodes(node_ids, delete_children, selection_network_path);
			}
			// Deletes selected_nodes. If `reconnect` is true, then all children nodes (secondary input) of the selected nodes are deleted and the siblings (primary input/output) are reconnected.
			// If `reconnect` is false, then only the selected nodes are deleted and not reconnected.
			NodeGraphMessage::DeleteSelectedNodes { delete_children } => {
				let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
					log::error!("Could not get selected nodes in DeleteSelectedNodes");
					return;
				};
				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::DeleteNodes {
					node_ids: selected_nodes.selected_nodes().cloned().collect::<Vec<_>>(),
					delete_children,
				});
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SelectedNodesUpdated);
				responses.add(NodeGraphMessage::SendGraph);
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

				let new_ids = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect::<HashMap<_, _>>();
				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::AddNodes { nodes, new_ids: new_ids.clone() });
				responses.add(NodeGraphMessage::SelectedNodesSet {
					nodes: new_ids.values().cloned().collect(),
				});
			}
			NodeGraphMessage::EnterNestedNetwork => {
				// Do not enter the nested network if the node was dragged
				if self.node_has_moved_in_drag {
					return;
				}

				let Some(network_metadata) = network_interface.network_metadata(selection_network_path) else {
					log::error!("Could not get network metadata in NodeGraphMessage::EnterNestedNetwork");
					return;
				};

				let click = ipp.mouse.position;
				let node_graph_point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);

				// Check if clicked on empty area (no node, no input/output connector)
				let clicked_id = network_interface.node_from_click(click, selection_network_path);
				let clicked_input = network_interface.input_connector_from_click(click, selection_network_path);
				let clicked_output = network_interface.output_connector_from_click(click, selection_network_path);

				if clicked_id.is_none() && clicked_input.is_none() && clicked_output.is_none() && self.context_menu.is_none() {
					// Create a context menu with node creation options
					self.context_menu = Some(ContextMenuInformation {
						context_menu_coordinates: (node_graph_point.x as i32, node_graph_point.y as i32),
						context_menu_data: ContextMenuData::CreateNode { compatible_type: None },
					});

					responses.add(FrontendMessage::UpdateContextMenuInformation {
						context_menu_information: self.context_menu.clone(),
					});
				}
				let Some(node_id) = network_interface.node_from_click(ipp.mouse.position, selection_network_path) else {
					return;
				};
				if network_interface
					.layer_click_target_from_click(ipp.mouse.position, network_interface::LayerClickTargetTypes::Visibility, selection_network_path)
					.is_some()
				{
					return;
				};
				if let Some(DocumentNodeImplementation::Network(_)) = network_interface.implementation(&node_id, selection_network_path) {
					responses.add(DocumentMessage::EnterNestedNetwork { node_id });
				}
			}
			NodeGraphMessage::ExposeInput { input_connector, new_exposed } => {
				let InputConnector::Node { node_id, input_index } = input_connector else {
					log::error!("Cannot expose/hide export");
					return;
				};
				let Some(node) = network_interface.document_node(&node_id, selection_network_path) else {
					log::error!("Could not find node {node_id} in NodeGraphMessage::ExposeInput");
					return;
				};
				let Some(mut input) = node.inputs.get(input_index).cloned() else {
					log::error!("Could not find input {input_index} in NodeGraphMessage::ExposeInput");
					return;
				};
				if let NodeInput::Value { exposed, .. } = &mut input {
					*exposed = new_exposed;
				} else if !new_exposed {
					// If hiding an input that is not a value, then disconnect it. This will convert it to a value input.
					responses.add(NodeGraphMessage::DisconnectInput { input_connector });
					responses.add(NodeGraphMessage::ExposeInput { input_connector, new_exposed });
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
			NodeGraphMessage::MergeSelectedNodes => {
				let new_ids = network_interface
					.selected_nodes_in_nested_network(breadcrumb_network_path)
					.unwrap()
					.selected_nodes()
					.map(|id| (*id, *id))
					.collect::<HashMap<NodeId, NodeId>>();

				let copied_nodes = network_interface.copy_nodes(&new_ids, breadcrumb_network_path).collect::<Vec<_>>();
				let selected_node_ids = copied_nodes.iter().map(|(node_id, _)| *node_id).collect::<HashSet<_>>();
				let selected_node_ids_vec = copied_nodes.iter().map(|(node_id, _)| *node_id).collect::<Vec<_>>();
				// Mapping of the encapsulating node inputs/outputs to where it needs to be connected
				let mut input_connections = Vec::new();
				let mut output_connections = Vec::new();
				// Mapping of the inner nodes that need to be connected to the imports/exports
				let mut import_connections = Vec::new();
				let mut export_connections = Vec::new();
				// Scan current nodes top to bottom and find all inputs/outputs connected to nodes that are not in the copied nodes. These will represent the new imports and exports.
				let Some(nodes_sorted_top_to_bottom) = network_interface.nodes_sorted_top_to_bottom(
					network_interface.selected_nodes_in_nested_network(breadcrumb_network_path).unwrap().selected_nodes(),
					breadcrumb_network_path,
				) else {
					return;
				};
				// Ensure that nodes can be grouped by checking if there is an unselected node between selected nodes
				for selected_node_id in &selected_node_ids {
					for input_index in 0..network_interface.number_of_inputs(selected_node_id, breadcrumb_network_path) {
						let input_connector = InputConnector::node(*selected_node_id, input_index);
						if let Some(upstream_deselected_node_id) = network_interface
							.upstream_output_connector(&input_connector, breadcrumb_network_path)
							.and_then(|output_connector| output_connector.node_id())
							.filter(|node_id| !selected_node_ids.contains(node_id))
						{
							for upstream_node_id in
								network_interface.upstream_flow_back_from_nodes(vec![upstream_deselected_node_id], breadcrumb_network_path, network_interface::FlowType::UpstreamFlow)
							{
								if selected_node_ids.contains(&upstream_node_id) {
									responses.add(DialogMessage::DisplayDialogError {
										title: "Error Grouping Nodes".to_string(),
										description: "A discontinuous selection of nodes cannot be grouped.\nEnsure no deselected nodes are between selected nodes".to_string(),
									});
									return;
								}
							}
						}
					}
				}
				for node_id in nodes_sorted_top_to_bottom {
					for input_index in 0..network_interface.number_of_inputs(&node_id, breadcrumb_network_path) {
						let current_input_connector = InputConnector::node(node_id, input_index);
						let Some(upstream_connector) = network_interface.upstream_output_connector(&current_input_connector, breadcrumb_network_path) else {
							continue;
						};
						if upstream_connector
							.node_id()
							.is_some_and(|upstream_node_id| selected_node_ids.iter().any(|copied_id| *copied_id == upstream_node_id))
						{
							continue;
						}

						// If the upstream connection is not part of the copied nodes, then connect it to the new imports, or add it if it has not already been added.
						let import_index = input_connections.iter().position(|old_connection| old_connection == &upstream_connector).unwrap_or_else(|| {
							input_connections.push(upstream_connector);
							input_connections.len() - 1
						});
						import_connections.push((current_input_connector, import_index));
					}
					for output_index in 0..network_interface.number_of_outputs(&node_id, breadcrumb_network_path) {
						let current_output_connector = OutputConnector::node(node_id, output_index);
						let Some(outward_wires) = network_interface.outward_wires(breadcrumb_network_path) else {
							log::error!("Could not get outward wires in upstream_nodes_below_layer");
							continue;
						};
						let Some(downstream_connections) = outward_wires.get(&current_output_connector).cloned() else {
							log::error!("Could not get downstream connections for {current_output_connector:?}");
							continue;
						};

						// The output gets connected to all the previous inputs the node was connected to
						let mut connect_output_to = Vec::new();
						for downstream_connection in downstream_connections {
							if downstream_connection.node_id().is_some_and(|downstream_node_id| selected_node_ids.contains(&downstream_node_id)) {
								continue;
							}
							connect_output_to.push(downstream_connection);
						}
						if !connect_output_to.is_empty() {
							// Every output connected to some non selected node forms a new export
							export_connections.push(current_output_connector);
							output_connections.push(connect_output_to);
						}
					}
				}

				// Use the network interface to add a default node, then set the imports, exports, paste the nodes inside, and connect them to the imports/exports
				let encapsulating_node_id = NodeId::new();
				let mut default_node_template = document_node_definitions::resolve_document_node_type("Default Network")
					.expect("Default Network node should exist")
					.default_node_template();
				let Some(center_of_selected_nodes) = network_interface.selected_nodes_bounding_box(breadcrumb_network_path).map(|[a, b]| (a + b) / 2.) else {
					log::error!("Could not get center of selected_nodes");
					return;
				};
				let center_of_selected_nodes_grid_space = IVec2::new((center_of_selected_nodes.x / 24. + 0.5).floor() as i32, (center_of_selected_nodes.y / 24. + 0.5).floor() as i32);
				default_node_template.persistent_node_metadata.node_type_metadata = NodeTypePersistentMetadata::node(center_of_selected_nodes_grid_space - IVec2::new(3, 1));
				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::InsertNode {
					node_id: encapsulating_node_id,
					node_template: default_node_template,
				});
				responses.add(NodeGraphMessage::SetDisplayNameImpl {
					node_id: encapsulating_node_id,
					alias: "Untitled Node".to_string(),
				});

				responses.add(DocumentMessage::EnterNestedNetwork { node_id: encapsulating_node_id });
				for _ in 0..input_connections.len() {
					responses.add(NodeGraphMessage::AddImport);
				}
				for _ in 0..output_connections.len() {
					responses.add(NodeGraphMessage::AddExport);
				}
				responses.add(NodeGraphMessage::AddNodes { nodes: copied_nodes, new_ids });
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: selected_node_ids_vec.clone() });

				// Shift the nodes back to the origin
				responses.add(NodeGraphMessage::ShiftSelectedNodesByAmount {
					graph_delta: -center_of_selected_nodes_grid_space - IVec2::new(2, 2),
					rubber_band: false,
				});

				for (input_connector, import_index) in import_connections {
					responses.add(NodeGraphMessage::CreateWire {
						output_connector: OutputConnector::Import(import_index),
						input_connector,
					});
				}
				for (export_index, output_connector) in export_connections.into_iter().enumerate() {
					responses.add(NodeGraphMessage::CreateWire {
						output_connector,
						input_connector: InputConnector::Export(export_index),
					});
				}
				responses.add(DocumentMessage::ExitNestedNetwork { steps_back: 1 });
				for (input_index, output_connector) in input_connections.into_iter().enumerate() {
					responses.add(NodeGraphMessage::CreateWire {
						output_connector,
						input_connector: InputConnector::node(encapsulating_node_id, input_index),
					});
				}
				for (output_index, input_connectors) in output_connections.into_iter().enumerate() {
					for input_connector in input_connectors {
						responses.add(NodeGraphMessage::CreateWire {
							output_connector: OutputConnector::node(encapsulating_node_id, output_index),
							input_connector,
						});
					}
				}
				responses.add(NodeGraphMessage::DeleteNodes {
					node_ids: selected_node_ids_vec,
					delete_children: false,
				});
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![encapsulating_node_id] });
				responses.add(NodeGraphMessage::SendGraph);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index } => {
				network_interface.move_layer_to_stack(layer, parent, insert_index, selection_network_path);
			}
			NodeGraphMessage::MoveNodeToChainStart { node_id, parent } => {
				network_interface.move_node_to_chain_start(&node_id, parent, selection_network_path);
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

				let new_ids: HashMap<_, _> = data.iter().map(|(id, _)| (*id, NodeId::new())).collect();
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

				if network_interface
					.layer_click_target_from_click(click, network_interface::LayerClickTargetTypes::Grip, selection_network_path)
					.is_some()
				{
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
						self.drag_start = None;
						responses.add(DocumentMessage::AbortTransaction);
						responses.add(NodeGraphMessage::SelectedNodesSet {
							nodes: self.selection_before_pointer_down.clone(),
						});
						return;
					}
					// Abort a box selection
					if self.box_selection_start.is_some() {
						self.box_selection_start = None;
						responses.add(NodeGraphMessage::SelectedNodesSet {
							nodes: self.selection_before_pointer_down.clone(),
						});
						responses.add(FrontendMessage::UpdateBox { box_selection: None });
						return;
					}
					// Abort dragging a wire
					if self.wire_in_progress_from_connector.is_some() {
						self.wire_in_progress_from_connector = None;
						self.wire_in_progress_to_connector = None;
						responses.add(DocumentMessage::AbortTransaction);
						responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: None });
						return;
					}

					let context_menu_data = if let Some(node_id) = clicked_id {
						let currently_is_node = !network_interface.is_layer(&node_id, selection_network_path);
						ContextMenuData::ToggleLayer { node_id, currently_is_node }
					} else {
						ContextMenuData::CreateNode { compatible_type: None }
					};

					// TODO: Create function
					let node_graph_shift = if matches!(context_menu_data, ContextMenuData::CreateNode { compatible_type: None }) {
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

				let Some(modify_import_export) = network_interface.modify_import_export(selection_network_path) else {
					log::error!("Could not get modify import export in PointerDown");
					return;
				};

				if modify_import_export.add_import_export.clicked_input_port_from_point(node_graph_point).is_some() {
					responses.add(DocumentMessage::AddTransaction);
					responses.add(NodeGraphMessage::AddExport);
					return;
				} else if modify_import_export.add_import_export.clicked_output_port_from_point(node_graph_point).is_some() {
					responses.add(DocumentMessage::AddTransaction);
					responses.add(NodeGraphMessage::AddImport);
					return;
				} else if let Some(remove_import_index) = modify_import_export.remove_imports_exports.clicked_output_port_from_point(node_graph_point) {
					responses.add(DocumentMessage::AddTransaction);
					responses.add(NodeGraphMessage::RemoveImport { import_index: remove_import_index });
					return;
				} else if let Some(remove_export_index) = modify_import_export.remove_imports_exports.clicked_input_port_from_point(node_graph_point) {
					responses.add(DocumentMessage::AddTransaction);
					responses.add(NodeGraphMessage::RemoveExport { export_index: remove_export_index });
					return;
				} else if let Some(move_import_index) = modify_import_export.reorder_imports_exports.clicked_output_port_from_point(node_graph_point) {
					responses.add(DocumentMessage::StartTransaction);
					self.reordering_import = Some(move_import_index);
					return;
				} else if let Some(move_export_index) = modify_import_export.reorder_imports_exports.clicked_input_port_from_point(node_graph_point) {
					responses.add(DocumentMessage::StartTransaction);
					self.reordering_export = Some(move_export_index);
					return;
				}

				self.selection_before_pointer_down = network_interface
					.selected_nodes_in_nested_network(selection_network_path)
					.map(|selected_nodes| selected_nodes.selected_nodes().cloned().collect())
					.unwrap_or_default();

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
				if let Some(clicked_visibility) = network_interface.layer_click_target_from_click(click, network_interface::LayerClickTargetTypes::Visibility, selection_network_path) {
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
					self.disconnecting = Some(*clicked_input);

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

					self.wire_in_progress_from_connector = network_interface.output_position(&clicked_output, selection_network_path);
					self.update_node_graph_hints(responses);
					return;
				}

				if let Some(clicked_id) = clicked_id {
					let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
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

						self.drag_start = Some((drag_start, false));
						self.begin_dragging = true;
						self.node_has_moved_in_drag = false;
						self.update_node_graph_hints(responses);
					}

					// Update the selection if it was modified
					if modified_selected {
						responses.add(NodeGraphMessage::SelectedNodesSet { nodes: updated_selected })
					}
					// Start the transaction after setting the node, since when the transactions ends it aborts any changes after this
					responses.add(DocumentMessage::StartTransaction);

					return;
				}

				// Clicked on the graph background so we box select
				if !shift_click {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() })
				}
				self.box_selection_start = Some((node_graph_point, false));
				self.update_node_graph_hints(responses);
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
								responses.add(NodeGraphMessage::DisconnectInput { input_connector: *disconnecting });
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
				} else if let Some((drag_start, dragged)) = &mut self.drag_start {
					if drag_start.start_x != point.x || drag_start.start_y != point.y {
						*dragged = true;
					}

					self.node_has_moved_in_drag = true;
					if self.begin_dragging {
						self.begin_dragging = false;
						if ipp.keyboard.get(Key::Alt as usize) {
							responses.add(NodeGraphMessage::DuplicateSelectedNodes);
							// Duplicating sets a 2x2 offset, so shift the nodes back to the original position
							responses.add(NodeGraphMessage::ShiftSelectedNodesByAmount {
								graph_delta: IVec2::new(-2, -2),
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

					responses.add(NodeGraphMessage::ShiftSelectedNodesByAmount { graph_delta, rubber_band: true });

					self.update_node_graph_hints(responses);
				} else if let Some((_, box_selection_dragged)) = &mut self.box_selection_start {
					*box_selection_dragged = true;
					responses.add(NodeGraphMessage::UpdateBoxSelection);
					self.update_node_graph_hints(responses);
				} else if self.reordering_import.is_some() {
					let Some(modify_import_export) = network_interface.modify_import_export(selection_network_path) else {
						log::error!("Could not get modify import export in PointerUp");
						return;
					};
					// Find the first import that is below the mouse position
					self.end_index = Some(
						modify_import_export
							.reorder_imports_exports
							.output_ports()
							.find_map(|(index, click_target)| {
								let Some(position) = click_target.bounding_box().map(|bbox| (bbox[0].y + bbox[1].y) / 2.) else {
									log::error!("Could not get bounding box for import: {index}");
									return None;
								};
								(position > point.y).then_some(*index)
							})
							.unwrap_or(modify_import_export.reorder_imports_exports.output_ports().count()),
					);
					responses.add(FrontendMessage::UpdateImportReorderIndex { index: self.end_index });
				} else if self.reordering_export.is_some() {
					let Some(modify_import_export) = network_interface.modify_import_export(selection_network_path) else {
						log::error!("Could not get modify import export in PointerUp");
						return;
					};
					// Find the first export that is below the mouse position
					self.end_index = Some(
						modify_import_export
							.reorder_imports_exports
							.input_ports()
							.find_map(|(index, click_target)| {
								let Some(position) = click_target.bounding_box().map(|bbox| (bbox[0].y + bbox[1].y) / 2.) else {
									log::error!("Could not get bounding box for export: {index}");
									return None;
								};
								(position > point.y).then_some(*index)
							})
							.unwrap_or(modify_import_export.reorder_imports_exports.input_ports().count()),
					);
					responses.add(FrontendMessage::UpdateExportReorderIndex { index: self.end_index });
				}
			}
			NodeGraphMessage::PointerUp => {
				if selection_network_path != breadcrumb_network_path {
					log::error!("Selection network path does not match breadcrumb network path in PointerUp");
					return;
				}
				let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
					log::error!("Could not get selected nodes in PointerUp");
					return;
				};
				let Some(network_metadata) = network_interface.network_metadata(selection_network_path) else {
					warn!("No network_metadata");
					return;
				};

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
							input_connector: *input_connector,
							output_connector: *output_connector,
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
						// Get the compatible type from the output connector
						let compatible_type = output_connector.and_then(|output_connector| {
							output_connector.node_id().and_then(|node_id| {
								let output_index = output_connector.index();
								// Get the output types from the network interface
								let output_types = network_interface.output_types(&node_id, selection_network_path);

								// Extract the type if available
								output_types.get(output_index).and_then(|type_option| type_option.as_ref()).map(|(output_type, _)| {
									// Create a search term based on the type
									format!("type:{}", output_type.clone().nested_type())
								})
							})
						});
						let appear_right_of_mouse = if ipp.mouse.position.x > ipp.viewport_bounds.size().x - 173. { -173. } else { 0. };
						let appear_above_mouse = if ipp.mouse.position.y > ipp.viewport_bounds.size().y - 34. { -34. } else { 0. };
						let node_graph_shift = DVec2::new(appear_right_of_mouse, appear_above_mouse) / network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.matrix2.x_axis.x;

						self.context_menu = Some(ContextMenuInformation {
							context_menu_coordinates: ((point.x + node_graph_shift.x) as i32, (point.y + node_graph_shift.y) as i32),
							context_menu_data: ContextMenuData::CreateNode { compatible_type },
						});

						responses.add(FrontendMessage::UpdateContextMenuInformation {
							context_menu_information: self.context_menu.clone(),
						});
						return;
					}
				}
				// End of dragging a node
				else if let Some((drag_start, _)) = &self.drag_start {
					self.shift_without_push = false;

					// Reset all offsets to end the rubber banding while dragging
					network_interface.unload_stack_dependents_y_offset(selection_network_path);
					let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
						log::error!("Could not get selected nodes in PointerUp");
						return;
					};

					// Only select clicked node if multiple are selected and they were not dragged
					if let Some(select_if_not_dragged) = self.select_if_not_dragged {
						let not_dragged = drag_start.start_x == point.x && drag_start.start_y == point.y;
						if not_dragged
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
					let Some(network) = network_interface.nested_network(selection_network_path) else { return };
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

					let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
						log::error!("Could not get selected nodes in PointerUp");
						return;
					};
					// Check if a single node was dragged onto a wire and that the node was dragged onto the wire
					if selected_nodes.selected_nodes_ref().len() == 1 && !self.begin_dragging {
						let selected_node_id = selected_nodes.selected_nodes_ref()[0];
						let has_primary_output_connection = network_interface
							.outward_wires(selection_network_path)
							.is_some_and(|outward_wires| outward_wires.get(&OutputConnector::node(selected_node_id, 0)).is_some_and(|outward_wires| !outward_wires.is_empty()));
						let Some(network) = network_interface.nested_network(selection_network_path) else {
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
									if is_stack_wire(&wire) { stack_wires.push(wire) } else { node_wires.push(wire) }
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
									let Some(network) = network_interface.nested_network(selection_network_path) else {
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
											input_connector: overlapping_wire.wire_end,
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
				// End of reordering an import
				else if let (Some(moving_import), Some(end_index)) = (self.reordering_import, self.end_index) {
					responses.add(NodeGraphMessage::ReorderImport {
						start_index: moving_import,
						end_index,
					});
					responses.add(DocumentMessage::EndTransaction);
				}
				// End of reordering an export
				else if let (Some(moving_export), Some(end_index)) = (self.reordering_export, self.end_index) {
					responses.add(NodeGraphMessage::ReorderExport {
						start_index: moving_export,
						end_index,
					});
					responses.add(DocumentMessage::EndTransaction);
				}
				self.drag_start = None;
				self.begin_dragging = false;
				self.box_selection_start = None;
				self.wire_in_progress_from_connector = None;
				self.wire_in_progress_to_connector = None;
				self.reordering_export = None;
				self.reordering_import = None;
				responses.add(DocumentMessage::EndTransaction);
				responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: None });
				responses.add(FrontendMessage::UpdateBox { box_selection: None });
				responses.add(FrontendMessage::UpdateImportReorderIndex { index: None });
				responses.add(FrontendMessage::UpdateExportReorderIndex { index: None });
				self.update_node_graph_hints(responses);
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
			NodeGraphMessage::RemoveImport { import_index: usize } => {
				network_interface.remove_import(usize, selection_network_path);
				responses.add(NodeGraphMessage::SendGraph);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::RemoveExport { export_index: usize } => {
				network_interface.remove_export(usize, selection_network_path);
				responses.add(NodeGraphMessage::SendGraph);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::ReorderImport { start_index, end_index } => {
				network_interface.reorder_import(start_index, end_index, selection_network_path);
				responses.add(NodeGraphMessage::SendGraph);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			NodeGraphMessage::ReorderExport { start_index, end_index } => {
				network_interface.reorder_export(start_index, end_index, selection_network_path);
				responses.add(NodeGraphMessage::SendGraph);
				responses.add(NodeGraphMessage::RunDocumentGraph);
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
					let (layer_widths, chain_widths, has_left_input_wire) = network_interface.collect_layer_widths(breadcrumb_network_path);
					let wires_direct_not_grid_aligned = preferences.graph_wire_style.is_direct();

					responses.add(NodeGraphMessage::UpdateImportsExports);
					responses.add(FrontendMessage::UpdateNodeGraph {
						nodes,
						wires,
						wires_direct_not_grid_aligned,
					});
					responses.add(FrontendMessage::UpdateLayerWidths {
						layer_widths,
						chain_widths,
						has_left_input_wire,
					});
					responses.add(NodeGraphMessage::SendSelectedNodes);
					self.update_node_graph_hints(responses);
				}
			}
			NodeGraphMessage::SetGridAlignedEdges => {
				if graph_view_overlay_open {
					network_interface.set_grid_aligned_edges(DVec2::new(ipp.viewport_bounds.bottom_right.x - ipp.viewport_bounds.top_left.x, 0.), breadcrumb_network_path);
					// Send the new edges to the frontend
					responses.add(NodeGraphMessage::UpdateImportsExports);
				}
			}
			NodeGraphMessage::SetInputValue { node_id, input_index, value } => {
				let input = NodeInput::value(value, false);
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, input_index),
					input,
				});
				responses.add(PropertiesPanelMessage::Refresh);
				if (network_interface
					.reference(&node_id, selection_network_path)
					.is_none_or(|reference| *reference != Some("Imaginate".to_string()))
					|| input_index == 0)
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
			NodeGraphMessage::ShiftSelectedNodesByAmount { mut graph_delta, rubber_band } => {
				while graph_delta != IVec2::ZERO {
					match graph_delta.x.cmp(&0) {
						Ordering::Greater => {
							responses.add(NodeGraphMessage::ShiftSelectedNodes {
								direction: Direction::Right,
								rubber_band,
							});
							graph_delta.x -= 1;
						}
						Ordering::Less => {
							responses.add(NodeGraphMessage::ShiftSelectedNodes {
								direction: Direction::Left,
								rubber_band,
							});
							graph_delta.x += 1;
						}
						Ordering::Equal => {}
					}

					match graph_delta.y.cmp(&0) {
						Ordering::Greater => {
							responses.add(NodeGraphMessage::ShiftSelectedNodes {
								direction: Direction::Down,
								rubber_band,
							});
							graph_delta.y -= 1;
						}
						Ordering::Less => {
							responses.add(NodeGraphMessage::ShiftSelectedNodes {
								direction: Direction::Up,
								rubber_band,
							});
							graph_delta.y += 1;
						}
						Ordering::Equal => {}
					}
				}
			}
			NodeGraphMessage::ToggleSelectedAsLayersOrNodes => {
				let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
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
			NodeGraphMessage::SetDisplayName {
				node_id,
				alias,
				skip_adding_history_step,
			} => {
				if !skip_adding_history_step {
					responses.add(DocumentMessage::StartTransaction);
				}
				responses.add(NodeGraphMessage::SetDisplayNameImpl { node_id, alias });
				if !skip_adding_history_step {
					// Does not add a history step if the name was not changed
					responses.add(DocumentMessage::EndTransaction);
				}
				responses.add(DocumentMessage::RenderRulers);
				responses.add(DocumentMessage::RenderScrollbars);
				responses.add(NodeGraphMessage::SendGraph);
			}
			NodeGraphMessage::SetDisplayNameImpl { node_id, alias } => {
				network_interface.set_display_name(&node_id, alias, selection_network_path);
			}
			NodeGraphMessage::SetImportExportName { name, index } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetImportExportNameImpl { name, index });
				responses.add(DocumentMessage::EndTransaction);
				responses.add(NodeGraphMessage::UpdateImportsExports);
			}
			NodeGraphMessage::SetImportExportNameImpl { name, index } => network_interface.set_import_export_name(name, index, breadcrumb_network_path),
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
				let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
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
				let Some(node_metadata) = network_interface.document_network_metadata().persistent_metadata.node_metadata.get(&node_id) else {
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
			NodeGraphMessage::ToggleSelectedIsPinned => {
				let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::ToggleSelectedIsPinned");
					return;
				};
				let node_ids = selected_nodes.selected_nodes().cloned().collect::<Vec<_>>();

				// If any of the selected nodes are pinned, unpin them all. Otherwise, pin them all.
				let pinned = !node_ids.iter().all(|node_id| network_interface.is_pinned(node_id, breadcrumb_network_path));

				responses.add(DocumentMessage::AddTransaction);
				for node_id in &node_ids {
					responses.add(NodeGraphMessage::SetPinned { node_id: *node_id, pinned });
				}
				responses.add(NodeGraphMessage::SetLockedOrVisibilitySideEffects { node_ids });
			}
			NodeGraphMessage::ToggleSelectedVisibility => {
				let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::ToggleSelectedLocked");
					return;
				};
				let node_ids = selected_nodes.selected_nodes().cloned().collect::<Vec<_>>();

				// If any of the selected nodes are hidden, show them all. Otherwise, hide them all.
				let visible = !node_ids.iter().all(|node_id| network_interface.is_visible(node_id, selection_network_path));

				responses.add(DocumentMessage::AddTransaction);
				for node_id in &node_ids {
					responses.add(NodeGraphMessage::SetVisibility { node_id: *node_id, visible });
				}
				responses.add(NodeGraphMessage::SetLockedOrVisibilitySideEffects { node_ids });
			}
			NodeGraphMessage::ToggleVisibility { node_id } => {
				let visible = !network_interface.is_visible(&node_id, selection_network_path);

				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::SetVisibility { node_id, visible });
				responses.add(NodeGraphMessage::SetLockedOrVisibilitySideEffects { node_ids: vec![node_id] });
			}
			NodeGraphMessage::SetPinned { node_id, pinned } => {
				network_interface.set_pinned(&node_id, selection_network_path, pinned);
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
				if let Some((box_selection_start, _)) = self.box_selection_start {
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

					let shift = ipp.keyboard.get(Key::Shift as usize);
					let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
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
						let quad = Quad::from_box([box_selection_start, box_selection_end_graph]);
						if click_targets.node_click_target.intersect_path(|| quad.bezier_lines(), DAffine2::IDENTITY) {
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
			NodeGraphMessage::UpdateImportsExports => {
				let imports = network_interface.frontend_imports(breadcrumb_network_path).unwrap_or_default();
				let exports = network_interface.frontend_exports(breadcrumb_network_path).unwrap_or_default();
				let add_import = network_interface
					.frontend_import_export_modify(
						|modify_import_export_click_target| modify_import_export_click_target.add_import_export.output_ports().collect::<Vec<_>>(),
						breadcrumb_network_path,
					)
					.into_iter()
					.next();
				let add_export = network_interface
					.frontend_import_export_modify(
						|modify_import_export_click_target| modify_import_export_click_target.add_import_export.input_ports().collect::<Vec<_>>(),
						breadcrumb_network_path,
					)
					.into_iter()
					.next();

				responses.add(FrontendMessage::UpdateImportsExports {
					imports,
					exports,
					add_import,
					add_export,
				});
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
					self.update_graph_bar_left(network_interface, breadcrumb_network_path, responses);
					self.send_node_bar_layout(responses);
				}
			}
			NodeGraphMessage::UpdateGraphBarRight => {
				self.update_graph_bar_right(graph_fade_artwork_percentage, network_interface, breadcrumb_network_path, navigation_handler);
				self.send_node_bar_layout(responses);
			}
			NodeGraphMessage::UpdateInSelectedNetwork => responses.add(FrontendMessage::UpdateInSelectedNetwork {
				in_selected_network: selection_network_path == breadcrumb_network_path,
			}),
			NodeGraphMessage::UpdateHints => {
				self.update_node_graph_hints(responses);
			}
			NodeGraphMessage::SendSelectedNodes => {
				let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(breadcrumb_network_path) else {
					log::error!("Could not get selected nodes in NodeGraphMessage::SendSelectedNodes");
					return;
				};
				responses.add(NodeGraphMessage::UpdateActionButtons);
				responses.add(FrontendMessage::UpdateNodeGraphSelection {
					selected: selected_nodes.selected_nodes().cloned().collect::<Vec<_>>(),
				});
			}
		}
		let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(selection_network_path) else {
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
				MergeSelectedNodes,
				ToggleSelectedAsLayersOrNodes,
				ToggleSelectedLocked,
				ToggleSelectedVisibility,
				ShiftSelectedNodes,
			));
		}

		common
	}

	/// Send the cached layout to the frontend for the control bar at the top of the node panel
	fn send_node_bar_layout(&self, responses: &mut VecDeque<Message>) {
		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout::new(self.widgets.to_vec())),
			layout_target: LayoutTarget::NodeGraphControlBar,
		});
	}

	/// Updates the buttons for visibility, locked, and preview
	fn update_graph_bar_left(&mut self, network_interface: &mut NodeNetworkInterface, breadcrumb_network_path: &[NodeId], responses: &mut VecDeque<Message>) {
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

		let Some(network) = network_interface.nested_network(breadcrumb_network_path) else {
			warn!("No network in update_selection_action_buttons");
			return;
		};

		let Some(selected_nodes) = network_interface.selected_nodes_in_nested_network(breadcrumb_network_path) else {
			warn!("No selected nodes in update_selection_action_buttons");
			return;
		};

		let has_selection = selected_nodes.has_selected_nodes();
		let selection_includes_layers = network_interface.selected_nodes().selected_layers(network_interface.document_metadata()).count() > 0;
		let selection_all_locked = network_interface.selected_nodes().selected_unlocked_layers(network_interface).count() == 0;
		let selection_all_visible = selected_nodes.selected_nodes().all(|node_id| network_interface.is_visible(node_id, breadcrumb_network_path));

		let mut widgets = vec![
			PopoverButton::new()
				.icon(Some("Node".to_string()))
				.tooltip("New Node (Right Click)")
				.popover_layout({
					let node_chooser = NodeCatalog::new()
						.on_update(move |node_type| {
							let node_id = NodeId::new();

							Message::Batched(Box::new([
								NodeGraphMessage::CreateNodeFromContextMenu {
									node_id: Some(node_id),
									node_type: node_type.clone(),
									xy: None,
								}
								.into(),
								NodeGraphMessage::SelectedNodesSet { nodes: vec![node_id] }.into(),
							]))
						})
						.widget_holder();
					vec![LayoutGroup::Row { widgets: vec![node_chooser] }]
				})
				.widget_holder(),
			//
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			//
			IconButton::new("NewLayer", 24)
				.tooltip("New Layer")
				.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::CreateEmptyFolder))
				.on_update(|_| DocumentMessage::CreateEmptyFolder.into())
				.widget_holder(),
			IconButton::new("Folder", 24)
				.tooltip("Group Selected")
				.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::GroupSelectedLayers))
				.on_update(|_| {
					let group_folder_type = GroupFolderType::Layer;
					DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
				})
				.disabled(!has_selection)
				.widget_holder(),
			IconButton::new("Trash", 24)
				.tooltip("Delete Selected")
				.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::DeleteSelectedLayers))
				.on_update(|_| DocumentMessage::DeleteSelectedLayers.into())
				.disabled(!has_selection)
				.widget_holder(),
			//
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			//
			IconButton::new(if selection_all_locked { "PadlockLocked" } else { "PadlockUnlocked" }, 24)
				.hover_icon(Some((if selection_all_locked { "PadlockUnlocked" } else { "PadlockLocked" }).into()))
				.tooltip(if selection_all_locked { "Unlock Selected" } else { "Lock Selected" })
				.tooltip_shortcut(action_keys!(NodeGraphMessageDiscriminant::ToggleSelectedLocked))
				.on_update(|_| NodeGraphMessage::ToggleSelectedLocked.into())
				.disabled(!has_selection || !selection_includes_layers)
				.widget_holder(),
			IconButton::new(if selection_all_visible { "EyeVisible" } else { "EyeHidden" }, 24)
				.hover_icon(Some((if selection_all_visible { "EyeHide" } else { "EyeShow" }).into()))
				.tooltip(if selection_all_visible { "Hide Selected" } else { "Show Selected" })
				.tooltip_shortcut(action_keys!(NodeGraphMessageDiscriminant::ToggleSelectedVisibility))
				.on_update(|_| NodeGraphMessage::ToggleSelectedVisibility.into())
				.disabled(!has_selection)
				.widget_holder(),
		];

		let mut selection = selected_nodes.selected_nodes();
		let (selection, no_other_selections) = (selection.next(), selection.count() == 0);
		let previewing = if matches!(network_interface.previewing(breadcrumb_network_path), Previewing::Yes { .. }) {
			network.exports.iter().find_map(|export| {
				let NodeInput::Node { node_id, .. } = export else { return None };
				Some(*node_id)
			})
		} else {
			None
		};

		// If only one node is selected then show the preview or stop previewing button
		if let Some(node_id) = previewing {
			let button = TextButton::new("End Preview")
				.icon(Some("FrameAll".to_string()))
				.tooltip("Restore preview to the graph output")
				.on_update(move |_| NodeGraphMessage::TogglePreview { node_id }.into())
				.widget_holder();
			widgets.extend([Separator::new(SeparatorType::Unrelated).widget_holder(), button]);
		} else if let Some(&node_id) = selection {
			let selection_is_not_already_the_output = !network
				.exports
				.iter()
				.any(|export| matches!(export, NodeInput::Node { node_id: export_node_id, .. } if *export_node_id == node_id));
			if selection_is_not_already_the_output && no_other_selections {
				let button = TextButton::new("Preview")
					.icon(Some("FrameAll".to_string()))
					.tooltip("Preview selected node/layer (Shortcut: Alt-click node/layer)")
					.on_update(move |_| NodeGraphMessage::TogglePreview { node_id }.into())
					.widget_holder();
				widgets.extend([Separator::new(SeparatorType::Unrelated).widget_holder(), button]);
			}
		}

		let subgraph_path_names_length = subgraph_path_names.len();
		if subgraph_path_names_length >= 2 {
			widgets.extend([
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				BreadcrumbTrailButtons::new(subgraph_path_names)
					.on_update(move |index| {
						DocumentMessage::ExitNestedNetwork {
							steps_back: subgraph_path_names_length - (*index as usize) - 1,
						}
						.into()
					})
					.widget_holder(),
			]);
		}

		self.widgets[0] = LayoutGroup::Row { widgets };
	}

	fn update_graph_bar_right(
		&mut self,
		graph_fade_artwork_percentage: f64,
		network_interface: &NodeNetworkInterface,
		breadcrumb_network_path: &[NodeId],
		navigation_handler: &NavigationMessageHandler,
	) {
		let Some(node_graph_ptz) = network_interface.node_graph_ptz(breadcrumb_network_path) else {
			log::error!("Could not get node graph PTZ");
			return;
		};

		let mut widgets = vec![
			NumberInput::new(Some(graph_fade_artwork_percentage))
				.percentage()
				.display_decimal_places(0)
				.label("Fade Artwork")
				.tooltip("Opacity of the graph background that covers the artwork")
				.on_update(move |number_input: &NumberInput| {
					DocumentMessage::SetGraphFadeArtwork {
						percentage: number_input.value.unwrap_or(graph_fade_artwork_percentage),
					}
					.into()
				})
				.widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
		];
		widgets.extend(navigation_controls(node_graph_ptz, navigation_handler, "Node Graph"));
		widgets.extend([
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextButton::new("Node Graph")
				.icon(Some("GraphViewOpen".into()))
				.hover_icon(Some("GraphViewClosed".into()))
				.tooltip("Hide Node Graph")
				.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::GraphViewOverlayToggle))
				.on_update(move |_| DocumentMessage::GraphViewOverlayToggle.into())
				.widget_holder(),
		]);

		self.widgets[1] = LayoutGroup::Row { widgets };
	}

	/// Collate the properties panel sections for a node graph
	pub fn collate_properties(context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
		// If the selected nodes are in the document network, use the document network. Otherwise, use the nested network
		let Some(selected_nodes) = context.network_interface.selected_nodes_in_nested_network(context.selection_network_path) else {
			warn!("No selected nodes in collate_properties");
			return Vec::new();
		};

		// We want:
		// - If only nodes (no layers) are selected: display each node's properties
		// - If one layer is selected, and zero or more of its (primary flow) upstream nodes: display the properties for the layer and all its upstream nodes
		// - If multiple layers are selected, or one node plus other non-upstream nodes: display nothing
		// - If nothing is selected, display any pinned nodes/layers

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
			0 => {
				let selected_nodes = nodes.iter().map(|node_id| node_properties::generate_node_properties(*node_id, context)).collect::<Vec<_>>();
				if !selected_nodes.is_empty() {
					return selected_nodes;
				}

				// TODO: Display properties for encapsulating node when no nodes are selected in a nested network
				// This may require store a separate path for the properties panel
				let mut properties = vec![LayoutGroup::Row {
					widgets: vec![
						Separator::new(SeparatorType::Related).widget_holder(),
						IconLabel::new("File").tooltip("Name of the current document").widget_holder(),
						Separator::new(SeparatorType::Related).widget_holder(),
						TextInput::new(context.document_name)
							.tooltip("Name of the current document")
							.on_update(|text_input| DocumentMessage::RenameDocument { new_name: text_input.value.clone() }.into())
							.widget_holder(),
						Separator::new(SeparatorType::Related).widget_holder(),
					],
				}];

				let Some(network) = context.network_interface.nested_network(context.selection_network_path) else {
					warn!("No network in collate_properties");
					return Vec::new();
				};
				// And if no nodes are selected, show properties for all pinned nodes
				let pinned_node_properties = network
					.nodes
					.keys()
					.cloned()
					.collect::<Vec<_>>()
					.iter()
					.filter_map(|node_id| {
						if context.network_interface.is_pinned(node_id, context.selection_network_path) {
							Some(node_properties::generate_node_properties(*node_id, context))
						} else {
							None
						}
					})
					.collect::<Vec<_>>();

				properties.extend(pinned_node_properties);
				properties
			}
			// If one layer is selected, filter out all selected nodes that are not upstream of it. If there are no nodes left, show properties for the layer. Otherwise, show nothing.
			1 => {
				let layer = layers[0];
				let nodes_not_upstream_of_layer = nodes.into_iter().filter(|&selected_node_id| {
					!context
						.network_interface
						.is_node_upstream_of_another_by_horizontal_flow(layer, context.selection_network_path, selected_node_id)
				});
				if nodes_not_upstream_of_layer.count() > 0 {
					return Vec::new();
				}

				let mut layer_properties = vec![LayoutGroup::Row {
					widgets: vec![
						Separator::new(SeparatorType::Related).widget_holder(),
						IconLabel::new("Layer").tooltip("Name of the selected layer").widget_holder(),
						Separator::new(SeparatorType::Related).widget_holder(),
						TextInput::new(context.network_interface.frontend_display_name(&layer, context.selection_network_path))
							.tooltip("Name of the selected layer")
							.on_update(move |text_input| {
								NodeGraphMessage::SetDisplayName {
									node_id: layer,
									alias: text_input.value.clone(),
									skip_adding_history_step: false,
								}
								.into()
							})
							.widget_holder(),
						Separator::new(SeparatorType::Related).widget_holder(),
						PopoverButton::new()
							.icon(Some("Node".to_string()))
							.tooltip("Add an operation to the end of this layer's chain of nodes")
							.popover_layout({
								let node_chooser = NodeCatalog::new()
									.on_update(move |node_type| {
										NodeGraphMessage::CreateNodeInLayerWithTransaction {
											node_type: node_type.clone(),
											layer: LayerNodeIdentifier::new_unchecked(layer),
										}
										.into()
									})
									.widget_holder();
								vec![LayoutGroup::Row { widgets: vec![node_chooser] }]
							})
							.widget_holder(),
						Separator::new(SeparatorType::Related).widget_holder(),
					],
				}];

				// Iterate through all the upstream nodes, but stop when we reach another layer (since that's a point where we switch from horizontal to vertical flow)
				let node_properties = context
					.network_interface
					.upstream_flow_back_from_nodes(vec![layer], context.selection_network_path, network_interface::FlowType::HorizontalFlow)
					.enumerate()
					.take_while(|(i, node_id)| {
						if *i == 0 {
							true
						} else {
							!context.network_interface.is_layer(node_id, context.selection_network_path)
						}
					})
					.collect::<Vec<_>>()
					.iter()
					.map(|(_, node_id)| node_properties::generate_node_properties(*node_id, context))
					.collect::<Vec<_>>();

				layer_properties.extend(node_properties);
				layer_properties
			}
			// If multiple layers and/or nodes are selected, show nothing
			_ => Vec::new(),
		}
	}

	fn collect_wires(network_interface: &NodeNetworkInterface, breadcrumb_network_path: &[NodeId]) -> Vec<FrontendNodeWire> {
		let Some(network) = network_interface.nested_network(breadcrumb_network_path) else {
			log::error!("Could not get network when collecting wires");
			return Vec::new();
		};
		let mut wires = network
			.nodes
			.iter()
			.flat_map(|(wire_end, node)| node.inputs.iter().filter(|input| input.is_exposed()).enumerate().map(move |(index, input)| (input, wire_end, index)))
			.filter_map(|(input, &wire_end, wire_end_input_index)| {
				match *input {
					NodeInput::Node {
						node_id: wire_start,
						output_index: wire_start_output_index,
						// TODO: add ui for lambdas
						lambda: _,
					} => Some(FrontendNodeWire {
						wire_start: OutputConnector::node(wire_start, wire_start_output_index),
						wire_end: InputConnector::node(wire_end, wire_end_input_index),
						dashed: false,
					}),
					NodeInput::Network { import_index, .. } => Some(FrontendNodeWire {
						wire_start: OutputConnector::Import(import_index),
						wire_end: InputConnector::node(wire_end, wire_end_input_index),
						dashed: false,
					}),
					_ => None,
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
		let Some(network) = network_interface.nested_network(breadcrumb_network_path) else {
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
		let mut frontend_inputs_lookup = frontend_inputs_lookup(breadcrumb_network_path, network_interface);
		let Some(network) = network_interface.nested_network(breadcrumb_network_path) else {
			log::error!("Could not get nested network when collecting nodes");
			return Vec::new();
		};
		let Some(network_metadata) = network_interface.network_metadata(breadcrumb_network_path) else {
			log::error!("Could not get network_metadata when collecting nodes");
			return Vec::new();
		};

		let mut nodes = Vec::new();
		for (&node_id, node) in &network.nodes {
			let node_id_path = [breadcrumb_network_path, (&[node_id])].concat();

			let inputs = frontend_inputs_lookup.remove(&node_id).unwrap_or_default();
			let mut inputs = inputs.into_iter().map(|input| {
				input.map(|input| FrontendGraphInput {
					data_type: FrontendGraphDataType::displayed_type(&input.ty, &input.type_source),
					resolved_type: Some(format!("{:?} from {:?}", &input.ty, input.type_source)),
					valid_types: input.valid_types.iter().map(|ty| ty.to_string()).collect(),
					name: input.name.unwrap_or_else(|| input.ty.nested_type().to_string()),
					connected_to: input.output_connector,
				})
			});

			let primary_input = inputs.next().flatten();
			let exposed_inputs = inputs.flatten().collect();

			let output_types = network_interface.output_types(&node_id, breadcrumb_network_path);
			let primary_output_type = output_types.first().cloned().flatten();
			let frontend_data_type = if let Some((output_type, type_source)) = &primary_output_type {
				FrontendGraphDataType::displayed_type(output_type, type_source)
			} else {
				FrontendGraphDataType::General
			};
			let connected_to = outward_wires.get(&OutputConnector::node(node_id, 0)).cloned().unwrap_or_default();
			let primary_output = if network_interface.has_primary_output(&node_id, breadcrumb_network_path) && !output_types.is_empty() {
				Some(FrontendGraphOutput {
					data_type: frontend_data_type,
					name: "Output 1".to_string(),
					resolved_type: primary_output_type.map(|(input, type_source)| format!("{input:?} from {type_source:?}")),
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
				let frontend_data_type = if let Some((output_type, type_source)) = &exposed_output {
					FrontendGraphDataType::displayed_type(output_type, type_source)
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
					.filter(|output_name| !output_name.is_empty())
					.unwrap_or_else(|| exposed_output.clone().map(|(output_type, _)| output_type.nested_type().to_string()).unwrap_or_default());

				let connected_to = outward_wires.get(&OutputConnector::node(node_id, index)).cloned().unwrap_or_default();
				exposed_outputs.push(FrontendGraphOutput {
					data_type: frontend_data_type,
					name: output_name,
					resolved_type: exposed_output.clone().map(|(input, type_source)| format!("{input:?} from {type_source:?}")),
					connected_to,
				});
			}
			let Some(network) = network_interface.nested_network(breadcrumb_network_path) else {
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
				.find(|error| error.node_path == node_id_path)
				.map(|error| format!("{:?}", error.error.clone()))
				.or_else(|| {
					if self.node_graph_errors.iter().any(|error| error.node_path.starts_with(&node_id_path)) {
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
				reference: network_interface.reference(&node_id, breadcrumb_network_path).cloned().unwrap_or_default(),
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
		let mut current_network = network_interface.nested_network(&current_network_path).unwrap();
		let mut subgraph_names = vec!["Document".to_string()];
		for node_id in breadcrumb_network_path {
			let node = current_network.nodes.get(node_id)?;
			if let Some(network) = node.implementation.get_network() {
				current_network = network;
			};
			subgraph_names.push(network_interface.frontend_display_name(node_id, &current_network_path));
			current_network_path.push(*node_id)
		}
		Some(subgraph_names)
	}

	fn update_layer_panel(network_interface: &NodeNetworkInterface, selection_network_path: &[NodeId], collapsed: &CollapsedLayers, responses: &mut VecDeque<Message>) {
		let selected_layers = network_interface
			.selected_nodes()
			.selected_layers(network_interface.document_metadata())
			.map(|layer| layer.to_node())
			.collect::<HashSet<_>>();

		let mut ancestors_of_selected = HashSet::new();
		let mut descendants_of_selected = HashSet::new();
		for selected_layer in &selected_layers {
			for ancestor in LayerNodeIdentifier::new(*selected_layer, network_interface, &[]).ancestors(network_interface.document_metadata()) {
				if ancestor != LayerNodeIdentifier::ROOT_PARENT && ancestor.to_node() != *selected_layer {
					ancestors_of_selected.insert(ancestor.to_node());
				}
			}
			for descendant in LayerNodeIdentifier::new(*selected_layer, network_interface, &[]).descendants(network_interface.document_metadata()) {
				descendants_of_selected.insert(descendant.to_node());
			}
		}

		for (&node_id, node_metadata) in &network_interface.document_network_metadata().persistent_metadata.node_metadata {
			if node_metadata.persistent_metadata.is_layer() {
				let layer = LayerNodeIdentifier::new(node_id, network_interface, &[]);

				let children_allowed =
						// The layer has other layers as children along the secondary input's horizontal flow
						layer.has_children(network_interface.document_metadata())
						|| (
							// Check if the last node in the chain has an exposed left input
							network_interface.upstream_flow_back_from_nodes(vec![node_id], &[], network_interface::FlowType::HorizontalFlow).last().is_some_and(|node_id|
								network_interface.document_node(&node_id, &[]).map_or_else(||{log::error!("Could not get node {node_id} in update_layer_panel"); false}, |node| {
									if network_interface.is_layer(&node_id, &[]) {
										node.inputs.iter().filter(|input| input.is_exposed_to_frontend(true)).nth(1).is_some_and(|input| input.as_value().is_some())
									} else {
										node.inputs.iter().filter(|input| input.is_exposed_to_frontend(true)).nth(0).is_some_and(|input| input.as_value().is_some())
									}
								}))
						);

				let parents_visible = layer.ancestors(network_interface.document_metadata()).filter(|&ancestor| ancestor != layer).all(|layer| {
					if layer != LayerNodeIdentifier::ROOT_PARENT {
						network_interface.document_node(&layer.to_node(), &[]).map(|node| node.visible).unwrap_or_default()
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

				let data = LayerPanelEntry {
					id: node_id,
					alias: network_interface.frontend_display_name(&node_id, &[]),
					tooltip: if cfg!(debug_assertions) { format!("Layer ID: {node_id}") } else { "".into() },
					in_selected_network: selection_network_path.is_empty(),
					children_allowed,
					children_present: layer.has_children(network_interface.document_metadata()),
					expanded: layer.has_children(network_interface.document_metadata()) && !collapsed.0.contains(&layer),
					depth: layer.ancestors(network_interface.document_metadata()).count() - 1,
					visible: network_interface.is_visible(&node_id, &[]),
					parents_visible,
					unlocked: !network_interface.is_locked(&node_id, &[]),
					parents_unlocked,
					parent_id: layer
						.parent(network_interface.document_metadata())
						.and_then(|parent| if parent != LayerNodeIdentifier::ROOT_PARENT { Some(parent.to_node()) } else { None }),
					selected: selected_layers.contains(&node_id),
					ancestor_of_selected: ancestors_of_selected.contains(&node_id),
					descendant_of_selected: descendants_of_selected.contains(&node_id),
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
		let curve_falloff_rate = curve_length * std::f64::consts::PI * 2.;

		let horizontal_curve_amount = -(2_f64.powf((-10. * horizontal_gap) / curve_falloff_rate)) + 1.;
		let vertical_curve_amount = -(2_f64.powf((-10. * vertical_gap) / curve_falloff_rate)) + 1.;
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

	pub fn update_node_graph_hints(&self, responses: &mut VecDeque<Message>) {
		// A wire is in progress and its start and end connectors are set
		let wiring = self.wire_in_progress_from_connector.is_some();

		// Node gragging is in progress (having already moved at least one pixel from the mouse down position)
		let dragging_nodes = self.drag_start.as_ref().is_some_and(|(_, dragged)| *dragged);

		// A box selection is in progress
		let dragging_box_selection = self.box_selection_start.is_some_and(|(_, box_selection_dragged)| box_selection_dragged);

		// Cancel the ongoing action
		if wiring || dragging_nodes || dragging_box_selection {
			let hint_data = HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]);
			responses.add(FrontendMessage::UpdateInputHints { hint_data });
			return;
		}

		// Default hints for all other states
		let mut hint_data = HintData(vec![
			HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, "Add Node")]),
			HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Select Node"), HintInfo::keys([Key::Shift], "Extend").prepend_plus()]),
			HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"), HintInfo::keys([Key::Shift], "Extend").prepend_plus()]),
		]);
		if self.has_selection {
			hint_data.0.extend([
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
				HintGroup(vec![HintInfo::keys([Key::Delete], "Delete Selected"), HintInfo::keys([Key::Control], "Keep Children").prepend_plus()]),
				HintGroup(vec![
					HintInfo::keys_and_mouse([Key::Alt], MouseMotion::LmbDrag, "Move Duplicate"),
					HintInfo::keys([Key::Control, Key::KeyD], "Duplicate").add_mac_keys([Key::Command, Key::KeyD]),
				]),
			]);
		}
		hint_data.0.extend([
			HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDouble, "Enter Node Subgraph")]),
			HintGroup(vec![HintInfo::keys_and_mouse([Key::Alt], MouseMotion::Lmb, "Preview Node Output")]),
		]);
		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}
}

#[derive(Default)]
struct InputLookup {
	name: Option<String>,
	ty: Type,
	type_source: TypeSource,
	valid_types: Vec<Type>,
	output_connector: Option<OutputConnector>,
}

type FrontendInputsLookup = HashMap<NodeId, Vec<Option<InputLookup>>>;

/// Create a lookup hashmap that can be used to create the frontend inputs. This is needed because `input_type` requires a mutable `network_interface`.
fn frontend_inputs_lookup(breadcrumb_network_path: &[NodeId], network_interface: &mut NodeNetworkInterface) -> FrontendInputsLookup {
	let Some(network) = network_interface.nested_network(breadcrumb_network_path) else {
		return Default::default();
	};
	let mut frontend_inputs_lookup = HashMap::new();
	for (&node_id, node) in network.nodes.iter() {
		let mut inputs = Vec::with_capacity(node.inputs.len());
		for (index, input) in node.inputs.iter().enumerate() {
			let is_exposed = input.is_exposed_to_frontend(breadcrumb_network_path.is_empty());

			// Skip not exposed inputs (they still get an entry to help with finding the primary input)
			if !is_exposed {
				inputs.push(None);
				continue;
			}

			// Get the name from the metadata here (since it also requires a reference to the `network_interface`)
			let name = network_interface
				.input_name(&node_id, index, breadcrumb_network_path)
				.filter(|s| !s.is_empty())
				.map(|name| name.to_string());
			// Get the output connector that feeds into this input (done here as well for simplicity)
			let connector = OutputConnector::from_input(input);

			inputs.push(Some(InputLookup {
				name,
				output_connector: connector,
				..Default::default()
			}));
		}
		frontend_inputs_lookup.insert(node_id, inputs);
	}

	for (&node_id, value) in frontend_inputs_lookup.iter_mut() {
		for (index, value) in value.iter_mut().enumerate() {
			// Skip not exposed inputs for efficiency
			let Some(value) = value else { continue };
			// Resolve the type (done in a separate loop because it requires a mutable reference to the `network_interface`)
			let (ty, type_source) = network_interface.input_type(&InputConnector::node(node_id, index), breadcrumb_network_path);
			value.ty = ty;
			value.type_source = type_source;
		}
	}

	for (&node_id, value) in frontend_inputs_lookup.iter_mut() {
		for (index, value) in value.iter_mut().enumerate() {
			// Skip not exposed inputs for efficiency
			let Some(value) = value else { continue };
			// Resolve the type (done in a separate loop because it requires a mutable reference to the `network_interface`)
			value.valid_types = network_interface.valid_input_types(&InputConnector::node(node_id, index), breadcrumb_network_path);
		}
	}
	frontend_inputs_lookup
}

impl Default for NodeGraphMessageHandler {
	fn default() -> Self {
		Self {
			network: Vec::new(),
			node_graph_errors: Vec::new(),
			has_selection: false,
			widgets: [LayoutGroup::Row { widgets: Vec::new() }, LayoutGroup::Row { widgets: Vec::new() }],
			drag_start: None,
			begin_dragging: false,
			node_has_moved_in_drag: false,
			shift_without_push: false,
			box_selection_start: None,
			selection_before_pointer_down: Vec::new(),
			disconnecting: None,
			initial_disconnecting: false,
			select_if_not_dragged: None,
			wire_in_progress_from_connector: None,
			wire_in_progress_to_connector: None,
			context_menu: None,
			deselect_on_pointer_up: None,
			auto_panning: Default::default(),
			preview_on_mouse_up: None,
			reordering_export: None,
			reordering_import: None,
			end_index: None,
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
			&& self.node_has_moved_in_drag == other.node_has_moved_in_drag
			&& self.box_selection_start == other.box_selection_start
			&& self.initial_disconnecting == other.initial_disconnecting
			&& self.select_if_not_dragged == other.select_if_not_dragged
			&& self.wire_in_progress_from_connector == other.wire_in_progress_from_connector
			&& self.wire_in_progress_to_connector == other.wire_in_progress_to_connector
			&& self.context_menu == other.context_menu
	}
}
