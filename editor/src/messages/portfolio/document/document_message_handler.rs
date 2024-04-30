use super::utility_types::error::EditorError;
use super::utility_types::misc::{BoundingBoxSnapTarget, GeometrySnapTarget, OptionBoundsSnapping, OptionPointSnapping, SnappingOptions, SnappingState};
use super::utility_types::nodes::{CollapsedLayers, SelectedNodes};
use crate::application::{generate_uuid, GRAPHITE_GIT_COMMIT_HASH};
use crate::consts::{ASYMPTOTIC_EFFECT, DEFAULT_DOCUMENT_NAME, FILE_SAVE_SUFFIX, SCALE_EFFECT, SCROLLBAR_SPACING};
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type;
use crate::messages::portfolio::document::node_graph::NodeGraphHandlerData;
use crate::messages::portfolio::document::overlays::grid_overlays::{grid_overlay, overlay_options};
use crate::messages::portfolio::document::properties_panel::utility_types::PropertiesPanelMessageHandlerData;
use crate::messages::portfolio::document::utility_types::document_metadata::{is_artboard, DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, DocumentMode, FlipAxis, PTZ};
use crate::messages::portfolio::document::utility_types::nodes::RawBuffer;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::{get_blend_mode, get_opacity};
use crate::messages::tool::utility_types::ToolType;
use crate::node_graph_executor::NodeGraphExecutor;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, DocumentNodeMetadata, NodeId, NodeInput, NodeNetwork, NodeOutput};
use graphene_core::raster::BlendMode;
use graphene_core::raster::ImageFrame;
use graphene_core::renderer::ClickTarget;
use graphene_core::transform::Footprint;
use graphene_core::vector::style::ViewMode;
use graphene_core::{concrete, generic, ProtoNodeIdentifier};
use graphene_std::wasm_application_io::WasmEditorApi;

use glam::{DAffine2, DVec2, IVec2};

use std::vec;

pub struct DocumentMessageData<'a> {
	pub document_id: DocumentId,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub persistent_data: &'a PersistentData,
	pub executor: &'a mut NodeGraphExecutor,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DocumentMessageHandler {
	// ======================
	// Child message handlers
	// ======================
	#[serde(skip)]
	navigation_handler: NavigationMessageHandler,
	#[serde(skip)]
	node_graph_handler: NodeGraphMessageHandler,
	#[serde(skip)]
	overlays_message_handler: OverlaysMessageHandler,
	#[serde(skip)]
	properties_panel_message_handler: PropertiesPanelMessageHandler,
	// ============================================
	// Fields that are saved in the document format
	// ============================================
	#[serde(default = "default_network")]
	pub network: NodeNetwork,
	#[serde(default = "default_selected_nodes")]
	pub selected_nodes: SelectedNodes,
	#[serde(default = "default_collapsed")]
	pub collapsed: CollapsedLayers,
	#[serde(default = "default_name")]
	pub name: String,
	#[serde(default = "default_commit_hash")]
	commit_hash: String,
	#[serde(default = "default_pan_tilt_zoom")]
	pub navigation: PTZ,
	#[serde(default = "default_document_mode")]
	document_mode: DocumentMode,
	#[serde(default = "default_view_mode")]
	pub view_mode: ViewMode,
	#[serde(default = "default_overlays_visible")]
	overlays_visible: bool,
	#[serde(default = "default_rulers_visible")]
	pub rulers_visible: bool,
	// =============================================
	// Fields omitted from the saved document format
	// =============================================
	#[serde(skip)]
	document_undo_history: VecDeque<NodeNetwork>,
	#[serde(skip)]
	document_redo_history: VecDeque<NodeNetwork>,
	#[serde(skip)]
	saved_hash: Option<u64>,
	#[serde(skip)]
	auto_saved_hash: Option<u64>,
	/// Don't allow aborting transactions whilst undoing to avoid #559
	#[serde(skip)]
	undo_in_progress: bool,
	#[serde(skip)]
	graph_view_overlay_open: bool,
	#[serde(skip)]
	pub snapping_state: SnappingState,
	#[serde(skip)]
	layer_range_selection_reference: Option<LayerNodeIdentifier>,
	#[serde(skip)]
	pub metadata: DocumentMetadata,
}

impl MessageHandler<DocumentMessage, DocumentMessageData<'_>> for DocumentMessageHandler {
	fn process_message(&mut self, message: DocumentMessage, responses: &mut VecDeque<Message>, data: DocumentMessageData) {
		let DocumentMessageData {
			document_id,
			ipp,
			persistent_data,
			executor,
		} = data;

		match message {
			// Sub-messages
			DocumentMessage::Navigation(message) => {
				let document_bounds = self.metadata().document_bounds_viewport_space();
				let data = NavigationMessageData {
					metadata: &self.metadata,
					document_bounds,
					ipp,
					selection_bounds: self.selected_visible_layers_bounding_box_viewport(),
					ptz: &mut self.navigation,
				};

				self.navigation_handler.process_message(message, responses, data);
			}
			DocumentMessage::Overlays(message) => {
				let overlays_visible = self.overlays_visible;
				self.overlays_message_handler.process_message(message, responses, OverlaysMessageData { overlays_visible, ipp });
			}
			DocumentMessage::PropertiesPanel(message) => {
				let properties_panel_message_handler_data = PropertiesPanelMessageHandlerData {
					node_graph_message_handler: &self.node_graph_handler,
					executor,
					document_name: self.name.as_str(),
					document_network: &self.network,
					document_metadata: &mut self.metadata,
					selected_nodes: &self.selected_nodes,
				};
				self.properties_panel_message_handler
					.process_message(message, responses, (persistent_data, properties_panel_message_handler_data));
			}
			DocumentMessage::NodeGraph(message) => {
				self.node_graph_handler.process_message(
					message,
					responses,
					NodeGraphHandlerData {
						document_network: &mut self.network,
						document_metadata: &mut self.metadata,
						selected_nodes: &mut self.selected_nodes,
						document_id,
						document_name: self.name.as_str(),
						collapsed: &mut self.collapsed,
						input: ipp,
						graph_view_overlay_open: self.graph_view_overlay_open,
					},
				);
			}
			DocumentMessage::GraphOperation(message) => {
				let data = GraphOperationMessageData {
					document_network: &mut self.network,
					document_metadata: &mut self.metadata,
					selected_nodes: &mut self.selected_nodes,
					collapsed: &mut self.collapsed,
					node_graph: &mut self.node_graph_handler,
				};
				let mut graph_operation_message_handler = GraphOperationMessageHandler {};
				graph_operation_message_handler.process_message(message, responses, data);
			}

			// Messages
			DocumentMessage::AbortTransaction => {
				if !self.undo_in_progress {
					self.undo(responses);
					responses.add(OverlaysMessage::Draw);
				}
			}
			DocumentMessage::AlignSelectedLayers { axis, aggregate } => {
				self.backup(responses);

				let axis = match axis {
					AlignAxis::X => DVec2::X,
					AlignAxis::Y => DVec2::Y,
				};
				let Some(combined_box) = self.selected_visible_layers_bounding_box_viewport() else {
					return;
				};

				let aggregated = match aggregate {
					AlignAggregate::Min => combined_box[0],
					AlignAggregate::Max => combined_box[1],
					AlignAggregate::Center => (combined_box[0] + combined_box[1]) / 2.,
				};
				for layer in self.selected_nodes.selected_unlocked_layers(self.metadata()) {
					let Some(bbox) = self.metadata().bounding_box_viewport(layer) else {
						continue;
					};
					let center = match aggregate {
						AlignAggregate::Min => bbox[0],
						AlignAggregate::Max => bbox[1],
						_ => (bbox[0] + bbox[1]) / 2.,
					};
					let translation = (aggregated - center) * axis;
					responses.add(GraphOperationMessage::TransformChange {
						layer,
						transform: DAffine2::from_translation(translation),
						transform_in: TransformIn::Viewport,
						skip_rerender: false,
					});
				}
			}
			DocumentMessage::BackupDocument { network } => self.backup_with_document(network, responses),
			DocumentMessage::ClearArtboards => {
				self.backup(responses);
				responses.add(GraphOperationMessage::ClearArtboards);
			}
			DocumentMessage::ClearLayersPanel => {
				// Send an empty layer list
				let data_buffer: RawBuffer = Self::default().serialize_root();
				responses.add(FrontendMessage::UpdateDocumentLayerStructure { data_buffer });

				// Clear the options bar
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(Default::default()),
					layout_target: LayoutTarget::LayersPanelOptions,
				});
			}
			DocumentMessage::CommitTransaction => (),
			DocumentMessage::CreateEmptyFolder => {
				let id = NodeId(generate_uuid());

				let parent = self
					.metadata()
					.deepest_common_ancestor(self.selected_nodes.selected_layers(self.metadata()), true)
					.unwrap_or(LayerNodeIdentifier::ROOT);

				let insert_index = parent
					.children(self.metadata())
					.enumerate()
					.find_map(|(index, item)| self.selected_nodes.selected_layers(self.metadata()).any(|x| x == item).then_some(index as isize))
					.unwrap_or(-1);
				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::NewCustomLayer {
					id,
					nodes: HashMap::new(),
					parent,
					insert_index,
					alias: String::new(),
				});
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });
			}
			DocumentMessage::DebugPrintDocument => {
				info!("{:#?}", self.network);
			}
			DocumentMessage::DeleteLayer { id } => {
				responses.add(NodeGraphMessage::DeleteNodes { node_ids: vec![id], reconnect: true });
				responses.add_front(BroadcastEvent::ToolAbort);
			}
			DocumentMessage::DeleteSelectedLayers => {
				self.backup(responses);

				responses.add_front(BroadcastEvent::SelectionChanged);
				for path in self.metadata().shallowest_unique_layers(self.selected_nodes.selected_layers(self.metadata())) {
					responses.add_front(DocumentMessage::DeleteLayer { id: path.last().unwrap().to_node() });
				}
			}
			DocumentMessage::DeselectAllLayers => {
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				self.layer_range_selection_reference = None;
			}
			DocumentMessage::DocumentHistoryBackward => self.undo_with_history(responses),
			DocumentMessage::DocumentHistoryForward => self.redo_with_history(responses),
			DocumentMessage::DocumentStructureChanged => {
				self.update_layers_panel_options_bar_widgets(responses);

				self.metadata.load_structure(&self.network, &mut self.selected_nodes);
				let data_buffer: RawBuffer = self.serialize_root();
				responses.add(FrontendMessage::UpdateDocumentLayerStructure { data_buffer });
			}
			DocumentMessage::DuplicateSelectedLayers => {
				self.backup(responses);
				for layer_ancestors in self.metadata.shallowest_unique_layers(self.selected_nodes.selected_layers(&self.metadata)) {
					let Some(layer) = layer_ancestors.last().copied() else { continue };
					let Some(parent) = layer.parent(&self.metadata) else { continue };
					let Some(node) = self.network().nodes.get(&layer.to_node()).and_then(|node| node.inputs.first()).and_then(|input| input.as_node()) else {
						continue;
					};

					let nodes = NodeGraphMessageHandler::copy_nodes(
						self.network(),
						&self
							.network()
							.upstream_flow_back_from_nodes(vec![node], graph_craft::document::FlowType::UpstreamFlow)
							.enumerate()
							.map(|(index, (_, node_id))| (node_id, NodeId(index as u64)))
							.collect(),
					)
					.collect();

					let id = NodeId(generate_uuid());
					let selected_layer_index = parent.children(self.metadata()).collect::<Vec<_>>().iter().position(|&sibling| sibling == layer).unwrap_or(0);
					let insert_index = if (selected_layer_index as i64 - 1) < 0 { -1 } else { selected_layer_index as isize };
					responses.add(GraphOperationMessage::NewCustomLayer {
						id,
						nodes,
						parent,
						insert_index,
						alias: String::new(),
					});
				}
			}
			DocumentMessage::FlipSelectedLayers { flip_axis } => {
				self.backup(responses);
				let scale = match flip_axis {
					FlipAxis::X => DVec2::new(-1., 1.),
					FlipAxis::Y => DVec2::new(1., -1.),
				};
				if let Some([min, max]) = self.selected_visible_and_unlock_layers_bounding_box_viewport() {
					let center = (max + min) / 2.;
					let bbox_trans = DAffine2::from_translation(-center);
					for layer in self.selected_nodes.selected_unlocked_layers(self.metadata()) {
						responses.add(GraphOperationMessage::TransformChange {
							layer,
							transform: DAffine2::from_scale(scale),
							transform_in: TransformIn::Scope { scope: bbox_trans },
							skip_rerender: false,
						});
					}
				}
			}
			DocumentMessage::GraphViewOverlay { open } => {
				self.graph_view_overlay_open = open;

				if open {
					responses.add(NodeGraphMessage::SendGraph);
				}
				responses.add(FrontendMessage::TriggerGraphViewOverlay { open });
			}
			DocumentMessage::GraphViewOverlayToggle => {
				responses.add(DocumentMessage::GraphViewOverlay { open: !self.graph_view_overlay_open });
			}
			DocumentMessage::GridOptions(grid) => {
				self.snapping_state.grid = grid;
				self.snapping_state.grid_snapping = true;
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			DocumentMessage::GridOverlays(mut overlay_context) => {
				if self.snapping_state.grid_snapping {
					grid_overlay(self, &mut overlay_context)
				}
			}
			DocumentMessage::GridVisible(enabled) => {
				self.snapping_state.grid_snapping = enabled;
				responses.add(OverlaysMessage::Draw);
			}
			DocumentMessage::GroupSelectedLayers => {
				let parent = self
					.metadata()
					.deepest_common_ancestor(self.selected_nodes.selected_layers(self.metadata()), false)
					.unwrap_or(LayerNodeIdentifier::ROOT);
				// Cancel grouping layers across different artboards
				// TODO: Group each set of layers for each artboard separately
				if parent == LayerNodeIdentifier::ROOT {
					return;
				}

				// Move layers in nested unselected folders above the first unselected parent folder
				let selected_layers = self.selected_nodes.selected_layers(self.metadata()).collect::<Vec<_>>();
				for layer in selected_layers.clone() {
					let mut first_unselected_parent_folder = layer.parent(&self.metadata).expect("Layer should always have parent");

					// Find folder in parent child stack
					loop {
						// Loop until parent layer is deselected. Note that parent cannot be selected, since it is an ancestor of all selected layers
						if !selected_layers.iter().any(|selected_layer| *selected_layer == first_unselected_parent_folder) {
							break;
						}
						let Some(new_folder) = first_unselected_parent_folder.parent(&self.metadata) else {
							log::error!("Layer should always have parent");
							return;
						};
						first_unselected_parent_folder = new_folder;
					}
					// Don't move nodes above new group folder parent
					if first_unselected_parent_folder == parent {
						continue;
					}

					// Disconnect above and below the old layer location
					self.disconnect_node(layer, responses);

					// Move disconnected node to folder
					let folder_position = self
						.network
						.nodes
						.get(&first_unselected_parent_folder.to_node())
						.expect("Current folder should always exist")
						.metadata
						.position;
					let Some(layer_to_move_node_mut) = self.network.nodes.get_mut(&layer.to_node()) else {
						return;
					};
					DocumentMessageHandler::disconnect_input(layer_to_move_node_mut, 0);
					layer_to_move_node_mut.metadata.position = folder_position;

					// Insert node right above the folder
					// TODO: Use insert layer between message
					let Some((folder_downstream_node_id, folder_downstream_input_index)) = DocumentMessageHandler::get_downstream_node(&self.network, &self.metadata, first_unselected_parent_folder)
					else {
						log::error!("Downstream node should always exist when inserting layer");
						return;
					};
					let downstream_input = self
						.network
						.nodes
						.get_mut(&folder_downstream_node_id)
						.and_then(|node| node.inputs.get_mut(folder_downstream_input_index));
					let Some(NodeInput::Node { node_id, .. }) = downstream_input else {
						log::error!("Downstream node should have a node input");
						return;
					};
					*node_id = layer.to_node();

					// Connect layer primary input to parent folder
					let Some(layer_node_input) = self.network.nodes.get_mut(&layer.to_node()).and_then(|node| node.inputs.get_mut(0)) else {
						log::error!("Layer should always have primary input");
						return;
					};
					*layer_node_input = NodeInput::node(first_unselected_parent_folder.to_node(), 0);

					let upstream_shift = IVec2::new(0, 3);
					let mut modify_inputs = ModifyInputsContext::new(&mut self.network, &mut self.metadata, &mut self.node_graph_handler, responses);
					modify_inputs.shift_upstream(first_unselected_parent_folder.to_node(), upstream_shift, true);
				}

				let calculated_insert_index = parent.children(self.metadata()).enumerate().find_map(|(index, direct_child)| {
					if self.selected_nodes.selected_layers(self.metadata()).any(|selected| selected == direct_child) {
						return Some(index as isize);
					}

					for descendant in direct_child.descendants(self.metadata()) {
						if self.selected_nodes.selected_layers(self.metadata()).any(|selected| selected == descendant) {
							return Some(index as isize);
						}
					}

					None
				});

				let folder_id = NodeId(generate_uuid());
				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::NewCustomLayer {
					id: folder_id,
					nodes: HashMap::new(),
					parent,
					insert_index: calculated_insert_index.unwrap_or(-1),
					alias: String::new(),
				});

				// Create a vec of nodes to move with all selected layers in the parent layer child stack, as well as each non layer sibling directly upstream of the selected layer
				let mut nodes_to_move = Vec::new();

				// Skip over horizontal non layer node chain that feeds into parent
				let Some(mut current_stack_node_id) = parent.first_child(&self.metadata).and_then(|current_stack_node| Some(current_stack_node.to_node())) else {
					log::error!("Folder should always have child");
					return;
				};

				let current_stack_node_id_ref = &mut current_stack_node_id;

				loop {
					let mut current_stack_node = self.network.nodes.get(current_stack_node_id_ref).expect("Current stack node id should always be a node");
					// Check if the current stack node is a selected layer
					if self
						.selected_nodes
						.selected_layers(&self.metadata)
						.any(|selected_node_id| selected_node_id.to_node() == *current_stack_node_id_ref)
					{
						nodes_to_move.push(*current_stack_node_id_ref);
						// Push all non layer sibling nodes directly upstream of the selected layer
						loop {
							if let Some(NodeInput::Node { node_id, .. }) = current_stack_node.inputs.get(0) {
								let next_node = self.network.nodes.get(node_id).expect("Stack node id should always be a node");
								// If the next node is a layer, immediately break and leave current stack node as the non layer node
								if next_node.is_layer {
									break;
								}
								*current_stack_node_id_ref = *node_id;
								current_stack_node = next_node;
								nodes_to_move.push(*current_stack_node_id_ref);
							} else {
								break;
							}
						}
					}
					// Get next node
					if let Some(NodeInput::Node { node_id, .. }) = current_stack_node.inputs.get(0) {
						*current_stack_node_id_ref = *node_id;
					} else {
						break;
					}
				}

				responses.add(GraphOperationMessage::MoveUpstreamSiblingsToChild {
					new_parent: folder_id,
					upstream_sibling_ids: nodes_to_move,
				});
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![folder_id] });

				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(DocumentMessage::DocumentStructureChanged);
				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::ImaginateGenerate => responses.add(PortfolioMessage::SubmitGraphRender { document_id }),
			DocumentMessage::ImaginateRandom { imaginate_node, then_generate } => {
				// Generate a random seed. We only want values between -2^53 and 2^53, because integer values
				// outside of this range can get rounded in f64
				let random_bits = generate_uuid();
				let random_value = ((random_bits >> 11) as f64).copysign(f64::from_bits(random_bits & (1 << 63)));

				responses.add(DocumentMessage::StartTransaction);
				// Set a random seed input
				responses.add(NodeGraphMessage::SetInputValue {
					node_id: *imaginate_node.last().unwrap(),
					// Needs to match the index of the seed parameter in `pub const IMAGINATE_NODE: DocumentNodeDefinition` in `document_node_type.rs`
					input_index: 3,
					value: graph_craft::document::value::TaggedValue::F64(random_value),
				});

				// Generate the image
				if then_generate {
					responses.add(DocumentMessage::ImaginateGenerate);
				}
			}
			DocumentMessage::ImportSvg {
				id,
				svg,
				transform,
				parent,
				insert_index,
			} => {
				self.backup(responses);
				responses.add(GraphOperationMessage::NewSvg {
					id,
					svg,
					transform,
					parent,
					insert_index,
				});
			}
			DocumentMessage::MoveSelectedLayersTo { parent, insert_index } => {
				let selected_layers = self.selected_nodes.selected_layers(self.metadata()).collect::<Vec<_>>();
				// Disallow trying to insert into self.
				if selected_layers.iter().any(|&layer| parent.ancestors(self.metadata()).any(|ancestor| ancestor == layer)) {
					return;
				}
				// Artboards can only have the Output node as the parent
				if selected_layers.iter().any(|&layer| self.metadata.is_artboard(layer)) && parent != LayerNodeIdentifier::ROOT {
					return;
				}
				// Disallow inserting layers between artboards. Since only artboards can output to Output node, the layer parent cannot be the output
				if !selected_layers.iter().any(|&layer| self.metadata.is_artboard(layer)) && parent == LayerNodeIdentifier::ROOT {
					return;
				}
				let insert_index = self.update_insert_index(&selected_layers, parent, insert_index);

				let binding = self.metadata.shallowest_unique_layers(self.selected_nodes.selected_layers(&self.metadata));
				let get_last_elements = binding.iter().map(|x| x.last().expect("empty path")).collect::<Vec<_>>();
				let mut ordered_last_elements = self.metadata.all_layers().filter(|layer| get_last_elements.contains(&layer)).collect::<Vec<_>>();
				ordered_last_elements.reverse();

				for layer_to_move in ordered_last_elements.clone() {
					// Part 1: Disconnect layer to move and reconnect downstream node to upstream sibling if it exists

					self.disconnect_node(layer_to_move, responses);
					// Part 2: Reconnect layer_to_move to new parent at insert index
					let (post_node_id, pre_node_id, post_node_input_index) = ModifyInputsContext::get_post_node_with_index(&self.network, parent.to_node(), insert_index);

					// Layer_to_move should always correspond to a node
					let Some(layer_to_move_node) = self.network.nodes.get(&layer_to_move.to_node()) else {
						continue;
					};
					// Move current layer to post node
					let post_node = self.network.nodes.get(&post_node_id).expect("Post node id should always refer to a node");
					let current_position = layer_to_move_node.metadata.position;
					let new_position = post_node.metadata.position;

					// If moved to top of a layer stack, move to the left of the post node. The stack will be shifted down later.
					// If moved within a stack, move directly on the post node. The rest of the stack will be shifted down later.
					let offset_to_post_node = if insert_index == 0 {
						new_position - current_position - IVec2::new(8, 0)
					} else {
						new_position - current_position
					};

					let mut modify_inputs = ModifyInputsContext::new(&mut self.network, &mut self.metadata, &mut self.node_graph_handler, responses);
					modify_inputs.shift_upstream(layer_to_move.to_node(), offset_to_post_node, true);

					// Update post_node input to layer_to_move
					// TODO: Use insert layer between message
					let post_node_mut = self.network.nodes.get_mut(&post_node_id).expect("Post node id should always refer to a node");
					if let Some(NodeInput::Node { node_id, .. }) = post_node_mut.inputs.get_mut(post_node_input_index) {
						*node_id = layer_to_move.to_node();
					} else if let Some(node_input) = post_node_mut.inputs.get_mut(post_node_input_index) {
						*node_input = NodeInput::node(layer_to_move.to_node(), 0);
					}

					let Some(layer_to_move_node_mut) = self.network.nodes.get_mut(&layer_to_move.to_node()) else {
						continue;
					};

					if let Some(pre_node_id) = pre_node_id {
						// If pre node exists, connect layer_to_move sibling input to that node.
						if let Some(node_input) = layer_to_move_node_mut.inputs.get_mut(0) {
							*node_input = NodeInput::node(pre_node_id, 0);
						}
					}

					// shift stack down, starting at the moved node.
					let mut modify_inputs: ModifyInputsContext = ModifyInputsContext::new(&mut self.network, &mut self.metadata, &mut self.node_graph_handler, responses);
					let shift = IVec2::new(0, 3);
					modify_inputs.shift_upstream(layer_to_move.to_node(), shift, true);

					self.metadata.load_structure(&self.network, &mut self.selected_nodes);
				}

				if ordered_last_elements.iter().any(|layer_to_move| self.network.connected_to_output(layer_to_move.to_node())) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(DocumentMessage::DocumentStructureChanged);
				responses.add(NodeGraphMessage::SendGraph);

				// responses.add(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
				// responses.add(DocumentMessage::DeleteSelectedLayers);
				// responses.add(PortfolioMessage::PasteIntoFolder {
				// 	clipboard: Clipboard::Internal,
				// 	parent,
				// 	insert_index,
				// });
			}
			DocumentMessage::NudgeSelectedLayers {
				delta_x,
				delta_y,
				resize,
				resize_opposite_corner,
			} => {
				self.backup(responses);

				let opposite_corner = ipp.keyboard.key(resize_opposite_corner);
				let delta = DVec2::new(delta_x, delta_y);

				match ipp.keyboard.key(resize) {
					// Nudge translation
					false => {
						for layer in self
							.selected_nodes
							.selected_layers(self.metadata())
							.filter(|&layer| self.selected_nodes.layer_visible(layer, self.metadata()) && !self.selected_nodes.layer_locked(layer, self.metadata()))
						{
							responses.add(GraphOperationMessage::TransformChange {
								layer,
								transform: DAffine2::from_translation(delta),
								transform_in: TransformIn::Local,
								skip_rerender: false,
							});
						}
					}
					// Nudge resize
					true => {
						let selected_bounding_box = self.metadata().selected_bounds_document_space(false, &self.selected_nodes);
						let Some([existing_top_left, existing_bottom_right]) = selected_bounding_box else { return };

						let size = existing_bottom_right - existing_top_left;
						let new_size = size + if opposite_corner { -delta } else { delta };
						let enlargement_factor = new_size / size;

						let position = existing_top_left + if opposite_corner { delta } else { DVec2::ZERO };
						let mut pivot = (existing_top_left * enlargement_factor - position) / (enlargement_factor - DVec2::splat(1.));
						if !pivot.x.is_finite() {
							pivot.x = 0.;
						}
						if !pivot.y.is_finite() {
							pivot.y = 0.;
						}

						let scale = DAffine2::from_scale(enlargement_factor);
						let pivot = DAffine2::from_translation(pivot);
						let transformation = pivot * scale * pivot.inverse();

						for layer in self
							.selected_nodes
							.selected_layers(self.metadata())
							.filter(|&layer| self.selected_nodes.layer_visible(layer, self.metadata()) && !self.selected_nodes.layer_locked(layer, self.metadata()))
						{
							let to = self.metadata().document_to_viewport.inverse() * self.metadata().downstream_transform_to_viewport(layer);
							let original_transform = self.metadata().upstream_transform(layer.to_node());
							let new = to.inverse() * transformation * to * original_transform;
							responses.add(GraphOperationMessage::TransformSet {
								layer,
								transform: new,
								transform_in: TransformIn::Local,
								skip_rerender: false,
							});
						}
					}
				}
			}
			DocumentMessage::PasteImage { image, mouse } => {
				// All the image's pixels have been converted to 0..=1, linear, and premultiplied by `Color::from_rgba8_srgb`

				let image_size = DVec2::new(image.width as f64, image.height as f64);

				// Align the layer with the mouse or center of viewport
				let viewport_location = mouse.map_or(ipp.viewport_bounds.center() + ipp.viewport_bounds.top_left, |pos| pos.into());
				let center_in_viewport = DAffine2::from_translation(self.metadata().document_to_viewport.inverse().transform_point2(viewport_location - ipp.viewport_bounds.top_left));
				let center_in_viewport_layerspace = center_in_viewport;

				// Scale the image to fit into a 512x512 box
				let image_size = image_size / DVec2::splat((image_size.max_element() / 512.).max(1.));

				// Make layer the size of the image
				let fit_image_size = DAffine2::from_scale_angle_translation(image_size, 0., image_size / -2.);

				let transform = center_in_viewport_layerspace * fit_image_size;

				responses.add(DocumentMessage::StartTransaction);

				let image_frame = ImageFrame { image, ..Default::default() };

				use crate::messages::tool::common_functionality::graph_modification_utils;
				let layer = graph_modification_utils::new_image_layer(image_frame, NodeId(generate_uuid()), self.new_layer_parent(), responses);

				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });

				responses.add(GraphOperationMessage::TransformSet {
					layer,
					transform,
					transform_in: TransformIn::Local,
					skip_rerender: false,
				});

				// Force chosen tool to be Select Tool after importing image.
				responses.add(ToolMessage::ActivateTool { tool_type: ToolType::Select });
			}
			DocumentMessage::PasteSvg { svg, mouse } => {
				use crate::messages::tool::common_functionality::graph_modification_utils;
				let viewport_location = mouse.map_or(ipp.viewport_bounds.center() + ipp.viewport_bounds.top_left, |pos| pos.into());
				let center_in_viewport = DAffine2::from_translation(self.metadata().document_to_viewport.inverse().transform_point2(viewport_location - ipp.viewport_bounds.top_left));
				let layer = graph_modification_utils::new_svg_layer(svg, center_in_viewport, NodeId(generate_uuid()), self.new_layer_parent(), responses);
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
				responses.add(ToolMessage::ActivateTool { tool_type: ToolType::Select });
			}
			DocumentMessage::Redo => {
				responses.add(SelectToolMessage::Abort);
				responses.add(DocumentMessage::DocumentHistoryForward);
				responses.add(ToolMessage::Redo);
				responses.add(OverlaysMessage::Draw);
			}
			DocumentMessage::RenameDocument { new_name } => {
				self.name = new_name;
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				responses.add(NodeGraphMessage::UpdateNewNodeGraph);
			}
			DocumentMessage::RenderRulers => {
				let document_transform_scale = self.navigation_handler.snapped_scale(self.navigation.zoom);

				let ruler_origin = self.metadata().document_to_viewport.transform_point2(DVec2::ZERO);
				let log = document_transform_scale.log2();
				let ruler_interval = if log < 0. { 100. * 2_f64.powf(-log.ceil()) } else { 100. / 2_f64.powf(log.ceil()) };
				let ruler_spacing = ruler_interval * document_transform_scale;

				responses.add(FrontendMessage::UpdateDocumentRulers {
					origin: ruler_origin.into(),
					spacing: ruler_spacing,
					interval: ruler_interval,
					visible: self.rulers_visible,
				});
			}
			DocumentMessage::RenderScrollbars => {
				let document_transform_scale = self.navigation_handler.snapped_scale(self.navigation.zoom);

				let scale = 0.5 + ASYMPTOTIC_EFFECT + document_transform_scale * SCALE_EFFECT;

				let viewport_size = ipp.viewport_bounds.size();
				let viewport_mid = ipp.viewport_bounds.center();
				let [bounds1, bounds2] = self.metadata().document_bounds_viewport_space().unwrap_or([viewport_mid; 2]);
				let bounds1 = bounds1.min(viewport_mid) - viewport_size * scale;
				let bounds2 = bounds2.max(viewport_mid) + viewport_size * scale;
				let bounds_length = (bounds2 - bounds1) * (1. + SCROLLBAR_SPACING);
				let scrollbar_position = DVec2::splat(0.5) - (bounds1.lerp(bounds2, 0.5) - viewport_mid) / (bounds_length - viewport_size);
				let scrollbar_multiplier = bounds_length - viewport_size;
				let scrollbar_size = viewport_size / bounds_length;

				responses.add(FrontendMessage::UpdateDocumentScrollbars {
					position: scrollbar_position.into(),
					size: scrollbar_size.into(),
					multiplier: scrollbar_multiplier.into(),
				});
			}
			DocumentMessage::SaveDocument => {
				self.set_save_state(true);
				responses.add(PortfolioMessage::AutoSaveActiveDocument);
				// Update the save status of the just saved document
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);

				let name = match self.name.ends_with(FILE_SAVE_SUFFIX) {
					true => self.name.clone(),
					false => self.name.clone() + FILE_SAVE_SUFFIX,
				};
				responses.add(FrontendMessage::TriggerDownloadTextFile {
					document: self.serialize_document(),
					name,
				})
			}
			DocumentMessage::SelectAllLayers => {
				let metadata = self.metadata();
				let all_layers_except_artboards_invisible_and_locked = metadata
					.all_layers()
					.filter(move |&layer| !metadata.is_artboard(layer))
					.filter(|&layer| self.selected_nodes.layer_visible(layer, metadata) && !self.selected_nodes.layer_locked(layer, metadata));
				let nodes = all_layers_except_artboards_invisible_and_locked.map(|layer| layer.to_node()).collect();
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
			}
			DocumentMessage::SelectedLayersLower => {
				responses.add(DocumentMessage::SelectedLayersReorder { relative_index_offset: 1 });
			}
			DocumentMessage::SelectedLayersLowerToBack => {
				responses.add(DocumentMessage::SelectedLayersReorder { relative_index_offset: isize::MAX });
			}
			DocumentMessage::SelectedLayersRaise => {
				responses.add(DocumentMessage::SelectedLayersReorder { relative_index_offset: -1 });
			}
			DocumentMessage::SelectedLayersRaiseToFront => {
				responses.add(DocumentMessage::SelectedLayersReorder { relative_index_offset: isize::MIN });
			}
			DocumentMessage::SelectedLayersReorder { relative_index_offset } => {
				self.selected_layers_reorder(relative_index_offset, responses);
			}
			DocumentMessage::SelectLayer { id, ctrl, shift } => {
				let layer = LayerNodeIdentifier::new(id, self.network());

				let mut nodes = vec![];

				// If we have shift pressed and a layer already selected then fill the range
				if let Some(last_selected) = self.layer_range_selection_reference.filter(|_| shift) {
					nodes.push(last_selected.to_node());
					nodes.push(id);

					// Fill the selection range
					self.metadata()
						.all_layers()
						.skip_while(|&node| node != layer && node != last_selected)
						.skip(1)
						.take_while(|&node| node != layer && node != last_selected)
						.for_each(|node| nodes.push(node.to_node()));
				} else {
					if ctrl {
						// Toggle selection when holding ctrl
						if self.selected_nodes.selected_layers_contains(layer, self.metadata()) {
							responses.add_front(NodeGraphMessage::SelectedNodesRemove { nodes: vec![id] });
						} else {
							responses.add_front(NodeGraphMessage::SelectedNodesAdd { nodes: vec![id] });
						}
						responses.add(BroadcastEvent::SelectionChanged);
					} else {
						nodes.push(id);
					}

					// Set our last selection reference
					self.layer_range_selection_reference = Some(layer);
				}

				// Don't create messages for empty operations
				if !nodes.is_empty() {
					// Add or set our selected layers
					if ctrl {
						responses.add_front(NodeGraphMessage::SelectedNodesAdd { nodes });
					} else {
						responses.add_front(NodeGraphMessage::SelectedNodesSet { nodes });
					}
				}
			}
			DocumentMessage::SetBlendModeForSelectedLayers { blend_mode } => {
				self.backup(responses);
				for layer in self.selected_nodes.selected_layers_except_artboards(self.metadata()) {
					responses.add(GraphOperationMessage::BlendModeSet { layer, blend_mode });
				}
			}
			DocumentMessage::SetOpacityForSelectedLayers { opacity } => {
				self.backup(responses);
				let opacity = opacity.clamp(0., 1.);

				for layer in self.selected_nodes.selected_layers_except_artboards(self.metadata()) {
					responses.add(GraphOperationMessage::OpacitySet { layer, opacity });
				}
			}
			DocumentMessage::SetOverlaysVisibility { visible } => {
				self.overlays_visible = visible;
				responses.add(BroadcastEvent::ToolAbort);
				responses.add(OverlaysMessage::Draw);
			}
			DocumentMessage::SetRangeSelectionLayer { new_layer } => {
				self.layer_range_selection_reference = new_layer;
			}
			DocumentMessage::SetSnapping {
				snapping_enabled,
				bounding_box_snapping,
				geometry_snapping,
			} => {
				if let Some(state) = snapping_enabled {
					self.snapping_state.snapping_enabled = state
				};

				if let Some(OptionBoundsSnapping {
					edge_midpoints,
					edges,
					centers,
					corners,
				}) = bounding_box_snapping
				{
					if let Some(state) = edge_midpoints {
						self.snapping_state.bounds.edge_midpoints = state
					};
					if let Some(state) = edges {
						self.snapping_state.bounds.edges = state
					};
					if let Some(state) = centers {
						self.snapping_state.bounds.centers = state
					};
					if let Some(state) = corners {
						self.snapping_state.bounds.corners = state
					};
				}

				if let Some(OptionPointSnapping {
					paths,
					path_intersections,
					anchors,
					line_midpoints,
					normals,
					tangents,
				}) = geometry_snapping
				{
					if let Some(state) = path_intersections {
						self.snapping_state.nodes.path_intersections = state
					};
					if let Some(state) = paths {
						self.snapping_state.nodes.paths = state
					};
					if let Some(state) = anchors {
						self.snapping_state.nodes.anchors = state
					};
					if let Some(state) = line_midpoints {
						self.snapping_state.nodes.line_midpoints = state
					};
					if let Some(state) = normals {
						self.snapping_state.nodes.normals = state
					};
					if let Some(state) = tangents {
						self.snapping_state.nodes.tangents = state
					};
				}
			}
			DocumentMessage::SetViewMode { view_mode } => {
				self.view_mode = view_mode;
				responses.add_front(NodeGraphMessage::RunDocumentGraph);
			}
			DocumentMessage::StartTransaction => self.backup(responses),
			DocumentMessage::ToggleLayerExpansion { id } => {
				let layer = LayerNodeIdentifier::new(id, self.network());
				if self.collapsed.0.contains(&layer) {
					self.collapsed.0.retain(|&collapsed_layer| collapsed_layer != layer);
				} else {
					self.collapsed.0.push(layer);
				}
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			DocumentMessage::Undo => {
				self.undo_in_progress = true;
				responses.add(ToolMessage::PreUndo);
				responses.add(DocumentMessage::DocumentHistoryBackward);
				responses.add(OverlaysMessage::Draw);
				responses.add(DocumentMessage::UndoFinished);
				responses.add(ToolMessage::Undo);
			}
			DocumentMessage::UndoFinished => self.undo_in_progress = false,
			DocumentMessage::UngroupSelectedLayers => {
				responses.add(DocumentMessage::StartTransaction);

				let folder_paths = self.metadata().folders_sorted_by_most_nested(self.selected_nodes.selected_layers(self.metadata()));
				// let mut ungrouped_folders = HashSet::new();

				for folder in folder_paths.clone() {
					// Cannot ungroup artboard
					let folder_node = self.network.nodes.get(&folder.to_node()).expect("Folder node should always exist");
					if folder_node.is_artboard() {
						return;
					}

					// Get first child layer node that feeds into the secondary input for the folder
					let Some(child_layer) = folder.first_child(&self.metadata) else {
						log::error!("Folder should always have a child");
						return;
					};
					let child_layer_node_id = child_layer.to_node();

					// Move child_layer stack x position to folder stack
					{
						let child_layer_node = self.network.nodes.get(&child_layer_node_id).expect("Child node should always exist for layer");
						let offset = folder_node.metadata.position - child_layer_node.metadata.position;
						let mut modify_inputs = ModifyInputsContext::new(&mut self.network, &mut self.metadata, &mut self.node_graph_handler, responses);
						modify_inputs.shift_upstream(child_layer_node_id, offset, true)
					}

					// Set the input for the node downstream of folder to the first layer node
					let Some((downstream_node_id, downstream_input_index)) = DocumentMessageHandler::get_downstream_node(&self.network, &self.metadata, folder) else {
						log::error!("Downstream node should always exist when moving layer");
						continue;
					};
					let Some(NodeInput::Node { node_id, .. }) = self
						.network
						.nodes
						.get_mut(&downstream_node_id)
						.expect("downstream node should always exist")
						.inputs
						.get_mut(downstream_input_index)
					else {
						log::error!("Could not get downstream node input");
						continue;
					};
					*node_id = child_layer_node_id;

					// Get the node that feeds into the primary input for the folder (if it exists)
					if let Some(NodeInput::Node { node_id, .. }) = self.network.nodes.get(&folder.to_node()).expect("Folder should always exist").inputs.get(0) {
						let upstream_sibling_id = *node_id;

						// Get the node at the bottom of the first layer node stack
						let mut last_child_node_id = child_layer_node_id;
						loop {
							if let Some(NodeInput::Node { node_id, .. }) = self.network.nodes.get(&last_child_node_id).expect("Child node should always exist").inputs.get(0) {
								last_child_node_id = *node_id;
							} else {
								break;
							}
						}

						// Connect the primary input of the bottom layer to the node to the upstream sibling
						let Some(node_input) = self.network.nodes.get_mut(&last_child_node_id).expect("Last child node should always exist").inputs.get_mut(0) else {
							log::error!("Could not get last child node primary input");
							continue;
						};
						*node_input = NodeInput::node(upstream_sibling_id, 0);

						// Shift upstream_sibling down by the height of the child layer stack
						let top_of_stack = self.network.nodes.get(&child_layer_node_id).expect("Child layer should always exist for child layer id");
						let bottom_of_stack = self.network.nodes.get(&last_child_node_id).expect("Last child layer should always exist for last child layer id");
						let target_distance = bottom_of_stack.metadata.position.y - top_of_stack.metadata.position.y;

						let folder_node = self.network.nodes.get(&folder.to_node()).expect("Folder node should always exist");
						let upstream_sibling_node = self.network.nodes.get(&upstream_sibling_id).expect("Upstream sibling node should always exist");
						let current_distance = upstream_sibling_node.metadata.position.y - folder_node.metadata.position.y;

						let y_offset = target_distance - current_distance + 3;
						let mut modify_inputs = ModifyInputsContext::new(&mut self.network, &mut self.metadata, &mut self.node_graph_handler, responses);
						modify_inputs.shift_upstream(upstream_sibling_id, IVec2::new(0, y_offset), true);
					}

					// Delete folder and all horizontal inputs
					responses.add(NodeGraphMessage::DeleteNodes {
						node_ids: vec![folder.to_node()],
						reconnect: true,
					});
				}
				if folder_paths.iter().any(|folder| self.network.connected_to_output(folder.to_node())) {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(DocumentMessage::DocumentStructureChanged);
				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::UpdateDocumentTransform { transform } => {
				self.metadata.document_to_viewport = transform;
				responses.add(DocumentMessage::RenderRulers);
				responses.add(DocumentMessage::RenderScrollbars);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			DocumentMessage::ZoomCanvasTo100Percent => {
				responses.add_front(NavigationMessage::SetCanvasZoom { zoom_factor: 1. });
			}
			DocumentMessage::ZoomCanvasTo200Percent => {
				responses.add_front(NavigationMessage::SetCanvasZoom { zoom_factor: 2. });
			}
			DocumentMessage::ZoomCanvasToFitAll => {
				if let Some(bounds) = self.metadata().document_bounds_document_space(true) {
					responses.add(NavigationMessage::SetCanvasTilt { angle_radians: 0. });
					responses.add(NavigationMessage::FitViewportToBounds { bounds, prevent_zoom_past_100: true });
				}
			}
			DocumentMessage::Noop => (),
		}
	}

	fn actions(&self) -> ActionList {
		unimplemented!("Must use `actions_with_graph_open` instead (unless we change every implementation of the MessageHandler trait).")
	}
}

impl DocumentMessageHandler {
	/// Runs an intersection test with all layers and a viewport space quad
	pub fn intersect_quad<'a>(&'a self, viewport_quad: graphene_core::renderer::Quad, network: &'a NodeNetwork) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
		let document_quad = self.metadata.document_to_viewport.inverse() * viewport_quad;
		self.metadata
			.root()
			.descendants(&self.metadata)
			.filter(|&layer| self.selected_nodes.layer_visible(layer, self.metadata()))
			.filter(|&layer| !self.selected_nodes.layer_locked(layer, self.metadata()))
			.filter(|&layer| !is_artboard(layer, network))
			.filter_map(|layer| self.metadata.click_target(layer).map(|targets| (layer, targets)))
			.filter(move |(layer, target)| target.iter().any(move |target| target.intersect_rectangle(document_quad, self.metadata.transform_to_document(*layer))))
			.map(|(layer, _)| layer)
	}

	/// Find all of the layers that were clicked on from a viewport space location
	pub fn click_xray(&self, viewport_location: DVec2) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		let point = self.metadata.document_to_viewport.inverse().transform_point2(viewport_location);
		self.metadata
			.root()
			.descendants(&self.metadata)
			.filter(|&layer| self.selected_nodes.layer_visible(layer, self.metadata()))
			.filter(|&layer| !self.selected_nodes.layer_locked(layer, self.metadata()))
			.filter_map(|layer| self.metadata.click_target(layer).map(|targets| (layer, targets)))
			.filter(move |(layer, target)| target.iter().any(|target: &ClickTarget| target.intersect_point(point, self.metadata.transform_to_document(*layer))))
			.map(|(layer, _)| layer)
	}

	/// Find the layer that has been clicked on from a viewport space location
	pub fn click(&self, viewport_location: DVec2, network: &NodeNetwork) -> Option<LayerNodeIdentifier> {
		self.click_xray(viewport_location).find(|&layer| !is_artboard(layer, network))
	}

	/// Get the combined bounding box of the click targets of the selected visible layers in viewport space
	pub fn selected_visible_layers_bounding_box_viewport(&self) -> Option<[DVec2; 2]> {
		self.selected_nodes
			.selected_visible_layers(self.metadata())
			.filter_map(|layer| self.metadata.bounding_box_viewport(layer))
			.reduce(graphene_core::renderer::Quad::combine_bounds)
	}

	pub fn selected_visible_and_unlock_layers_bounding_box_viewport(&self) -> Option<[DVec2; 2]> {
		self.selected_nodes
			.selected_visible_and_unlocked_layers(self.metadata())
			.filter_map(|layer| self.metadata.bounding_box_viewport(layer))
			.reduce(graphene_core::renderer::Quad::combine_bounds)
	}

	pub fn network(&self) -> &NodeNetwork {
		&self.network
	}

	pub fn metadata(&self) -> &DocumentMetadata {
		&self.metadata
	}

	pub fn serialize_document(&self) -> String {
		let val = serde_json::to_string(self);
		// We fully expect the serialization to succeed
		val.unwrap()
	}

	pub fn deserialize_document(serialized_content: &str) -> Result<Self, EditorError> {
		serde_json::from_str(serialized_content).map_err(|e| EditorError::DocumentDeserialization(e.to_string()))
	}

	pub fn with_name(name: String, ipp: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> Self {
		let mut document = Self { name, ..Self::default() };
		let transform = document.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.size() / 2., DVec2::ZERO, 0., 1.);
		document.metadata.document_to_viewport = transform;
		responses.add(DocumentMessage::UpdateDocumentTransform { transform });

		document
	}

	pub fn with_name_and_content(name: String, serialized_content: String) -> Result<Self, EditorError> {
		let mut document = Self::deserialize_document(&serialized_content)?;
		document.name = name;
		Ok(document)
	}

	/// Called recursively by the entry function [`serialize_root`].
	fn serialize_structure(&self, folder: LayerNodeIdentifier, structure_section: &mut Vec<u64>, data_section: &mut Vec<u64>, path: &mut Vec<LayerNodeIdentifier>) {
		let mut space = 0;
		for layer_node in folder.children(self.metadata()) {
			data_section.push(layer_node.to_node().0);
			space += 1;
			if layer_node.has_children(self.metadata()) && !self.collapsed.0.contains(&layer_node) {
				path.push(layer_node);

				// TODO: Skip if folder is not expanded.
				structure_section.push(space);
				self.serialize_structure(layer_node, structure_section, data_section, path);
				space = 0;

				path.pop();
			}
		}
		structure_section.push(space | 1 << 63);
	}

	/// Serializes the layer structure into a condensed 1D structure.
	///
	/// # Format
	/// It is a string of numbers broken into three sections:
	///
	/// | Data                                                                                                                          | Description                                                   | Length           |
	/// |------------------------------------------------------------------------------------------------------------------------------ |---------------------------------------------------------------|------------------|
	/// | `4,` `2, 1, -2, -0,` `16533113728871998040,3427872634365736244,18115028555707261608,15878401910454357952,449479075714955186`  | Encoded example data                                          |                  |
	/// | _____________________________________________________________________________________________________________________________ | _____________________________________________________________ | ________________ |
	/// | **Length** section: `4`                                                                                                       | Length of the **Structure** section (`L` = `structure.len()`) | First value      |
	/// | **Structure** section: `2, 1, -2, -0`                                                                                         | The **Structure** section                                     | Next `L` values  |
	/// | **Data** section: `16533113728871998040, 3427872634365736244, 18115028555707261608, 15878401910454357952, 449479075714955186` | The **Data** section (layer IDs)                              | Remaining values |
	///
	/// The data section lists the layer IDs for all folders/layers in the tree as read from top to bottom.
	/// The structure section lists signed numbers. The sign indicates a folder indentation change (`+` is down a level, `-` is up a level).
	/// The numbers in the structure block encode the indentation. For example:
	/// - `2` means read two elements from the data section, then place a `[`.
	/// - `-x` means read `x` elements from the data section and then insert a `]`.
	///
	/// ```text
	/// 2     V 1  V -2  A -0 A
	/// 16533113728871998040,3427872634365736244,  18115028555707261608, 15878401910454357952,449479075714955186
	/// 16533113728871998040,3427872634365736244,[ 18115028555707261608,[15878401910454357952,449479075714955186]    ]
	/// ```
	///
	/// Resulting layer panel:
	/// ```text
	/// 16533113728871998040
	/// 3427872634365736244
	/// [3427872634365736244,18115028555707261608]
	/// [3427872634365736244,18115028555707261608,15878401910454357952]
	/// [3427872634365736244,18115028555707261608,449479075714955186]
	/// ```
	pub fn serialize_root(&self) -> RawBuffer {
		let mut structure_section = vec![LayerNodeIdentifier::ROOT.to_node().0];
		let mut data_section = Vec::new();
		self.serialize_structure(self.metadata().root(), &mut structure_section, &mut data_section, &mut vec![]);

		// Remove the ROOT element. Prepend `L`, the length (excluding the ROOT) of the structure section (which happens to be where the ROOT element was).
		structure_section[0] = structure_section.len() as u64 - 1;
		// Append the data section to the end.
		structure_section.extend(data_section);

		structure_section.as_slice().into()
	}

	/// Places a document into the history system
	fn backup_with_document(&mut self, network: NodeNetwork, responses: &mut VecDeque<Message>) {
		self.document_redo_history.clear();
		self.document_undo_history.push_back(network);
		if self.document_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			self.document_undo_history.pop_front();
		}

		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
	}

	/// Copies the entire document into the history system
	pub fn backup(&mut self, responses: &mut VecDeque<Message>) {
		self.backup_with_document(self.network.clone(), responses);
	}

	// TODO: Is this now redundant?
	/// Push a message backing up the document in its current state
	pub fn backup_nonmut(&self, responses: &mut VecDeque<Message>) {
		responses.add(DocumentMessage::BackupDocument { network: self.network.clone() });
	}

	/// Replace the document with a new document save, returning the document save.
	pub fn replace_document(&mut self, network: NodeNetwork) -> NodeNetwork {
		std::mem::replace(&mut self.network, network)
	}

	pub fn undo_with_history(&mut self, responses: &mut VecDeque<Message>) {
		let Some(previous_network) = self.undo(responses) else { return };

		self.document_redo_history.push_back(previous_network);
		if self.document_redo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			self.document_redo_history.pop_front();
		}
	}
	pub fn undo(&mut self, responses: &mut VecDeque<Message>) -> Option<NodeNetwork> {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		// If there is no history return and don't broadcast SelectionChanged
		let Some(network) = self.document_undo_history.pop_back() else { return None };

		responses.add(BroadcastEvent::SelectionChanged);

		let previous_network = std::mem::replace(&mut self.network, network);
		Some(previous_network)
	}
	pub fn redo(&mut self, responses: &mut VecDeque<Message>) -> Option<NodeNetwork> {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		// If there is no history return and don't broadcast SelectionChanged
		let Some(network) = self.document_redo_history.pop_back() else { return None };

		responses.add(BroadcastEvent::SelectionChanged);

		let previous_network = std::mem::replace(&mut self.network, network);
		Some(previous_network)
	}
	pub fn redo_with_history(&mut self, responses: &mut VecDeque<Message>) {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		let Some(previous_network) = self.redo(responses) else { return };

		self.document_undo_history.push_back(previous_network);
		if self.document_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			self.document_undo_history.pop_front();
		}
	}

	pub fn current_hash(&self) -> Option<u64> {
		self.document_undo_history.iter().last().map(|network| network.current_hash())
	}

	pub fn is_auto_saved(&self) -> bool {
		self.current_hash() == self.auto_saved_hash
	}

	pub fn is_saved(&self) -> bool {
		self.current_hash() == self.saved_hash
	}

	pub fn is_graph_overlay_open(&self) -> bool {
		self.graph_view_overlay_open
	}

	pub fn set_auto_save_state(&mut self, is_saved: bool) {
		if is_saved {
			self.auto_saved_hash = self.current_hash();
		} else {
			self.auto_saved_hash = None;
		}
	}

	pub fn set_save_state(&mut self, is_saved: bool) {
		if is_saved {
			self.saved_hash = self.current_hash();
		} else {
			self.saved_hash = None;
		}
	}
	// TODO: Replace with disconnect message
	pub fn disconnect_input(layer_to_disconnect_node: &mut DocumentNode, input_index: usize) {
		let Some(node_type) = resolve_document_node_type(&layer_to_disconnect_node.name) else {
			warn!("Node {} not in library", layer_to_disconnect_node.name);
			return;
		};
		let Some(existing_input) = layer_to_disconnect_node.inputs.get_mut(input_index) else {
			warn!("Node does not have and input at the selected index");
			return;
		};
		let mut default_input = node_type.inputs[input_index].default.clone();
		if let NodeInput::Value { exposed, .. } = &mut default_input {
			*exposed = existing_input.is_exposed();
		}
		*existing_input = default_input;
	}

	pub fn get_downstream_node(network: &NodeNetwork, metadata: &DocumentMetadata, layer_to_move: LayerNodeIdentifier) -> Option<(NodeId, usize)> {
		let mut downstream_layer = None;
		if let Some(previous_sibling) = layer_to_move.previous_sibling(metadata) {
			downstream_layer = Some((previous_sibling.to_node(), false))
		} else if let Some(parent) = layer_to_move.parent(metadata) {
			downstream_layer = Some((parent.to_node(), true))
		};

		// Downstream layer should always exist
		let Some((downstream_layer_node_id, downstream_layer_is_parent)) = downstream_layer else {
			return None;
		};

		// Horizontal traversal if layer_to_move is the top of its layer stack, primary traversal if not
		let flow_type = if downstream_layer_is_parent {
			graph_craft::document::FlowType::HorizontalFlow
		} else {
			graph_craft::document::FlowType::PrimaryFlow
		};

		network
			.upstream_flow_back_from_nodes(vec![downstream_layer_node_id], flow_type)
			.find(|(node, node_id)| {
				// Get secondary input only if it is the downstream_layer_node_id, the parent of layer to move, and a layer node (parent might be output)
				let is_parent_layer = downstream_layer_is_parent && downstream_layer_node_id == *node_id && node.is_layer;
				let node_input_index = if is_parent_layer { 1 } else { 0 };
				node.inputs.get(node_input_index).is_some_and(|node_input| {
					if let NodeInput::Node { node_id, .. } = node_input {
						*node_id == layer_to_move.to_node()
					} else {
						false
					}
				})
			})
			.map(|(downstream_node, downstream_node_id)| {
				let is_parent_layer = downstream_layer_is_parent && downstream_layer_node_id == downstream_node_id && downstream_node.is_layer;
				let downstream_input_index = if is_parent_layer { 1 } else { 0 };
				(downstream_node_id, downstream_input_index)
			})
	}

	// TODO: move into message
	pub fn disconnect_node(&mut self, layer_to_disconnect: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		let Some((downstream_node_id, downstream_input_index)) = DocumentMessageHandler::get_downstream_node(&self.network, &self.metadata, layer_to_disconnect) else {
			log::error!("Downstream node should always exist when moving layer");
			return;
		};
		let layer_to_move_sibling_input = self.network.nodes.get(&layer_to_disconnect.to_node()).and_then(|node| node.inputs.get(0));
		if let Some(NodeInput::Node { node_id, .. }) = layer_to_move_sibling_input {
			let upstream_sibling_id = node_id.clone();
			let Some(downstream_node) = self.network.nodes.get_mut(&downstream_node_id) else {
				return;
			};
			if let Some(NodeInput::Node { node_id, .. }) = downstream_node.inputs.get_mut(downstream_input_index) {
				*node_id = upstream_sibling_id;
			}
			let upstream_shift = IVec2::new(0, -3);
			let mut modify_inputs = ModifyInputsContext::new(&mut self.network, &mut self.metadata, &mut self.node_graph_handler, responses);
			modify_inputs.shift_upstream(upstream_sibling_id, upstream_shift, true);
		} else {
			// Disconnect node directly downstream if upstream sibling doesn't exist
			let Some(downstream_node) = self.network.nodes.get_mut(&downstream_node_id) else {
				return;
			};
			DocumentMessageHandler::disconnect_input(downstream_node, downstream_input_index);
		}
		{
			let Some(layer_to_move_node_mut) = self.network.nodes.get_mut(&layer_to_disconnect.to_node()) else {
				return;
			};
			DocumentMessageHandler::disconnect_input(layer_to_move_node_mut, 0);
		}
	}
	/// When working with an insert index, deleting the layers may cause the insert index to point to a different location (if the layer being deleted was located before the insert index).
	///
	/// This function updates the insert index so that it points to the same place after the specified `layers` are deleted.
	fn update_insert_index(&self, layers: &[LayerNodeIdentifier], parent: LayerNodeIdentifier, insert_index: isize) -> usize {
		let take_amount = if insert_index < 0 { usize::MAX } else { insert_index as usize };
		let layer_ids_above = parent.children(self.metadata()).take(take_amount);
		layer_ids_above.filter(|layer_id| !layers.contains(layer_id)).count() as usize
	}

	/// Finds the parent folder which, based on the current selections, should be the container of any newly added layers.
	pub fn new_layer_parent(&self) -> LayerNodeIdentifier {
		self.metadata()
			.deepest_common_ancestor(self.selected_nodes.selected_layers(self.metadata()), true)
			.unwrap_or_else(|| self.metadata().active_artboard())
	}

	/// Loads layer resources such as creating the blob URLs for the images and loading all of the fonts in the document.
	pub fn load_layer_resources(&self, responses: &mut VecDeque<Message>) {
		let mut fonts = HashSet::new();
		for (_node_id, node) in self.network.recursive_nodes() {
			for input in &node.inputs {
				if let NodeInput::Value {
					tagged_value: TaggedValue::Font(font),
					..
				} = input
				{
					fonts.insert(font.clone());
				}
			}
		}
		for font in fonts {
			responses.add_front(FrontendMessage::TriggerFontLoad { font, is_default: false });
		}
	}

	pub fn update_document_widgets(&self, responses: &mut VecDeque<Message>) {
		// Document mode (dropdown menu at the left of the bar above the viewport, before the tool options)

		let document_mode_layout = WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![
				DropdownInput::new(
					vec![vec![
						MenuListEntry::new(format!("{:?}", DocumentMode::DesignMode))
							.label(DocumentMode::DesignMode.to_string())
							.icon(DocumentMode::DesignMode.icon_name()),
						MenuListEntry::new(format!("{:?}", DocumentMode::SelectMode))
							.label(DocumentMode::SelectMode.to_string())
							.icon(DocumentMode::SelectMode.icon_name())
							.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(330) }.into()),
						MenuListEntry::new(format!("{:?}", DocumentMode::GuideMode))
							.label(DocumentMode::GuideMode.to_string())
							.icon(DocumentMode::GuideMode.icon_name())
							.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(331) }.into()),
					]])
					.selected_index(Some(self.document_mode as u32))
					.draw_icon( true)
					.interactive( false) // TODO: set to true when dialogs are not spawned
					.widget_holder(),
				Separator::new(SeparatorType::Section).widget_holder(),
			],
		}]);

		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(document_mode_layout),
			layout_target: LayoutTarget::DocumentMode,
		});

		// Document bar (right portion of the bar above the viewport)

		let snapping_state = self.snapping_state.clone();

		let mut widgets = vec![
			CheckboxInput::new(self.overlays_visible)
				.icon("Overlays")
				.tooltip("Overlays")
				.on_update(|optional_input: &CheckboxInput| DocumentMessage::SetOverlaysVisibility { visible: optional_input.checked }.into())
				.widget_holder(),
			PopoverButton::new()
				.popover_layout(vec![
					LayoutGroup::Row {
						widgets: vec![TextLabel::new("Overlays").bold(true).widget_holder()],
					},
					LayoutGroup::Row {
						widgets: vec![TextLabel::new("Coming soon").widget_holder()],
					},
				])
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			CheckboxInput::new(snapping_state.snapping_enabled)
				.icon("Snapping")
				.tooltip("Snapping")
				.on_update(move |optional_input: &CheckboxInput| {
					let snapping_enabled = optional_input.checked;
					DocumentMessage::SetSnapping {
						snapping_enabled: Some(snapping_enabled),
						bounding_box_snapping: None,
						geometry_snapping: None,
					}
					.into()
				})
				.widget_holder(),
			PopoverButton::new()
				.popover_layout(
					[
						LayoutGroup::Row {
							widgets: vec![TextLabel::new("Snapping").bold(true).widget_holder()],
						},
						LayoutGroup::Row {
							widgets: vec![TextLabel::new(SnappingOptions::BoundingBoxes.to_string()).widget_holder()],
						},
					]
					.into_iter()
					.chain(
						[
							(BoundingBoxSnapTarget::Center, snapping_state.bounds.centers),
							(BoundingBoxSnapTarget::Corner, snapping_state.bounds.corners),
							(BoundingBoxSnapTarget::Edge, snapping_state.bounds.edges),
							(BoundingBoxSnapTarget::EdgeMidpoint, snapping_state.bounds.edge_midpoints),
						]
						.into_iter()
						.map(|(enum_type, bound_state)| LayoutGroup::Row {
							widgets: vec![
								CheckboxInput::new(bound_state)
									.on_update(move |input: &CheckboxInput| {
										DocumentMessage::SetSnapping {
											snapping_enabled: None,
											bounding_box_snapping: Some(OptionBoundsSnapping {
												edges: if enum_type == BoundingBoxSnapTarget::Edge { Some(input.checked) } else { None },
												edge_midpoints: if enum_type == BoundingBoxSnapTarget::EdgeMidpoint { Some(input.checked) } else { None },
												centers: if enum_type == BoundingBoxSnapTarget::Center { Some(input.checked) } else { None },
												corners: if enum_type == BoundingBoxSnapTarget::Corner { Some(input.checked) } else { None },
											}),
											geometry_snapping: None,
										}
										.into()
									})
									.widget_holder(),
								TextLabel::new(enum_type.to_string()).widget_holder(),
							],
						})
						.chain(
							[
								LayoutGroup::Row {
									widgets: vec![TextLabel::new(SnappingOptions::Geometry.to_string()).widget_holder()],
								},
								LayoutGroup::Row {
									widgets: vec![
										CheckboxInput::new(snapping_state.nodes.anchors)
											.on_update(move |input: &CheckboxInput| {
												DocumentMessage::SetSnapping {
													snapping_enabled: None,
													bounding_box_snapping: None,
													geometry_snapping: Some(OptionPointSnapping {
														anchors: Some(input.checked),
														..Default::default()
													}),
												}
												.into()
											})
											.widget_holder(),
										TextLabel::new("Anchor").widget_holder(),
									],
								},
							]
							.into_iter()
							.chain(
								[
									(GeometrySnapTarget::LineMidpoint, snapping_state.nodes.line_midpoints),
									(GeometrySnapTarget::Path, snapping_state.nodes.paths),
									(GeometrySnapTarget::Normal, snapping_state.nodes.normals),
									(GeometrySnapTarget::Tangent, snapping_state.nodes.tangents),
									(GeometrySnapTarget::Intersection, snapping_state.nodes.path_intersections),
								]
								.into_iter()
								.map(|(enum_type, bound_state)| LayoutGroup::Row {
									widgets: vec![
										CheckboxInput::new(bound_state)
											.on_update(move |input: &CheckboxInput| {
												DocumentMessage::SetSnapping {
													snapping_enabled: None,
													bounding_box_snapping: None,
													geometry_snapping: Some(OptionPointSnapping {
														anchors: None,
														line_midpoints: if enum_type == GeometrySnapTarget::LineMidpoint { Some(input.checked) } else { None },
														paths: if enum_type == GeometrySnapTarget::Path { Some(input.checked) } else { None },
														normals: if enum_type == GeometrySnapTarget::Normal { Some(input.checked) } else { None },
														tangents: if enum_type == GeometrySnapTarget::Tangent { Some(input.checked) } else { None },
														path_intersections: if enum_type == GeometrySnapTarget::Intersection { Some(input.checked) } else { None },
													}),
												}
												.into()
											})
											.widget_holder(),
										TextLabel::new(enum_type.to_string()).widget_holder(),
									],
								}),
							),
						),
					)
					.collect(),
				)
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			CheckboxInput::new(self.snapping_state.grid_snapping)
				.icon("Grid")
				.tooltip("Grid")
				.on_update(|optional_input: &CheckboxInput| DocumentMessage::GridVisible(optional_input.checked).into())
				.widget_holder(),
			PopoverButton::new()
				.popover_layout(overlay_options(&self.snapping_state.grid))
				.popover_min_width(Some(320))
				.widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(vec![
				RadioEntryData::new("normal")
					.icon("ViewModeNormal")
					.tooltip("View Mode: Normal")
					.on_update(|_| DocumentMessage::SetViewMode { view_mode: ViewMode::Normal }.into()),
				RadioEntryData::new("outline")
					.icon("ViewModeOutline")
					.tooltip("View Mode: Outline")
					.on_update(|_| DocumentMessage::SetViewMode { view_mode: ViewMode::Outline }.into()),
				RadioEntryData::new("pixels")
					.icon("ViewModePixels")
					.tooltip("View Mode: Pixels")
					.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(320) }.into()),
			])
			.selected_index(match self.view_mode {
				ViewMode::Normal => Some(0),
				_ => Some(1),
			})
			.widget_holder(),
			PopoverButton::new()
				.popover_layout(vec![
					LayoutGroup::Row {
						widgets: vec![TextLabel::new("View Mode").bold(true).widget_holder()],
					},
					LayoutGroup::Row {
						widgets: vec![TextLabel::new("Coming soon").widget_holder()],
					},
				])
				.widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			IconButton::new("ZoomIn", 24)
				.tooltip("Zoom In")
				.tooltip_shortcut(action_keys!(NavigationMessageDiscriminant::IncreaseCanvasZoom))
				.on_update(|_| NavigationMessage::IncreaseCanvasZoom { center_on_mouse: false }.into())
				.widget_holder(),
			IconButton::new("ZoomOut", 24)
				.tooltip("Zoom Out")
				.tooltip_shortcut(action_keys!(NavigationMessageDiscriminant::DecreaseCanvasZoom))
				.on_update(|_| NavigationMessage::DecreaseCanvasZoom { center_on_mouse: false }.into())
				.widget_holder(),
			IconButton::new("ZoomReset", 24)
				.tooltip("Reset Tilt and Zoom to 100%")
				.tooltip_shortcut(action_keys!(NavigationMessageDiscriminant::ResetCanvasTiltAndZoomTo100Percent))
				.on_update(|_| NavigationMessage::ResetCanvasTiltAndZoomTo100Percent.into())
				.widget_holder(),
			PopoverButton::new()
				.popover_layout(vec![
					LayoutGroup::Row {
						widgets: vec![TextLabel::new("Canvas Navigation").bold(true).widget_holder()],
					},
					LayoutGroup::Row {
						widgets: vec![TextLabel::new(
							"
								Interactive controls in this\n\
								menu are coming soon.\n\
								\n\
								Pan:\n\
								• Middle Click Drag\n\
								\n\
								Tilt:\n\
								• Alt + Middle Click Drag\n\
								\n\
								Zoom:\n\
								• Shift + Middle Click Drag\n\
								• Ctrl + Scroll Wheel Roll
							"
							.trim(),
						)
						.multiline(true)
						.widget_holder()],
					},
				])
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(self.navigation_handler.snapped_scale(self.navigation.zoom) * 100.))
				.unit("%")
				.min(0.000001)
				.max(1000000.)
				.tooltip("Document zoom within the viewport")
				.on_update(|number_input: &NumberInput| {
					NavigationMessage::SetCanvasZoom {
						zoom_factor: number_input.value.unwrap() / 100.,
					}
					.into()
				})
				.increment_behavior(NumberInputIncrementBehavior::Callback)
				.increment_callback_decrease(|_| NavigationMessage::DecreaseCanvasZoom { center_on_mouse: false }.into())
				.increment_callback_increase(|_| NavigationMessage::IncreaseCanvasZoom { center_on_mouse: false }.into())
				.widget_holder(),
		];

		let tilt_value = self.navigation_handler.snapped_angle(self.navigation.tilt) / (std::f64::consts::PI / 180.);
		if tilt_value.abs() > 0.00001 {
			widgets.extend([
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(tilt_value))
					.unit("°")
					.step(15.)
					.tooltip("Document tilt within the viewport")
					.on_update(|number_input: &NumberInput| {
						NavigationMessage::SetCanvasTilt {
							angle_radians: number_input.value.unwrap() * (std::f64::consts::PI / 180.),
						}
						.into()
					})
					.widget_holder(),
			]);
		}

		widgets.extend([
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextButton::new("Node Graph")
				.icon(Some((if self.graph_view_overlay_open { "GraphViewOpen" } else { "GraphViewClosed" }).into()))
				.hover_icon(Some((if self.graph_view_overlay_open { "GraphViewClosed" } else { "GraphViewOpen" }).into()))
				.tooltip(if self.graph_view_overlay_open { "Hide Node Graph" } else { "Show Node Graph" })
				.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::GraphViewOverlayToggle))
				.on_update(move |_| DocumentMessage::GraphViewOverlayToggle.into())
				.widget_holder(),
		]);

		let document_bar_layout = WidgetLayout::new(vec![LayoutGroup::Row { widgets }]);

		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(document_bar_layout),
			layout_target: LayoutTarget::DocumentBar,
		});
	}

	pub fn update_layers_panel_options_bar_widgets(&self, responses: &mut VecDeque<Message>) {
		// Get an iterator over the selected layers (excluding artboards which don't have an opacity or blend mode).
		let selected_layers_except_artboards = self.selected_nodes.selected_layers_except_artboards(self.metadata());

		// Look up the current opacity and blend mode of the selected layers (if any), and split the iterator into the first tuple and the rest.
		let mut opacity_and_blend_mode = selected_layers_except_artboards.map(|layer| (get_opacity(layer, &self.network).unwrap_or(100.), get_blend_mode(layer, &self.network).unwrap_or_default()));
		let first_opacity_and_blend_mode = opacity_and_blend_mode.next();
		let result_opacity_and_blend_mode = opacity_and_blend_mode;

		// If there are no selected layers, disable the opacity and blend mode widgets.
		let disabled = first_opacity_and_blend_mode.is_none();

		// Amongst the selected layers, check if the opacities and blend modes are identical across all layers.
		// The result is setting `option` and `blend_mode` to Some value if all their values are identical, or None if they are not.
		// If identical, we display the value in the widget. If not, we display a dash indicating dissimilarity.
		let (opacity, blend_mode) = first_opacity_and_blend_mode
			.map(|(first_opacity, first_blend_mode)| {
				let mut opacity_identical = true;
				let mut blend_mode_identical = true;

				for (opacity, blend_mode) in result_opacity_and_blend_mode {
					if (opacity - first_opacity).abs() > (f64::EPSILON * 100.) {
						opacity_identical = false;
					}
					if blend_mode != first_blend_mode {
						blend_mode_identical = false;
					}
				}

				(opacity_identical.then_some(first_opacity), blend_mode_identical.then_some(first_blend_mode))
			})
			.unwrap_or((None, None));

		let blend_mode_menu_entries = BlendMode::list_svg_subset()
			.iter()
			.map(|modes| {
				modes
					.iter()
					.map(|&blend_mode| {
						MenuListEntry::new(format!("{blend_mode:?}"))
							.label(blend_mode.to_string())
							.on_update(move |_| DocumentMessage::SetBlendModeForSelectedLayers { blend_mode }.into())
					})
					.collect()
			})
			.collect();

		let has_selection = self.selected_nodes.selected_layers(self.metadata()).next().is_some();
		let selection_all_visible = self.selected_nodes.selected_layers(self.metadata()).all(|layer| self.metadata().node_is_visible(layer.to_node()));
		let selection_all_locked = self.selected_nodes.selected_layers(self.metadata()).all(|layer| self.metadata().node_is_locked(layer.to_node()));

		let layers_panel_options_bar = WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![
				DropdownInput::new(blend_mode_menu_entries)
					.selected_index(blend_mode.and_then(|blend_mode| blend_mode.index_in_list_svg_subset()).map(|index| index as u32))
					.disabled(disabled)
					.draw_icon(false)
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(opacity)
					.label("Opacity")
					.unit("%")
					.display_decimal_places(2)
					.disabled(disabled)
					.min(0.)
					.max(100.)
					.range_min(Some(0.))
					.range_max(Some(100.))
					.mode_range()
					.on_update(|number_input: &NumberInput| {
						if let Some(value) = number_input.value {
							DocumentMessage::SetOpacityForSelectedLayers { opacity: value / 100. }.into()
						} else {
							Message::NoOp
						}
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
					.on_update(|_| DocumentMessage::GroupSelectedLayers.into())
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
					.disabled(!has_selection)
					.widget_holder(),
				IconButton::new(if selection_all_visible { "EyeVisible" } else { "EyeHidden" }, 24)
					.hover_icon(Some((if selection_all_visible { "EyeHide" } else { "EyeShow" }).into()))
					.tooltip(if selection_all_visible { "Hide Selected" } else { "Show Selected" })
					.tooltip_shortcut(action_keys!(NodeGraphMessageDiscriminant::ToggleSelectedVisibility))
					.on_update(|_| NodeGraphMessage::ToggleSelectedVisibility.into())
					.disabled(!has_selection)
					.widget_holder(),
			],
		}]);

		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(layers_panel_options_bar),
			layout_target: LayoutTarget::LayersPanelOptions,
		});
	}

	pub fn selected_layers_reorder(&mut self, relative_index_offset: isize, responses: &mut VecDeque<Message>) {
		self.backup(responses);

		let mut selected_layers = self.selected_nodes.selected_layers(self.metadata());

		let first_or_last_selected_layer = match relative_index_offset.signum() {
			-1 => selected_layers.next(),
			1 => selected_layers.last(),
			_ => panic!("selected_layers_reorder() must be given a non-zero value"),
		};

		let Some(pivot_layer) = first_or_last_selected_layer else {
			return;
		};
		let Some(parent) = pivot_layer.parent(self.metadata()) else {
			return;
		};

		let sibling_layer_paths: Vec<_> = parent.children(self.metadata()).collect();
		let Some(pivot_index) = sibling_layer_paths.iter().position(|path| *path == pivot_layer) else {
			return;
		};

		let max = sibling_layer_paths.len() as i64 - 1;
		let insert_index = (pivot_index as i64 + relative_index_offset as i64).clamp(0, max) as usize;

		let Some(&neighbor) = sibling_layer_paths.get(insert_index) else {
			return;
		};
		let Some(neighbor_index) = sibling_layer_paths.iter().position(|path| *path == neighbor) else {
			return;
		};

		// If moving down, insert below this layer. If moving up, insert above this layer.
		let insert_index = if relative_index_offset < 0 { neighbor_index } else { neighbor_index + 1 } as isize;
		responses.add(DocumentMessage::StartTransaction);
		responses.add(DocumentMessage::MoveSelectedLayersTo { parent, insert_index });
	}

	pub fn actions_with_graph_open(&self) -> ActionList {
		let mut common = actions!(DocumentMessageDiscriminant;
			Noop,
			Undo,
			Redo,
			SelectAllLayers,
			DeselectAllLayers,
			SaveDocument,
			SetSnapping,
			DebugPrintDocument,
			ZoomCanvasToFitAll,
			ZoomCanvasTo100Percent,
			ZoomCanvasTo200Percent,
			GraphViewOverlayToggle,
			CreateEmptyFolder,
		);

		if self.graph_view_overlay_open {
			let escape = actions!(DocumentMessageDiscriminant; GraphViewOverlay);
			common.extend(escape);
		}

		if self.selected_nodes.selected_layers(self.metadata()).next().is_some() {
			let select = actions!(DocumentMessageDiscriminant;
				DeleteSelectedLayers,
				DuplicateSelectedLayers,
				NudgeSelectedLayers,
				SelectedLayersLower,
				SelectedLayersLowerToBack,
				SelectedLayersRaise,
				SelectedLayersRaiseToFront,
				GroupSelectedLayers,
				UngroupSelectedLayers,
			);
			common.extend(select);
		}
		common.extend(self.navigation_handler.actions());
		common.extend(self.node_graph_handler.actions_with_node_graph_open(self.graph_view_overlay_open));
		common
	}
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			// ======================
			// Child message handlers
			// ======================
			navigation_handler: NavigationMessageHandler::default(),
			node_graph_handler: NodeGraphMessageHandler::default(),
			overlays_message_handler: OverlaysMessageHandler::default(),
			properties_panel_message_handler: PropertiesPanelMessageHandler::default(),
			// ============================================
			// Fields that are saved in the document format
			// ============================================
			network: root_network(),
			selected_nodes: SelectedNodes::default(),
			collapsed: CollapsedLayers::default(),
			name: DEFAULT_DOCUMENT_NAME.to_string(),
			commit_hash: GRAPHITE_GIT_COMMIT_HASH.to_string(),
			navigation: PTZ::default(),
			document_mode: DocumentMode::DesignMode,
			view_mode: ViewMode::default(),
			overlays_visible: true,
			rulers_visible: true,
			// =============================================
			// Fields omitted from the saved document format
			// =============================================
			document_undo_history: VecDeque::new(),
			document_redo_history: VecDeque::new(),
			saved_hash: None,
			auto_saved_hash: None,
			undo_in_progress: false,
			graph_view_overlay_open: false,
			snapping_state: SnappingState::default(),
			layer_range_selection_reference: None,
			metadata: Default::default(),
		}
	}
}

#[inline(always)]
fn default_network() -> NodeNetwork {
	DocumentMessageHandler::default().network
}
#[inline(always)]
fn default_selected_nodes() -> SelectedNodes {
	DocumentMessageHandler::default().selected_nodes
}
#[inline(always)]
fn default_collapsed() -> CollapsedLayers {
	DocumentMessageHandler::default().collapsed
}
#[inline(always)]
fn default_name() -> String {
	DocumentMessageHandler::default().name
}
#[inline(always)]
fn default_commit_hash() -> String {
	DocumentMessageHandler::default().commit_hash
}
#[inline(always)]
fn default_pan_tilt_zoom() -> PTZ {
	DocumentMessageHandler::default().navigation
}
#[inline(always)]
fn default_document_mode() -> DocumentMode {
	DocumentMessageHandler::default().document_mode
}
#[inline(always)]
fn default_view_mode() -> ViewMode {
	DocumentMessageHandler::default().view_mode
}
#[inline(always)]
fn default_overlays_visible() -> bool {
	DocumentMessageHandler::default().overlays_visible
}
#[inline(always)]
fn default_rulers_visible() -> bool {
	DocumentMessageHandler::default().rulers_visible
}

fn root_network() -> NodeNetwork {
	{
		let mut network = NodeNetwork::default();
		let node = graph_craft::document::DocumentNode {
			name: "Output".into(),
			inputs: vec![NodeInput::value(TaggedValue::GraphicGroup(Default::default()), true), NodeInput::Network(concrete!(WasmEditorApi))],
			implementation: graph_craft::document::DocumentNodeImplementation::Network(NodeNetwork {
				imports: vec![NodeId(3), NodeId(0)],
				exports: vec![NodeOutput::new(NodeId(3), 0)],
				nodes: [
					DocumentNode {
						name: "EditorApi".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Canvas".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::CreateSurfaceNode")),
						skip_deduplication: true,
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
						manual_composition: Some(concrete!(())),
						inputs: vec![NodeInput::node(NodeId(1), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
						..Default::default()
					},
					DocumentNode {
						name: "RenderNode".to_string(),
						inputs: vec![
							NodeInput::node(NodeId(0), 0),
							NodeInput::Network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(generic!(T)))),
							NodeInput::node(NodeId(2), 0),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::RenderNode<_, _, _>")),
						..Default::default()
					},
				]
				.into_iter()
				.enumerate()
				.map(|(id, node)| (NodeId(id as u64), node))
				.collect(),
				..Default::default()
			}),
			metadata: DocumentNodeMetadata::position((8, 4)),
			..Default::default()
		};
		network.push_node(node);
		network
	}
}
