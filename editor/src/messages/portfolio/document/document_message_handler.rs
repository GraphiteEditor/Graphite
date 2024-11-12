use super::node_graph::document_node_definitions;
use super::node_graph::utility_types::Transform;
use super::overlays::utility_types::Pivot;
use super::utility_types::clipboards::Clipboard;
use super::utility_types::error::EditorError;
use super::utility_types::misc::{SnappingOptions, SnappingState, GET_SNAP_BOX_FUNCTIONS, GET_SNAP_GEOMETRY_FUNCTIONS};
use super::utility_types::network_interface::{self, NodeNetworkInterface, TransactionStatus};
use super::utility_types::nodes::{CollapsedLayers, SelectedNodes};
use crate::application::{generate_uuid, GRAPHITE_GIT_COMMIT_HASH};
use crate::consts::{ASYMPTOTIC_EFFECT, COLOR_OVERLAY_GRAY, DEFAULT_DOCUMENT_NAME, FILE_SAVE_SUFFIX, SCALE_EFFECT, SCROLLBAR_SPACING, VIEWPORT_ROTATE_SNAP_INTERVAL};
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::NodeGraphHandlerData;
use crate::messages::portfolio::document::overlays::grid_overlays::{grid_overlay, overlay_options};
use crate::messages::portfolio::document::properties_panel::utility_types::PropertiesPanelMessageHandlerData;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, DocumentMode, FlipAxis, PTZ};
use crate::messages::portfolio::document::utility_types::nodes::RawBuffer;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::{self, get_blend_mode, get_opacity};
use crate::messages::tool::tool_messages::select_tool::SelectToolPointerKeys;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::ToolType;
use crate::node_graph_executor::NodeGraphExecutor;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeNetwork, OldNodeNetwork};
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::vector::style::ViewMode;
use graphene_std::renderer::{ClickTarget, Quad};
use graphene_std::vector::path_bool_lib;

use glam::{DAffine2, DVec2, IVec2};

pub struct DocumentMessageData<'a> {
	pub document_id: DocumentId,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub persistent_data: &'a PersistentData,
	pub executor: &'a mut NodeGraphExecutor,
	pub current_tool: &'a ToolType,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct DocumentMessageHandler {
	// ======================
	// Child message handlers
	// ======================
	//
	#[serde(skip)]
	pub navigation_handler: NavigationMessageHandler,
	#[serde(skip)]
	pub node_graph_handler: NodeGraphMessageHandler,
	#[serde(skip)]
	overlays_message_handler: OverlaysMessageHandler,
	#[serde(skip)]
	properties_panel_message_handler: PropertiesPanelMessageHandler,

	// ============================================
	// Fields that are saved in the document format
	// ============================================
	//
	// Contains the NodeNetwork and acts an an interface to manipulate the NodeNetwork with custom setters in order to keep NetworkMetadata in sync
	pub network_interface: NodeNetworkInterface,
	/// List of the [`LayerNodeIdentifier`]s that are currently collapsed by the user in the Layers panel.
	/// Collapsed means that the expansion arrow isn't set to show the children of these layers.
	pub collapsed: CollapsedLayers,
	/// The name of the document, which is displayed in the tab and title bar of the editor.
	pub name: String,
	/// The full Git commit hash of the Graphite repository that was used to build the editor.
	/// We save this to provide a hint about which version of the editor was used to create the document.
	pub commit_hash: String,
	/// The current pan, tilt, and zoom state of the viewport's view of the document canvas.
	pub document_ptz: PTZ,
	/// The current mode that the document is in, which starts out as Design Mode. This choice affects the editing behavior of the tools.
	pub document_mode: DocumentMode,
	/// The current view mode that the user has set for rendering the document within the viewport.
	/// This is usually "Normal" but can be set to "Outline" or "Pixels" to see the canvas differently.
	pub view_mode: ViewMode,
	/// Sets whether or not all the viewport overlays should be drawn on top of the artwork.
	/// This includes tool interaction visualizations (like the transform cage and path anchors/handles), the grid, and more.
	pub overlays_visible: bool,
	/// Sets whether or not the rulers should be drawn along the top and left edges of the viewport area.
	pub rulers_visible: bool,
	/// The current user choices for snapping behavior, including whether snapping is enabled at all.
	pub snapping_state: SnappingState,
	/// Sets whether or not the node graph is drawn (as an overlay) on top of the viewport area, or otherwise if it's hidden.
	pub graph_view_overlay_open: bool,
	/// The current opacity of the faded node graph background that covers up the artwork.
	pub graph_fade_artwork_percentage: f64,

	// =============================================
	// Fields omitted from the saved document format
	// =============================================
	//
	/// Path to network currently viewed in the node graph overlay. This will eventually be stored in each panel, so that multiple panels can refer to different networks
	#[serde(skip)]
	breadcrumb_network_path: Vec<NodeId>,
	/// Path to network that is currently selected. Updated based on the most recently clicked panel.
	#[serde(skip)]
	selection_network_path: Vec<NodeId>,
	/// Stack of document network snapshots for previous history states.
	#[serde(skip)]
	document_undo_history: VecDeque<NodeNetworkInterface>,
	/// Stack of document network snapshots for future history states.
	#[serde(skip)]
	document_redo_history: VecDeque<NodeNetworkInterface>,
	/// Hash of the document snapshot that was most recently saved to disk by the user.
	#[serde(skip)]
	saved_hash: Option<u64>,
	/// Hash of the document snapshot that was most recently auto-saved to the IndexedDB storage that will reopen when the editor is reloaded.
	#[serde(skip)]
	auto_saved_hash: Option<u64>,
	/// The ID of the layer at the start of a range selection in the Layers panel.
	/// If the user clicks or Ctrl-clicks one layer, it becomes the start of the range selection and then Shift-clicking another layer selects all layers between the start and end.
	#[serde(skip)]
	layer_range_selection_reference: Option<LayerNodeIdentifier>,
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
			network_interface: default_document_network_interface(),
			collapsed: CollapsedLayers::default(),
			name: DEFAULT_DOCUMENT_NAME.to_string(),
			commit_hash: GRAPHITE_GIT_COMMIT_HASH.to_string(),
			document_ptz: PTZ::default(),
			document_mode: DocumentMode::DesignMode,
			view_mode: ViewMode::default(),
			overlays_visible: true,
			rulers_visible: true,
			graph_view_overlay_open: false,
			snapping_state: SnappingState::default(),
			graph_fade_artwork_percentage: 80.,
			// =============================================
			// Fields omitted from the saved document format
			// =============================================
			breadcrumb_network_path: Vec::new(),
			selection_network_path: Vec::new(),
			document_undo_history: VecDeque::new(),
			document_redo_history: VecDeque::new(),
			saved_hash: None,
			auto_saved_hash: None,
			layer_range_selection_reference: None,
		}
	}
}

impl MessageHandler<DocumentMessage, DocumentMessageData<'_>> for DocumentMessageHandler {
	fn process_message(&mut self, message: DocumentMessage, responses: &mut VecDeque<Message>, data: DocumentMessageData) {
		let DocumentMessageData {
			document_id,
			ipp,
			persistent_data,
			executor,
			current_tool,
		} = data;

		let selected_nodes_bounding_box_viewport = self.network_interface.selected_nodes_bounding_box_viewport(&self.breadcrumb_network_path);
		let selected_visible_layers_bounding_box_viewport = self.selected_visible_layers_bounding_box_viewport();
		match message {
			// Sub-messages
			DocumentMessage::Navigation(message) => {
				let data = NavigationMessageData {
					network_interface: &mut self.network_interface,
					breadcrumb_network_path: &self.breadcrumb_network_path,
					ipp,
					selection_bounds: if self.graph_view_overlay_open {
						selected_nodes_bounding_box_viewport
					} else {
						selected_visible_layers_bounding_box_viewport
					},
					document_ptz: &mut self.document_ptz,
					graph_view_overlay_open: self.graph_view_overlay_open,
				};

				self.navigation_handler.process_message(message, responses, data);
			}
			DocumentMessage::Overlays(message) => {
				let overlays_visible = self.overlays_visible;
				self.overlays_message_handler.process_message(message, responses, OverlaysMessageData { overlays_visible, ipp });
			}
			DocumentMessage::PropertiesPanel(message) => {
				let properties_panel_message_handler_data = PropertiesPanelMessageHandlerData {
					network_interface: &self.network_interface,
					selection_network_path: &self.selection_network_path,
					document_name: self.name.as_str(),
					executor,
				};
				self.properties_panel_message_handler
					.process_message(message, responses, (persistent_data, properties_panel_message_handler_data));
			}
			DocumentMessage::NodeGraph(message) => {
				self.node_graph_handler.process_message(
					message,
					responses,
					NodeGraphHandlerData {
						network_interface: &mut self.network_interface,
						selection_network_path: &self.selection_network_path,
						breadcrumb_network_path: &self.breadcrumb_network_path,
						document_id,
						collapsed: &mut self.collapsed,
						ipp,
						graph_view_overlay_open: self.graph_view_overlay_open,
						graph_fade_artwork_percentage: self.graph_fade_artwork_percentage,
						navigation_handler: &self.navigation_handler,
					},
				);
			}
			DocumentMessage::GraphOperation(message) => {
				let data = GraphOperationMessageData {
					network_interface: &mut self.network_interface,
					collapsed: &mut self.collapsed,
					node_graph: &mut self.node_graph_handler,
				};
				let mut graph_operation_message_handler = GraphOperationMessageHandler {};
				graph_operation_message_handler.process_message(message, responses, data);
			}
			DocumentMessage::AlignSelectedLayers { axis, aggregate } => {
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

				let mut added_transaction = false;
				for layer in self.network_interface.selected_nodes(&[]).unwrap().selected_unlocked_layers(&self.network_interface) {
					let Some(bbox) = self.metadata().bounding_box_viewport(layer) else {
						continue;
					};
					let center = match aggregate {
						AlignAggregate::Min => bbox[0],
						AlignAggregate::Max => bbox[1],
						_ => (bbox[0] + bbox[1]) / 2.,
					};
					let translation = (aggregated - center) * axis;
					if !added_transaction {
						responses.add(DocumentMessage::AddTransaction);
						added_transaction = true;
					}
					responses.add(GraphOperationMessage::TransformChange {
						layer,
						transform: DAffine2::from_translation(translation),
						transform_in: TransformIn::Viewport,
						skip_rerender: false,
					});
				}
			}
			DocumentMessage::ClearArtboards => {
				responses.add(DocumentMessage::AddTransaction);
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
			DocumentMessage::InsertBooleanOperation { operation } => {
				responses.add(DocumentMessage::AddTransaction);

				let Some(parent) = self.network_interface.deepest_common_ancestor(&[], false) else {
					// Cancel grouping layers across different artboards
					// TODO: Group each set of layers for each artboard separately
					return;
				};
				let insert_index = DocumentMessageHandler::get_calculated_insert_index(self.metadata(), self.network_interface.selected_nodes(&[]).unwrap(), parent);

				let folder_id = NodeId::new();
				let boolean_operation_layer = LayerNodeIdentifier::new_unchecked(folder_id);
				responses.add(GraphOperationMessage::NewBooleanOperationLayer {
					id: folder_id,
					operation,
					parent,
					insert_index,
				});
				responses.add(NodeGraphMessage::SetDisplayNameImpl {
					node_id: folder_id,
					alias: "Boolean Operation".to_string(),
				});
				// Move all shallowest selected layers as children
				responses.add(DocumentMessage::MoveSelectedLayersToGroup { parent: boolean_operation_layer });
			}
			DocumentMessage::CreateEmptyFolder => {
				let id = NodeId::new();

				let parent = self
					.network_interface
					.deepest_common_ancestor(&self.selection_network_path, true)
					.unwrap_or(LayerNodeIdentifier::ROOT_PARENT);

				let insert_index = DocumentMessageHandler::get_calculated_insert_index(self.metadata(), self.network_interface.selected_nodes(&[]).unwrap(), parent);

				responses.add(DocumentMessage::AddTransaction);
				responses.add(GraphOperationMessage::NewCustomLayer {
					id,
					nodes: Vec::new(),
					parent,
					insert_index,
				});
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });
			}
			DocumentMessage::DebugPrintDocument => {
				info!("{:?}", self.network_interface);
			}
			DocumentMessage::DeleteNode { node_id } => {
				responses.add(DocumentMessage::StartTransaction);

				responses.add(NodeGraphMessage::DeleteNodes {
					node_ids: vec![node_id],
					delete_children: true,
				});
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SelectedNodesUpdated);
				responses.add(NodeGraphMessage::SendGraph);
				responses.add(DocumentMessage::EndTransaction);
			}
			DocumentMessage::DeleteSelectedLayers => {
				responses.add(NodeGraphMessage::DeleteSelectedNodes { delete_children: true });
			}
			DocumentMessage::DeselectAllLayers => {
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				self.layer_range_selection_reference = None;
			}
			DocumentMessage::DocumentHistoryBackward => self.undo_with_history(ipp, responses),
			DocumentMessage::DocumentHistoryForward => self.redo_with_history(ipp, responses),
			DocumentMessage::DocumentStructureChanged => {
				self.update_layers_panel_options_bar_widgets(responses);

				self.network_interface.load_structure();
				let data_buffer: RawBuffer = self.serialize_root();
				responses.add(FrontendMessage::UpdateDocumentLayerStructure { data_buffer });
			}
			DocumentMessage::DrawArtboardOverlays(overlay_context) => {
				for layer in self.metadata().all_layers() {
					if !self.network_interface.is_artboard(&layer.to_node(), &[]) {
						continue;
					}
					let Some(bounds) = self.metadata().bounding_box_document(layer) else { continue };

					let name = self.network_interface.frontend_display_name(&layer.to_node(), &[]);

					let transform = self.metadata().document_to_viewport
						* DAffine2::from_translation(bounds[0].min(bounds[1]))
						* DAffine2::from_scale(DVec2::splat(self.document_ptz.zoom().recip()))
						* DAffine2::from_translation(-DVec2::Y * 4.);

					overlay_context.text(&name, COLOR_OVERLAY_GRAY, None, transform, 0., [Pivot::Start, Pivot::End]);
				}
			}
			DocumentMessage::DuplicateSelectedLayers => {
				let parent = self.new_layer_parent(false);
				let calculated_insert_index =
					DocumentMessageHandler::get_calculated_insert_index(self.network_interface.document_metadata(), self.network_interface.selected_nodes(&[]).unwrap(), parent);

				responses.add(DocumentMessage::AddTransaction);
				responses.add(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
				responses.add(PortfolioMessage::PasteIntoFolder {
					clipboard: Clipboard::Internal,
					parent,
					insert_index: calculated_insert_index,
				});
			}
			DocumentMessage::EnterNestedNetwork { node_id } => {
				self.breadcrumb_network_path.push(node_id);
				self.selection_network_path.clone_from(&self.breadcrumb_network_path);
				responses.add(NodeGraphMessage::SendGraph);
				responses.add(DocumentMessage::ZoomCanvasToFitAll);
			}
			DocumentMessage::Escape => {
				if self.node_graph_handler.drag_start.is_some() {
					responses.add(DocumentMessage::AbortTransaction);
					self.node_graph_handler.drag_start = None;
				} else if self
					.node_graph_handler
					.context_menu
					.as_ref()
					.is_some_and(|context_menu| matches!(context_menu.context_menu_data, super::node_graph::utility_types::ContextMenuData::CreateNode))
				{
					// Close the context menu
					self.node_graph_handler.context_menu = None;
					responses.add(FrontendMessage::UpdateContextMenuInformation { context_menu_information: None });
					self.node_graph_handler.wire_in_progress_from_connector = None;
					self.node_graph_handler.wire_in_progress_to_connector = None;
					responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: None });
				} else {
					responses.add(DocumentMessage::GraphViewOverlay { open: false });
				}
			}
			DocumentMessage::ExitNestedNetwork { steps_back } => {
				for _ in 0..steps_back {
					self.breadcrumb_network_path.pop();
					self.selection_network_path.clone_from(&self.breadcrumb_network_path);
				}
				responses.add(DocumentMessage::PTZUpdate);
				responses.add(NodeGraphMessage::SetGridAlignedEdges);
				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::FlipSelectedLayers { flip_axis } => {
				let scale = match flip_axis {
					FlipAxis::X => DVec2::new(-1., 1.),
					FlipAxis::Y => DVec2::new(1., -1.),
				};
				if let Some([min, max]) = self.selected_visible_and_unlock_layers_bounding_box_viewport() {
					let center = (max + min) / 2.;
					let bbox_trans = DAffine2::from_translation(-center);
					let mut added_transaction = false;
					for layer in self.network_interface.selected_nodes(&[]).unwrap().selected_unlocked_layers(&self.network_interface) {
						if !added_transaction {
							responses.add(DocumentMessage::AddTransaction);
							added_transaction = true;
						}
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

				responses.add(FrontendMessage::UpdateGraphViewOverlay { open });
				responses.add(FrontendMessage::UpdateGraphFadeArtwork {
					percentage: self.graph_fade_artwork_percentage,
				});

				// Update the tilt menu bar buttons to be disabled when the graph is open
				responses.add(MenuBarMessage::SendLayout);

				responses.add(DocumentMessage::RenderRulers);
				responses.add(DocumentMessage::RenderScrollbars);
				if open {
					responses.add(ToolMessage::DeactivateTools);
					responses.add(OverlaysMessage::Draw); // Clear the overlays
					responses.add(NavigationMessage::CanvasTiltSet { angle_radians: 0. });
					responses.add(NodeGraphMessage::SetGridAlignedEdges);
					responses.add(NodeGraphMessage::UpdateGraphBarRight);
					responses.add(NodeGraphMessage::SendGraph);
				} else {
					responses.add(ToolMessage::ActivateTool { tool_type: *current_tool });
				}
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
			DocumentMessage::GridVisibility(enabled) => {
				self.snapping_state.grid_snapping = enabled;
				responses.add(OverlaysMessage::Draw);
			}
			DocumentMessage::GroupSelectedLayers => {
				responses.add(DocumentMessage::AddTransaction);

				let Some(parent) = self.network_interface.deepest_common_ancestor(&self.selection_network_path, false) else {
					// Cancel grouping layers across different artboards
					// TODO: Group each set of layers for each artboard separately
					return;
				};
				let insert_index = DocumentMessageHandler::get_calculated_insert_index(self.metadata(), self.network_interface.selected_nodes(&[]).unwrap(), parent);

				let node_id = NodeId::new();
				let new_group_node = document_node_definitions::resolve_document_node_type("Merge")
					.expect("Failed to create merge node")
					.default_node_template();
				responses.add(NodeGraphMessage::InsertNode {
					node_id,
					node_template: new_group_node,
				});
				let new_group_folder = LayerNodeIdentifier::new_unchecked(node_id);
				// Move the new folder to the correct position
				responses.add(NodeGraphMessage::MoveLayerToStack {
					layer: new_group_folder,
					parent,
					insert_index,
				});

				responses.add(DocumentMessage::MoveSelectedLayersToGroup { parent: new_group_folder });
			}
			DocumentMessage::ImaginateGenerate { imaginate_node } => {
				let random_value = generate_uuid();
				responses.add(NodeGraphMessage::SetInputValue {
					node_id: *imaginate_node.last().unwrap(),
					// Needs to match the index of the seed parameter in `pub const IMAGINATE_NODE: DocumentNodeDefinition` in `document_node_type.rs`
					input_index: 17,
					value: graph_craft::document::value::TaggedValue::U64(random_value),
				});

				responses.add(PortfolioMessage::SubmitGraphRender { document_id, ignore_hash: false });
			}
			DocumentMessage::ImaginateRandom { imaginate_node, then_generate } => {
				// Generate a random seed. We only want values between -2^53 and 2^53, because integer values
				// outside of this range can get rounded in f64
				let random_bits = generate_uuid();
				let random_value = ((random_bits >> 11) as f64).copysign(f64::from_bits(random_bits & (1 << 63)));

				responses.add(DocumentMessage::AddTransaction);
				// Set a random seed input
				responses.add(NodeGraphMessage::SetInputValue {
					node_id: *imaginate_node.last().unwrap(),
					// Needs to match the index of the seed parameter in `pub const IMAGINATE_NODE: DocumentNodeDefinition` in `document_node_type.rs`
					input_index: 3,
					value: graph_craft::document::value::TaggedValue::F64(random_value),
				});

				// Generate the image
				if then_generate {
					responses.add(DocumentMessage::ImaginateGenerate { imaginate_node });
				}
			}
			DocumentMessage::MoveSelectedLayersTo { parent, insert_index } => {
				if !self.selection_network_path.is_empty() {
					log::error!("Moving selected layers is only supported for the Document Network");
					return;
				}

				// Disallow trying to insert into self.
				if self
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_layers(self.metadata())
					.any(|layer| parent.ancestors(self.metadata()).any(|ancestor| ancestor == layer))
				{
					return;
				}
				// Artboards can only have `ROOT_PARENT` as the parent.
				let any_artboards = self
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_layers(self.metadata())
					.any(|layer| self.network_interface.is_artboard(&layer.to_node(), &self.selection_network_path));
				if any_artboards && parent != LayerNodeIdentifier::ROOT_PARENT {
					return;
				}

				// Non-artboards cannot be put at the top level if artboards also exist there
				let selected_any_non_artboards = self
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_layers(self.metadata())
					.any(|layer| !self.network_interface.is_artboard(&layer.to_node(), &self.selection_network_path));

				let top_level_artboards = LayerNodeIdentifier::ROOT_PARENT
					.children(self.metadata())
					.any(|layer| self.network_interface.is_artboard(&layer.to_node(), &self.selection_network_path));

				if selected_any_non_artboards && parent == LayerNodeIdentifier::ROOT_PARENT && top_level_artboards {
					return;
				}

				let layers_to_move = self.network_interface.shallowest_unique_layers_sorted(&self.selection_network_path);
				// Offset the index for layers to move that are below another layer to move. For example when moving 1 and 2 between 3 and 4, 2 should be inserted at the same index as 1 since 1 is moved first.
				let layers_to_move_with_insert_offset = layers_to_move
					.iter()
					.map(|layer| {
						if layer.parent(self.metadata()) != Some(parent) {
							(*layer, 0)
						} else {
							let upstream_selected_siblings = layer
								.downstream_siblings(self.network_interface.document_metadata())
								.filter(|sibling| {
									sibling != layer
										&& layers_to_move.iter().any(|layer| {
											layer == sibling
												&& layer
													.parent(self.metadata())
													.is_some_and(|parent| parent.children(self.metadata()).position(|child| child == *layer) < Some(insert_index))
										})
								})
								.count();
							(*layer, upstream_selected_siblings)
						}
					})
					.collect::<Vec<_>>();

				responses.add(DocumentMessage::AddTransaction);
				for (layer_index, (layer_to_move, insert_offset)) in layers_to_move_with_insert_offset.into_iter().enumerate() {
					let calculated_insert_index = insert_index + layer_index - insert_offset;
					responses.add(NodeGraphMessage::MoveLayerToStack {
						layer: layer_to_move,
						parent,
						insert_index: calculated_insert_index,
					});
				}

				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::MoveSelectedLayersToGroup { parent } => {
				// Group all shallowest unique selected layers in order
				let all_layers_to_group_sorted = self.network_interface.shallowest_unique_layers_sorted(&self.selection_network_path);

				for layer_to_group in all_layers_to_group_sorted.into_iter().rev() {
					responses.add(NodeGraphMessage::MoveLayerToStack {
						layer: layer_to_group,
						parent,
						insert_index: 0,
					});
				}

				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![parent.to_node()] });
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(DocumentMessage::DocumentStructureChanged);
				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::NudgeSelectedLayers {
				delta_x,
				delta_y,
				resize,
				resize_opposite_corner,
			} => {
				responses.add(DocumentMessage::AddTransaction);

				let resize = ipp.keyboard.key(resize);
				let resize_opposite_corner = ipp.keyboard.key(resize_opposite_corner);

				let can_move = |layer| {
					self.network_interface
						.selected_nodes(&[])
						.is_some_and(|selected| selected.layer_visible(layer, &self.network_interface) && !selected.layer_locked(layer, &self.network_interface))
				};

				// Nudge translation without resizing
				if !resize {
					let transform = DAffine2::from_translation(DVec2::from_angle(-self.document_ptz.tilt()).rotate(DVec2::new(delta_x, delta_y)));

					for layer in self.network_interface.shallowest_unique_layers(&[]).filter(|layer| can_move(*layer)) {
						responses.add(GraphOperationMessage::TransformChange {
							layer,
							transform,
							transform_in: TransformIn::Local,
							skip_rerender: false,
						});
					}

					return;
				}

				let selected_bounding_box = self.network_interface.selected_bounds_document_space(false, &[]);
				let Some([existing_top_left, existing_bottom_right]) = selected_bounding_box else { return };

				// Swap and negate coordinates as needed to match the resize direction that's closest to the current tilt angle
				let tilt = (self.document_ptz.tilt() + std::f64::consts::TAU) % std::f64::consts::TAU;
				let (delta_x, delta_y, opposite_x, opposite_y) = match ((tilt + std::f64::consts::FRAC_PI_4) / std::f64::consts::FRAC_PI_2).floor() as i32 % 4 {
					0 => (delta_x, delta_y, false, false),
					1 => (delta_y, -delta_x, false, true),
					2 => (-delta_x, -delta_y, true, true),
					3 => (-delta_y, delta_x, true, false),
					_ => unreachable!(),
				};

				let size = existing_bottom_right - existing_top_left;
				let enlargement = DVec2::new(
					if resize_opposite_corner != opposite_x { -delta_x } else { delta_x },
					if resize_opposite_corner != opposite_y { -delta_y } else { delta_y },
				);
				let enlargement_factor = (enlargement + size) / size;

				let position = DVec2::new(
					existing_top_left.x + if resize_opposite_corner != opposite_x { delta_x } else { 0. },
					existing_top_left.y + if resize_opposite_corner != opposite_y { delta_y } else { 0. },
				);
				let mut pivot = (existing_top_left * enlargement_factor - position) / (enlargement_factor - DVec2::ONE);
				if !pivot.x.is_finite() {
					pivot.x = 0.;
				}
				if !pivot.y.is_finite() {
					pivot.y = 0.;
				}
				let scale = DAffine2::from_scale(enlargement_factor);
				let pivot = DAffine2::from_translation(pivot);
				let transformation = pivot * scale * pivot.inverse();
				let document_to_viewport = self.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.center(), &self.document_ptz);

				for layer in self.network_interface.shallowest_unique_layers(&[]).filter(|layer| can_move(*layer)) {
					let to = document_to_viewport.inverse() * self.metadata().downstream_transform_to_viewport(layer);
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
			DocumentMessage::PasteImage {
				name,
				image,
				mouse,
				parent_and_insert_index,
			} => {
				// All the image's pixels have been converted to 0..=1, linear, and premultiplied by `Color::from_rgba8_srgb`

				let image_size = DVec2::new(image.width as f64, image.height as f64);

				// Align the layer with the mouse or center of viewport
				let viewport_location = mouse.map_or(ipp.viewport_bounds.center() + ipp.viewport_bounds.top_left, |pos| pos.into());

				let document_to_viewport = self.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.center(), &self.document_ptz);
				let center_in_viewport = DAffine2::from_translation(document_to_viewport.inverse().transform_point2(viewport_location - ipp.viewport_bounds.top_left));
				let center_in_viewport_layerspace = center_in_viewport;

				// Make layer the size of the image
				let fit_image_size = DAffine2::from_scale_angle_translation(image_size, 0., image_size / -2.);

				let transform = center_in_viewport_layerspace * fit_image_size;

				let layer_node_id = NodeId::new();
				let layer_id = LayerNodeIdentifier::new_unchecked(layer_node_id);

				responses.add(DocumentMessage::AddTransaction);

				let image_frame = ImageFrame { image, ..Default::default() };
				let layer = graph_modification_utils::new_image_layer(image_frame, layer_node_id, self.new_layer_parent(true), responses);

				if let Some(name) = name {
					responses.add(NodeGraphMessage::SetDisplayName {
						node_id: layer.to_node(),
						alias: name,
					});
				}
				if let Some((parent, insert_index)) = parent_and_insert_index {
					responses.add(NodeGraphMessage::MoveLayerToStack {
						layer: layer_id,
						parent,
						insert_index,
					});
				}

				// `layer` cannot be `ROOT_PARENT` since it is the newly created layer
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
			DocumentMessage::PasteSvg {
				name,
				svg,
				mouse,
				parent_and_insert_index,
			} => {
				let document_to_viewport = self.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.center(), &self.document_ptz);
				let viewport_location = mouse.map_or(ipp.viewport_bounds.center() + ipp.viewport_bounds.top_left, |pos| pos.into());
				let center_in_viewport = DAffine2::from_translation(document_to_viewport.inverse().transform_point2(viewport_location - ipp.viewport_bounds.top_left));

				let layer_node_id = NodeId::new();
				let layer_id = LayerNodeIdentifier::new_unchecked(layer_node_id);

				responses.add(DocumentMessage::AddTransaction);

				let layer = graph_modification_utils::new_svg_layer(svg, center_in_viewport, layer_node_id, self.new_layer_parent(true), responses);

				if let Some(name) = name {
					responses.add(NodeGraphMessage::SetDisplayName {
						node_id: layer.to_node(),
						alias: name,
					});
				}
				if let Some((parent, insert_index)) = parent_and_insert_index {
					responses.add(NodeGraphMessage::MoveLayerToStack {
						layer: layer_id,
						parent,
						insert_index,
					});
				}

				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
				responses.add(ToolMessage::ActivateTool { tool_type: ToolType::Select });
			}
			DocumentMessage::Redo => {
				if self.network_interface.transaction_status() != TransactionStatus::Finished {
					return;
				}
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
				let current_ptz = if self.graph_view_overlay_open {
					let Some(network_metadata) = self.network_interface.network_metadata(&self.breadcrumb_network_path) else {
						return;
					};
					&network_metadata.persistent_metadata.navigation_metadata.node_graph_ptz
				} else {
					&self.document_ptz
				};
				let document_to_viewport = self.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.center(), current_ptz);

				let ruler_scale = if !self.graph_view_overlay_open {
					self.navigation_handler.snapped_zoom(current_ptz.zoom())
				} else {
					self.navigation_handler.snapped_zoom(current_ptz.zoom() * (crate::consts::GRID_SIZE as f64))
				};

				let ruler_origin = document_to_viewport.transform_point2(DVec2::ZERO);
				let log = ruler_scale.log2();
				let mut ruler_interval: f64 = if log < 0. { 100. * 2_f64.powf(-log.ceil()) } else { 100. / 2_f64.powf(log.ceil()) };

				// When the interval becomes too small, force it to be a whole number, then to powers of 10.
				// The progression of intervals is:
				// ..., 100, 50, 25, 12.5, 6 (6.25), 4 (3.125), 2 (1.5625), 1, 0.1, 0.01, ...
				if ruler_interval < 1. {
					ruler_interval = 10_f64.powf(ruler_interval.log10().ceil());
				} else if ruler_interval < 12.5 {
					// Round to nearest even number
					ruler_interval = 2. * (ruler_interval / 2.).round();
				}

				if self.graph_view_overlay_open {
					ruler_interval = ruler_interval.max(1.);
				}

				let ruler_spacing = ruler_interval * ruler_scale;

				responses.add(FrontendMessage::UpdateDocumentRulers {
					origin: ruler_origin.into(),
					spacing: ruler_spacing,
					interval: ruler_interval,
					visible: self.rulers_visible,
				});
			}
			DocumentMessage::RenderScrollbars => {
				let document_transform_scale = self.navigation_handler.snapped_zoom(self.document_ptz.zoom());

				let scale = 0.5 + ASYMPTOTIC_EFFECT + document_transform_scale * SCALE_EFFECT;

				let viewport_size = ipp.viewport_bounds.size();
				let viewport_mid = ipp.viewport_bounds.center();
				let [bounds1, bounds2] = if !self.graph_view_overlay_open {
					self.metadata().document_bounds_viewport_space().unwrap_or([viewport_mid; 2])
				} else {
					self.network_interface.graph_bounds_viewport_space(&self.breadcrumb_network_path).unwrap_or([viewport_mid; 2])
				};
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
				let all_layers_except_artboards_invisible_and_locked = metadata.all_layers().filter(|&layer| !self.network_interface.is_artboard(&layer.to_node(), &[])).filter(|&layer| {
					self.network_interface.selected_nodes(&[]).unwrap().layer_visible(layer, &self.network_interface)
						&& !self.network_interface.selected_nodes(&[]).unwrap().layer_locked(layer, &self.network_interface)
				});
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
				let layer = LayerNodeIdentifier::new(id, &self.network_interface, &[]);

				let mut nodes = vec![];

				// If we have shift pressed and a layer already selected then fill the range
				if let Some(last_selected) = self.layer_range_selection_reference.filter(|_| shift) {
					if last_selected == LayerNodeIdentifier::ROOT_PARENT {
						log::error!("ROOT_PARENT cannot be selected in SelectLayer");
						return;
					}

					nodes.push(last_selected.to_node());
					nodes.push(id);

					// Fill the selection range
					self.metadata()
						.all_layers()
						.skip_while(|&node| node != layer && node != last_selected)
						.skip(1)
						.take_while(|&node| node != layer && node != last_selected)
						.for_each(|node| {
							if node == LayerNodeIdentifier::ROOT_PARENT {
								log::error!("ROOT_PARENT should not exist in all_layers")
							} else {
								nodes.push(node.to_node())
							}
						});
				} else {
					if ctrl {
						// Toggle selection when holding ctrl
						if self.network_interface.selected_nodes(&[]).unwrap().selected_layers_contains(layer, self.metadata()) {
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
			DocumentMessage::SetActivePanel { active_panel: panel } => {
				use crate::messages::portfolio::utility_types::PanelType;
				match panel {
					PanelType::Document => {
						if self.graph_view_overlay_open {
							self.selection_network_path.clone_from(&self.breadcrumb_network_path);
						} else {
							self.selection_network_path = vec![]
						}
					}
					PanelType::Layers => self.selection_network_path = vec![],
					_ => {}
				}
				responses.add(PropertiesPanelMessage::Refresh);
				responses.add(NodeGraphMessage::UpdateLayerPanel);
				responses.add(NodeGraphMessage::UpdateInSelectedNetwork)
			}
			DocumentMessage::SetBlendModeForSelectedLayers { blend_mode } => {
				for layer in self.network_interface.selected_nodes(&[]).unwrap().selected_layers_except_artboards(&self.network_interface) {
					responses.add(GraphOperationMessage::BlendModeSet { layer, blend_mode });
				}
			}
			DocumentMessage::SetGraphFadeArtwork { percentage } => {
				self.graph_fade_artwork_percentage = percentage;
				responses.add(FrontendMessage::UpdateGraphFadeArtwork { percentage });
			}
			DocumentMessage::SetNodePinned { node_id, pinned } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetPinned { node_id, pinned });
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SelectedNodesUpdated);
				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::SetOpacityForSelectedLayers { opacity } => {
				let opacity = opacity.clamp(0., 1.);
				for layer in self.network_interface.selected_nodes(&[]).unwrap().selected_layers_except_artboards(&self.network_interface) {
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
			DocumentMessage::SetSnapping { closure, snapping_state } => {
				if let Some(closure) = closure {
					*closure(&mut self.snapping_state) = snapping_state;
				}
			}
			DocumentMessage::SetToNodeOrLayer { node_id, is_layer } => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(NodeGraphMessage::SetToNodeOrLayer { node_id, is_layer });
			}
			DocumentMessage::SetViewMode { view_mode } => {
				self.view_mode = view_mode;
				responses.add_front(NodeGraphMessage::RunDocumentGraph);
			}
			// Note: A transaction should never be started in a scope that mutates the network interface, since it will only be run after that scope ends.
			DocumentMessage::StartTransaction => {
				self.network_interface.start_transaction();
				let network_interface_clone = self.network_interface.clone();
				self.document_undo_history.push_back(network_interface_clone);
				if self.document_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
					self.document_undo_history.pop_front();
				}
				// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			// Commits the transaction if the network was mutated since the transaction started, otherwise it aborts the transaction
			DocumentMessage::EndTransaction => match self.network_interface.transaction_status() {
				TransactionStatus::Started => {
					responses.add_front(DocumentMessage::AbortTransaction);
				}
				TransactionStatus::Modified => {
					responses.add_front(DocumentMessage::CommitTransaction);
				}
				TransactionStatus::Finished => {}
			},
			DocumentMessage::CommitTransaction => {
				if self.network_interface.transaction_status() == TransactionStatus::Finished {
					return;
				}
				self.network_interface.finish_transaction();
				self.document_redo_history.clear();
			}
			DocumentMessage::AbortTransaction => {
				if self.network_interface.transaction_status() == TransactionStatus::Finished {
					return;
				}

				self.undo(ipp, responses);
				self.network_interface.finish_transaction();
				responses.add(OverlaysMessage::Draw);
			}
			DocumentMessage::AddTransaction => {
				// Reverse order since they are added to the front
				responses.add_front(DocumentMessage::CommitTransaction);
				responses.add_front(DocumentMessage::StartTransaction);
			}
			DocumentMessage::ToggleLayerExpansion { id } => {
				let layer = LayerNodeIdentifier::new(id, &self.network_interface, &[]);
				if self.collapsed.0.contains(&layer) {
					self.collapsed.0.retain(|&collapsed_layer| collapsed_layer != layer);
				} else {
					self.collapsed.0.push(layer);
				}
				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::ToggleSelectedLocked => responses.add(NodeGraphMessage::ToggleSelectedLocked),
			DocumentMessage::ToggleSelectedVisibility => {
				responses.add(NodeGraphMessage::ToggleSelectedVisibility);
			}
			DocumentMessage::ToggleGridVisibility => {
				self.snapping_state.grid_snapping = !self.snapping_state.grid_snapping;
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			DocumentMessage::ToggleOverlaysVisibility => {
				self.overlays_visible = !self.overlays_visible;
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			DocumentMessage::ToggleSnapping => {
				self.snapping_state.snapping_enabled = !self.snapping_state.snapping_enabled;
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			DocumentMessage::UpdateUpstreamTransforms { upstream_transforms } => {
				self.network_interface.update_transforms(upstream_transforms);
			}
			DocumentMessage::UpdateClickTargets { click_targets } => {
				// TODO: Allow non layer nodes to have click targets
				let layer_click_targets = click_targets
					.into_iter()
					.filter(|(node_id, _)|
						// Ensure that the layer is in the document network to prevent logging an error
						self.network_interface.network(&[]).unwrap().nodes.contains_key(node_id))
					.filter_map(|(node_id, click_targets)| {
						self.network_interface.is_layer(&node_id, &[]).then(|| {
							let layer = LayerNodeIdentifier::new(node_id, &self.network_interface, &[]);
							(layer, click_targets)
						})
					})
					.collect();
				self.network_interface.update_click_targets(layer_click_targets);
			}
			DocumentMessage::UpdateClipTargets { clip_targets } => {
				self.network_interface.update_clip_targets(clip_targets);
			}
			DocumentMessage::UpdateVectorModify { vector_modify } => {
				self.network_interface.update_vector_modify(vector_modify);
			}
			DocumentMessage::Undo => {
				if self.network_interface.transaction_status() != TransactionStatus::Finished {
					return;
				}
				responses.add(ToolMessage::PreUndo);
				responses.add(DocumentMessage::DocumentHistoryBackward);
				responses.add(OverlaysMessage::Draw);
				responses.add(ToolMessage::Undo);
			}
			DocumentMessage::UngroupSelectedLayers => {
				if !self.selection_network_path.is_empty() {
					log::error!("Ungrouping selected layers is only supported for the Document Network");
					return;
				}
				responses.add(DocumentMessage::AddTransaction);

				let folder_paths = self.network_interface.folders_sorted_by_most_nested(&self.selection_network_path);
				for folder in folder_paths {
					if folder == LayerNodeIdentifier::ROOT_PARENT {
						log::error!("ROOT_PARENT cannot be selected when ungrouping selected layers");
						continue;
					}

					// Cannot ungroup artboard
					if self.network_interface.is_artboard(&folder.to_node(), &self.selection_network_path) {
						return;
					}

					responses.add(DocumentMessage::UngroupLayer { layer: folder });
				}

				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(DocumentMessage::DocumentStructureChanged);
				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::UngroupLayer { layer } => {
				let parent = layer.parent(self.metadata()).expect("Ungrouped folder must have a parent");
				let folder_index = parent.children(self.metadata()).position(|child| child == layer).unwrap_or(0);

				// Move all children of the folder above the folder in reverse order since each children is moved above the previous one
				for child in layer.children(self.metadata()).collect::<Vec<_>>().into_iter().rev() {
					responses.add(NodeGraphMessage::MoveLayerToStack {
						layer: child,
						parent,
						insert_index: folder_index,
					});
				}

				// Delete empty group folder
				responses.add(NodeGraphMessage::DeleteNodes {
					node_ids: vec![layer.to_node()],
					delete_children: true,
				});
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SelectedNodesUpdated);
				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::PTZUpdate => {
				if !self.graph_view_overlay_open {
					let transform = self.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.center(), &self.document_ptz);
					self.network_interface.set_document_to_viewport_transform(transform);
					// Ensure selection box is kept in sync with the pointer when the PTZ changes
					responses.add(SelectToolMessage::PointerMove(SelectToolPointerKeys {
						axis_align: Key::Shift,
						snap_angle: Key::Control,
						center: Key::Alt,
						duplicate: Key::Alt,
					}));
					responses.add(NodeGraphMessage::RunDocumentGraph);
				} else {
					let Some(network_metadata) = self.network_interface.network_metadata(&self.breadcrumb_network_path) else {
						return;
					};

					let transform = self
						.navigation_handler
						.calculate_offset_transform(ipp.viewport_bounds.center(), &network_metadata.persistent_metadata.navigation_metadata.node_graph_ptz);
					self.network_interface.set_transform(transform, &self.breadcrumb_network_path);
					let imports = self.network_interface.frontend_imports(&self.breadcrumb_network_path).unwrap_or_default();
					let exports = self.network_interface.frontend_exports(&self.breadcrumb_network_path).unwrap_or_default();
					let add_import = self.network_interface.frontend_import_modify(&self.breadcrumb_network_path);
					let add_export = self.network_interface.frontend_export_modify(&self.breadcrumb_network_path);

					responses.add(DocumentMessage::RenderRulers);
					responses.add(DocumentMessage::RenderScrollbars);
					responses.add(NodeGraphMessage::UpdateEdges);
					responses.add(NodeGraphMessage::UpdateBoxSelection);
					responses.add(FrontendMessage::UpdateImportsExports {
						imports,
						exports,
						add_import,
						add_export,
					});
					responses.add(FrontendMessage::UpdateNodeGraphTransform {
						transform: Transform {
							scale: transform.matrix2.x_axis.x,
							x: transform.translation.x,
							y: transform.translation.y,
						},
					})
				}
			}
			DocumentMessage::SelectionStepBack => {
				self.network_interface.selection_step_back(&self.selection_network_path);
				responses.add(BroadcastEvent::SelectionChanged);
			}
			DocumentMessage::SelectionStepForward => {
				self.network_interface.selection_step_forward(&self.selection_network_path);
				responses.add(BroadcastEvent::SelectionChanged);
			}
			DocumentMessage::WrapContentInArtboard { place_artboard_at_origin } => {
				// Get bounding box of all layers
				let bounds = self.network_interface.document_bounds_document_space(false);
				let Some(bounds) = bounds else { return };
				let bounds_rounded_dimensions = (bounds[1] - bounds[0]).round();

				// Create an artboard and set its dimensions to the bounding box size and location
				let node_id = NodeId::new();
				let node_layer_id = LayerNodeIdentifier::new_unchecked(node_id);
				let new_artboard_node = document_node_definitions::resolve_document_node_type("Artboard")
					.expect("Failed to create artboard node")
					.default_node_template();
				responses.add(NodeGraphMessage::InsertNode {
					node_id,
					node_template: new_artboard_node,
				});
				responses.add(NodeGraphMessage::ShiftNodePosition { node_id, x: 15, y: -3 });
				responses.add(GraphOperationMessage::ResizeArtboard {
					layer: LayerNodeIdentifier::new_unchecked(node_id),
					location: if place_artboard_at_origin { IVec2::ZERO } else { bounds[0].round().as_ivec2() },
					dimensions: bounds_rounded_dimensions.as_ivec2(),
				});

				// Connect the current output data to the artboard's input data, and the artboard's output to the document output
				responses.add(NodeGraphMessage::InsertNodeBetween {
					node_id,
					input_connector: network_interface::InputConnector::Export(0),
					insert_node_input_index: 1,
				});

				// Shift the content by half its width and height so it gets centered in the artboard
				responses.add(GraphOperationMessage::TransformChange {
					layer: node_layer_id,
					transform: DAffine2::from_translation(bounds_rounded_dimensions / 2.),
					transform_in: TransformIn::Local,
					skip_rerender: false,
				});
			}
			DocumentMessage::ZoomCanvasTo100Percent => {
				responses.add_front(NavigationMessage::CanvasZoomSet { zoom_factor: 1. });
			}
			DocumentMessage::ZoomCanvasTo200Percent => {
				responses.add_front(NavigationMessage::CanvasZoomSet { zoom_factor: 2. });
			}
			DocumentMessage::ZoomCanvasToFitAll => {
				let bounds = if self.graph_view_overlay_open {
					self.network_interface.all_nodes_bounding_box(&self.breadcrumb_network_path).cloned()
				} else {
					self.network_interface.document_bounds_document_space(true)
				};
				if let Some(bounds) = bounds {
					responses.add(NavigationMessage::CanvasTiltSet { angle_radians: 0. });
					responses.add(NavigationMessage::FitViewportToBounds { bounds, prevent_zoom_past_100: true });
				} else {
					warn!("Cannot zoom due to no bounds")
				}
			}
			DocumentMessage::Noop => (),
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(DocumentMessageDiscriminant;
			CreateEmptyFolder,
			DebugPrintDocument,
			DeselectAllLayers,
			GraphViewOverlayToggle,
			Noop,
			Redo,
			SaveDocument,
			SelectAllLayers,
			SetSnapping,
			ToggleGridVisibility,
			ToggleOverlaysVisibility,
			ToggleSnapping,
			Undo,
			SelectionStepForward,
			SelectionStepBack,
			ZoomCanvasTo100Percent,
			ZoomCanvasTo200Percent,
			ZoomCanvasToFitAll,
		);

		// Additional actions if there are any selected layers
		if self.network_interface.selected_nodes(&[]).unwrap().selected_layers(self.metadata()).next().is_some() {
			let mut select = actions!(DocumentMessageDiscriminant;
				DeleteSelectedLayers,
				DuplicateSelectedLayers,
				GroupSelectedLayers,
				SelectedLayersLower,
				SelectedLayersLowerToBack,
				SelectedLayersRaise,
				SelectedLayersRaiseToFront,
				UngroupSelectedLayers,
				ToggleSelectedLocked
			);
			if !self.graph_view_overlay_open {
				select.extend(actions!(DocumentMessageDiscriminant;
					NudgeSelectedLayers,
					ToggleSelectedVisibility,
				));
			}
			common.extend(select);
		}

		// Additional actions if the node graph is open
		if self.graph_view_overlay_open {
			common.extend(actions!(DocumentMessageDiscriminant;
				Escape
			));
			common.extend(self.node_graph_handler.actions_additional_if_node_graph_is_open());
		}
		// More additional actions
		common.extend(self.navigation_handler.actions());
		common.extend(self.node_graph_handler.actions());
		common
	}
}

impl DocumentMessageHandler {
	/// Runs an intersection test with all layers and a viewport space quad
	pub fn intersect_quad<'a>(&'a self, viewport_quad: graphene_core::renderer::Quad, ipp: &InputPreprocessorMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
		let document_to_viewport = self.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.center(), &self.document_ptz);
		let document_quad = document_to_viewport.inverse() * viewport_quad;

		ClickXRayIter::new(&self.network_interface, XRayTarget::Quad(document_quad))
	}

	/// Runs an intersection test with all layers and a viewport space quad; ignoring artboards
	pub fn intersect_quad_no_artboards<'a>(&'a self, viewport_quad: graphene_core::renderer::Quad, ipp: &InputPreprocessorMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
		self.intersect_quad(viewport_quad, ipp).filter(|layer| !self.network_interface.is_artboard(&layer.to_node(), &[]))
	}

	/// Find all of the layers that were clicked on from a viewport space location
	pub fn click_xray(&self, ipp: &InputPreprocessorMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		let document_to_viewport = self.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.center(), &self.document_ptz);
		let point = document_to_viewport.inverse().transform_point2(ipp.mouse.position);
		ClickXRayIter::new(&self.network_interface, XRayTarget::Point(point))
	}

	/// Find the deepest layer given in the sorted array (by returning the one which is not a folder from the list of layers under the click location).
	pub fn find_deepest(&self, node_list: &[LayerNodeIdentifier]) -> Option<LayerNodeIdentifier> {
		node_list
			.iter()
			.find(|&&layer| {
				if layer != LayerNodeIdentifier::ROOT_PARENT {
					!layer.has_children(self.network_interface.document_metadata())
				} else {
					log::error!("ROOT_PARENT should not exist in find_deepest");
					false
				}
			})
			.copied()
	}

	/// Find layers under the location in viewport space that was clicked, listed by their depth in the layer tree hierarchy.
	pub fn click_list<'a>(&'a self, ipp: &InputPreprocessorMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
		self.click_xray(ipp)
			.filter(move |&layer| !self.network_interface.is_artboard(&layer.to_node(), &[]))
			.skip_while(|&layer| layer == LayerNodeIdentifier::ROOT_PARENT)
			.scan(true, |last_had_children, layer| {
				if *last_had_children {
					*last_had_children = layer.has_children(self.network_interface.document_metadata());
					Some(layer)
				} else {
					None
				}
			})
	}

	/// Find the deepest layer that has been clicked on from a location in viewport space.
	pub fn click(&self, ipp: &InputPreprocessorMessageHandler) -> Option<LayerNodeIdentifier> {
		self.click_list(ipp).last()
	}

	/// Get the combined bounding box of the click targets of the selected visible layers in viewport space
	pub fn selected_visible_layers_bounding_box_viewport(&self) -> Option<[DVec2; 2]> {
		self.network_interface
			.selected_nodes(&[])
			.unwrap()
			.selected_visible_layers(&self.network_interface)
			.filter_map(|layer| self.metadata().bounding_box_viewport(layer))
			.reduce(graphene_core::renderer::Quad::combine_bounds)
	}

	pub fn selected_visible_and_unlock_layers_bounding_box_viewport(&self) -> Option<[DVec2; 2]> {
		self.network_interface
			.selected_nodes(&[])
			.unwrap()
			.selected_visible_and_unlocked_layers(&self.network_interface)
			.filter_map(|layer| self.metadata().bounding_box_viewport(layer))
			.reduce(graphene_core::renderer::Quad::combine_bounds)
	}

	pub fn document_network(&self) -> &NodeNetwork {
		self.network_interface.network(&[]).unwrap()
	}

	pub fn metadata(&self) -> &DocumentMetadata {
		self.network_interface.document_metadata()
	}

	pub fn serialize_document(&self) -> String {
		let val = serde_json::to_string(self);
		// We fully expect the serialization to succeed
		val.unwrap()
	}

	pub fn deserialize_document(serialized_content: &str) -> Result<Self, EditorError> {
		let document_message_handler = serde_json::from_str::<OldDocumentMessageHandler>(serialized_content)
			.map_or_else(
				|_| serde_json::from_str::<DocumentMessageHandler>(serialized_content),
				|old_message_handler| {
					let default_document_message_handler = DocumentMessageHandler {
						network_interface: NodeNetworkInterface::from_old_network(old_message_handler.network),
						collapsed: old_message_handler.collapsed,
						commit_hash: old_message_handler.commit_hash,
						document_ptz: old_message_handler.document_ptz,
						document_mode: old_message_handler.document_mode,
						view_mode: old_message_handler.view_mode,
						overlays_visible: old_message_handler.overlays_visible,
						rulers_visible: old_message_handler.rulers_visible,
						graph_view_overlay_open: old_message_handler.graph_view_overlay_open,
						snapping_state: old_message_handler.snapping_state,
						..Default::default()
					};
					Ok(default_document_message_handler)
				},
			)
			.map_err(|e| EditorError::DocumentDeserialization(e.to_string()))?;
		Ok(document_message_handler)
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
		let mut structure_section = vec![NodeId(0).0];
		let mut data_section = Vec::new();
		self.serialize_structure(LayerNodeIdentifier::ROOT_PARENT, &mut structure_section, &mut data_section, &mut vec![]);

		// Remove the ROOT element. Prepend `L`, the length (excluding the ROOT) of the structure section (which happens to be where the ROOT element was).
		structure_section[0] = structure_section.len() as u64 - 1;
		// Append the data section to the end.
		structure_section.extend(data_section);

		structure_section.as_slice().into()
	}

	pub fn undo_with_history(&mut self, ipp: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(previous_network) = self.undo(ipp, responses) else { return };

		self.document_redo_history.push_back(previous_network);
		if self.document_redo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			self.document_redo_history.pop_front();
		}
	}

	pub fn undo(&mut self, ipp: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> Option<NodeNetworkInterface> {
		// If there is no history return and don't broadcast SelectionChanged
		let mut network_interface = self.document_undo_history.pop_back()?;

		// Set the previous network navigation metadata to the current navigation metadata
		network_interface.copy_all_navigation_metadata(&self.network_interface);
		std::mem::swap(&mut network_interface.resolved_types, &mut self.network_interface.resolved_types);

		//Update the metadata transform based on document PTZ
		let transform = self.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.center(), &self.document_ptz);
		network_interface.set_document_to_viewport_transform(transform);

		// Ensure document structure is loaded so that updating the selected nodes has the correct metadata
		network_interface.load_structure();

		let previous_network = std::mem::replace(&mut self.network_interface, network_interface);

		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		responses.add(NodeGraphMessage::SelectedNodesUpdated);
		responses.add(NodeGraphMessage::ForceRunDocumentGraph);
		// TODO: Remove once the footprint is used to load the imports/export distances from the edge
		responses.add(NodeGraphMessage::SetGridAlignedEdges);
		responses.add(Message::StartBuffer);
		Some(previous_network)
	}
	pub fn redo_with_history(&mut self, ipp: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		// Push the UpdateOpenDocumentsList message to the queue in order to update the save status of the open documents
		let Some(previous_network) = self.redo(ipp, responses) else { return };

		self.document_undo_history.push_back(previous_network);
		if self.document_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			self.document_undo_history.pop_front();
		}
	}

	pub fn redo(&mut self, ipp: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> Option<NodeNetworkInterface> {
		// If there is no history return and don't broadcast SelectionChanged
		let mut network_interface = self.document_redo_history.pop_back()?;

		// Set the previous network navigation metadata to the current navigation metadata
		network_interface.copy_all_navigation_metadata(&self.network_interface);

		//Update the metadata transform based on document PTZ
		let transform = self.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.center(), &self.document_ptz);
		network_interface.set_document_to_viewport_transform(transform);

		let previous_network = std::mem::replace(&mut self.network_interface, network_interface);
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		responses.add(NodeGraphMessage::SelectedNodesUpdated);
		responses.add(NodeGraphMessage::ForceRunDocumentGraph);

		Some(previous_network)
	}

	pub fn current_hash(&self) -> Option<u64> {
		self.document_undo_history.iter().last().map(|network| network.network(&[]).unwrap().current_hash())
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

	/// Finds the parent folder which, based on the current selections, should be the container of any newly added layers.
	pub fn new_layer_parent(&self, include_self: bool) -> LayerNodeIdentifier {
		self.network_interface
			.deepest_common_ancestor(&self.selection_network_path, include_self)
			.unwrap_or_else(|| self.network_interface.all_artboards().iter().next().copied().unwrap_or(LayerNodeIdentifier::ROOT_PARENT))
	}

	pub fn get_calculated_insert_index(metadata: &DocumentMetadata, selected_nodes: SelectedNodes, parent: LayerNodeIdentifier) -> usize {
		parent
			.children(metadata)
			.enumerate()
			.find_map(|(index, direct_child)| {
				if selected_nodes.selected_layers(metadata).any(|selected| selected == direct_child) {
					return Some(index);
				}

				for descendant in direct_child.descendants(metadata) {
					if selected_nodes.selected_layers(metadata).any(|selected| selected == descendant) {
						return Some(index);
					}
				}

				None
			})
			.unwrap_or(0)
	}

	/// Loads layer resources such as creating the blob URLs for the images and loading all of the fonts in the document.
	pub fn load_layer_resources(&self, responses: &mut VecDeque<Message>) {
		let mut fonts = HashSet::new();
		for (_node_id, node) in self.document_network().recursive_nodes() {
			for input in &node.inputs {
				if let Some(TaggedValue::Font(font)) = input.as_value() {
					fonts.insert(font.clone());
				}
			}
		}
		for font in fonts {
			responses.add_front(FrontendMessage::TriggerFontLoad { font });
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
							.on_commit(|_| DialogMessage::RequestComingSoonDialog { issue: Some(330) }.into()),
						MenuListEntry::new(format!("{:?}", DocumentMode::GuideMode))
							.label(DocumentMode::GuideMode.to_string())
							.icon(DocumentMode::GuideMode.icon_name())
							.on_commit(|_| DialogMessage::RequestComingSoonDialog { issue: Some(331) }.into()),
					]])
					.selected_index(Some(self.document_mode as u32))
					.draw_icon(true)
					.interactive(false) // TODO: set to true when dialogs are not spawned
					.widget_holder(),
				Separator::new(SeparatorType::Section).widget_holder(),
			],
		}]);

		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(document_mode_layout),
			layout_target: LayoutTarget::DocumentMode,
		});

		// Document bar (right portion of the bar above the viewport)

		let mut snapping_state = self.snapping_state.clone();
		let mut snapping_state2 = self.snapping_state.clone();

		let mut widgets = vec![
			CheckboxInput::new(self.overlays_visible)
				.icon("Overlays")
				.tooltip("Overlays")
				.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::ToggleOverlaysVisibility))
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
				.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::ToggleSnapping))
				.on_update(move |optional_input: &CheckboxInput| {
					DocumentMessage::SetSnapping {
						closure: Some(|snapping_state| &mut snapping_state.snapping_enabled),
						snapping_state: optional_input.checked,
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
					.chain(GET_SNAP_BOX_FUNCTIONS.into_iter().map(|(name, closure)| LayoutGroup::Row {
						widgets: vec![
									CheckboxInput::new(*closure(&mut snapping_state))
										.on_update(move |input: &CheckboxInput| DocumentMessage::SetSnapping { closure: Some(closure), snapping_state: input.checked }.into())
										.widget_holder(),
									TextLabel::new(name).widget_holder(),
								],
					}))
					.chain(
						[LayoutGroup::Row {
							widgets: vec![TextLabel::new(SnappingOptions::Geometry.to_string()).widget_holder()],
						}]
						.into_iter()
						.chain(GET_SNAP_GEOMETRY_FUNCTIONS.into_iter().map(|(name, closure)| LayoutGroup::Row {
							widgets: vec![
									CheckboxInput::new(*closure(&mut snapping_state2))
										.on_update(move |input: &CheckboxInput| DocumentMessage::SetSnapping { closure: Some(closure), snapping_state: input.checked }.into())
										.widget_holder(),
									TextLabel::new(name).widget_holder(),
								],
						})),
					)
					.collect(),
				)
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			CheckboxInput::new(self.snapping_state.grid_snapping)
				.icon("Grid")
				.tooltip("Grid")
				.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::ToggleGridVisibility))
				.on_update(|optional_input: &CheckboxInput| DocumentMessage::GridVisibility(optional_input.checked).into())
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
		];

		widgets.extend(navigation_controls(&self.document_ptz, &self.navigation_handler, "Canvas"));

		let tilt_value = self.navigation_handler.snapped_tilt(self.document_ptz.tilt()) / (std::f64::consts::PI / 180.);
		if tilt_value.abs() > 0.00001 {
			widgets.extend([
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(tilt_value))
					.unit("°")
					.increment_behavior(NumberInputIncrementBehavior::Callback)
					.increment_callback_increase(|number_input: &NumberInput| {
						let one = 1. + f64::EPSILON * 100.;
						NavigationMessage::CanvasTiltSet {
							angle_radians: ((number_input.value.unwrap() / VIEWPORT_ROTATE_SNAP_INTERVAL + one).floor() * VIEWPORT_ROTATE_SNAP_INTERVAL).to_radians(),
						}
						.into()
					})
					.increment_callback_decrease(|number_input: &NumberInput| {
						let one = 1. + f64::EPSILON * 100.;
						NavigationMessage::CanvasTiltSet {
							angle_radians: ((number_input.value.unwrap() / VIEWPORT_ROTATE_SNAP_INTERVAL - one).ceil() * VIEWPORT_ROTATE_SNAP_INTERVAL).to_radians(),
						}
						.into()
					})
					.tooltip("Canvas Tilt")
					.on_update(|number_input: &NumberInput| {
						NavigationMessage::CanvasTiltSet {
							angle_radians: number_input.value.unwrap().to_radians(),
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
		let selected_nodes = self.network_interface.selected_nodes(&[]).unwrap();
		let selected_layers_except_artboards = selected_nodes.selected_layers_except_artboards(&self.network_interface);

		// Look up the current opacity and blend mode of the selected layers (if any), and split the iterator into the first tuple and the rest.
		let mut opacity_and_blend_mode = selected_layers_except_artboards.map(|layer| {
			(
				get_opacity(layer, &self.network_interface).unwrap_or(100.),
				get_blend_mode(layer, &self.network_interface).unwrap_or_default(),
			)
		});
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
							.on_commit(|_| DocumentMessage::AddTransaction.into())
					})
					.collect()
			})
			.collect();

		let has_selection = self.network_interface.selected_nodes(&[]).unwrap().selected_layers(self.metadata()).next().is_some();
		let selection_all_visible = self
			.network_interface
			.selected_nodes(&[])
			.unwrap()
			.selected_layers(self.metadata())
			.all(|layer| self.network_interface.is_visible(&layer.to_node(), &[]));
		let selection_all_locked = self
			.network_interface
			.selected_nodes(&[])
			.unwrap()
			.selected_layers(self.metadata())
			.all(|layer| self.network_interface.is_locked(&layer.to_node(), &[]));

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
					.on_commit(|_| DocumentMessage::AddTransaction.into())
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
					.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::ToggleSelectedLocked))
					.on_update(|_| NodeGraphMessage::ToggleSelectedLocked.into())
					.disabled(!has_selection)
					.widget_holder(),
				IconButton::new(if selection_all_visible { "EyeVisible" } else { "EyeHidden" }, 24)
					.hover_icon(Some((if selection_all_visible { "EyeHide" } else { "EyeShow" }).into()))
					.tooltip(if selection_all_visible { "Hide Selected" } else { "Show Selected" })
					.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::ToggleSelectedVisibility))
					.on_update(|_| DocumentMessage::ToggleSelectedVisibility.into())
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
		let selected_nodes = self.network_interface.selected_nodes(&[]).unwrap();
		let mut selected_layers = selected_nodes.selected_layers(self.metadata());

		let first_or_last_selected_layer = match relative_index_offset.signum() {
			-1 => selected_layers.next(),
			1 => selected_layers.last(),
			_ => panic!("selected_layers_reorder() must be given a non-zero value"),
		};

		let Some(pivot_layer) = first_or_last_selected_layer else {
			return;
		};
		let parent = pivot_layer.parent(self.metadata()).unwrap_or(LayerNodeIdentifier::ROOT_PARENT);

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
		let insert_index = if relative_index_offset < 0 { neighbor_index } else { neighbor_index + 1 };
		responses.add(DocumentMessage::MoveSelectedLayersTo { parent, insert_index });
	}
}

/// Create a network interface with a single export
fn default_document_network_interface() -> NodeNetworkInterface {
	let mut network_interface = NodeNetworkInterface::default();
	network_interface.add_export(TaggedValue::ArtboardGroup(graphene_core::ArtboardGroup::EMPTY), -1, "".to_string(), &[]);
	network_interface
}

/// Targets for the [`ClickXRayIter`]. In order to reduce computation, we prefer just a point/path test where possible.
#[derive(Clone)]
enum XRayTarget {
	Point(DVec2),
	Quad(Quad),
	Path(Vec<path_bool_lib::PathSegment>),
}

/// The result for the [`ClickXRayIter`] on the layer
struct XRayResult {
	clicked: bool,
	use_children: bool,
}

/// An iterator for finding layers within an [`XRayTarget`]. Constructed by [`DocumentMessageHandler::intersect_quad`] and [`DocumentMessageHandler::click_xray`].
#[derive(Clone)]
pub struct ClickXRayIter<'a> {
	next_layer: Option<LayerNodeIdentifier>,
	network_interface: &'a NodeNetworkInterface,
	parent_targets: Vec<(LayerNodeIdentifier, XRayTarget)>,
}

fn quad_to_path_lib_segments(quad: Quad) -> Vec<path_bool_lib::PathSegment> {
	quad.edges().into_iter().map(|[start, end]| path_bool_lib::PathSegment::Line(start, end)).collect()
}

fn click_targets_to_path_lib_segments<'a>(click_targets: impl Iterator<Item = &'a ClickTarget>, transform: DAffine2) -> Vec<path_bool_lib::PathSegment> {
	let segment = |bezier: bezier_rs::Bezier| match bezier.handles {
		bezier_rs::BezierHandles::Linear => path_bool_lib::PathSegment::Line(bezier.start, bezier.end),
		bezier_rs::BezierHandles::Quadratic { handle } => path_bool_lib::PathSegment::Quadratic(bezier.start, handle, bezier.end),
		bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => path_bool_lib::PathSegment::Cubic(bezier.start, handle_start, handle_end, bezier.end),
	};
	click_targets
		.flat_map(|target| target.subpath().iter())
		.map(|bezier| segment(bezier.apply_transformation(|x| transform.transform_point2(x))))
		.collect()
}

impl<'a> ClickXRayIter<'a> {
	fn new(network_interface: &'a NodeNetworkInterface, target: XRayTarget) -> Self {
		if let Some(first_layer) = LayerNodeIdentifier::ROOT_PARENT.first_child(network_interface.document_metadata()) {
			Self {
				network_interface,
				next_layer: Some(first_layer),
				parent_targets: vec![(LayerNodeIdentifier::ROOT_PARENT, target)],
			}
		} else {
			Self {
				network_interface,
				next_layer: Default::default(),
				parent_targets: Default::default(),
			}
		}
	}

	/// Handles the checking of the layer where the target is a rect or path
	fn check_layer_area_target(&mut self, click_targets: Option<&Vec<ClickTarget>>, clip: bool, layer: LayerNodeIdentifier, path: Vec<path_bool_lib::PathSegment>, transform: DAffine2) -> XRayResult {
		// Convert back to Bezier-rs types for intersections
		let segment = |bezier: &path_bool_lib::PathSegment| match *bezier {
			path_bool_lib::PathSegment::Line(start, end) => bezier_rs::Bezier::from_linear_dvec2(start, end),
			path_bool_lib::PathSegment::Cubic(start, h1, h2, end) => bezier_rs::Bezier::from_cubic_dvec2(start, h1, h2, end),
			path_bool_lib::PathSegment::Quadratic(start, h1, end) => bezier_rs::Bezier::from_quadratic_dvec2(start, h1, end),
			path_bool_lib::PathSegment::Arc(_, _, _, _, _, _, _) => unimplemented!(),
		};
		let get_clip = || path.iter().map(segment);

		let intersects = click_targets.map_or(false, |targets| targets.iter().any(|target| target.intersect_path(get_clip, transform)));
		let clicked = intersects;
		let mut use_children = !clip || intersects;

		// In the case of a clip path where the area partially intersects, it is necessary to do a boolean operation.
		// We do this on this using the target area to reduce computation (as the target area is usually very simple).
		if clip && intersects {
			let clip_path = click_targets_to_path_lib_segments(click_targets.iter().flat_map(|x| x.iter()), transform);
			let subtracted = graphene_std::vector::boolean_intersect(path, clip_path).into_iter().flatten().collect::<Vec<_>>();
			if subtracted.is_empty() {
				use_children = false;
			} else {
				// All child layers will use the new clipped target area
				self.parent_targets.push((layer, XRayTarget::Path(subtracted)));
			}
		}
		XRayResult { clicked, use_children }
	}

	/// Handles the checking of the layer to find if it has been clicked
	fn check_layer(&mut self, layer: LayerNodeIdentifier) -> XRayResult {
		let selected_layers = self.network_interface.selected_nodes(&[]).unwrap();
		// Discard invisible and locked layers
		if !selected_layers.layer_visible(layer, self.network_interface) || selected_layers.layer_locked(layer, self.network_interface) {
			return XRayResult { clicked: false, use_children: false };
		}

		let click_targets = self.network_interface.document_metadata().click_targets(layer);
		let transform = self.network_interface.document_metadata().transform_to_document(layer);
		let target = &self.parent_targets.last().expect("In `check_layer()`: there should be a `target`").1;
		let clip = self.network_interface.document_metadata().is_clip(layer.to_node());

		match target {
			// Single points are much cheaper than paths so have their own special case
			XRayTarget::Point(point) => {
				let intersects = click_targets.map_or(false, |targets| targets.iter().any(|target| target.intersect_point(*point, transform)));
				XRayResult {
					clicked: intersects,
					use_children: !clip || intersects,
				}
			}
			XRayTarget::Quad(quad) => self.check_layer_area_target(click_targets, clip, layer, quad_to_path_lib_segments(*quad), transform),
			XRayTarget::Path(path) => self.check_layer_area_target(click_targets, clip, layer, path.clone(), transform),
		}
	}
}

pub fn navigation_controls(ptz: &PTZ, navigation_handler: &NavigationMessageHandler, tooltip_name: &str) -> [WidgetHolder; 6] {
	[
		IconButton::new("ZoomIn", 24)
			.tooltip("Zoom In")
			.tooltip_shortcut(action_keys!(NavigationMessageDiscriminant::CanvasZoomIncrease))
			.on_update(|_| NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }.into())
			.widget_holder(),
		IconButton::new("ZoomOut", 24)
			.tooltip("Zoom Out")
			.tooltip_shortcut(action_keys!(NavigationMessageDiscriminant::CanvasZoomDecrease))
			.on_update(|_| NavigationMessage::CanvasZoomDecrease { center_on_mouse: false }.into())
			.widget_holder(),
		IconButton::new("ZoomReset", 24)
			.tooltip("Reset Tilt and Zoom to 100%")
			.tooltip_shortcut(action_keys!(NavigationMessageDiscriminant::CanvasTiltResetAndZoomTo100Percent))
			.on_update(|_| NavigationMessage::CanvasTiltResetAndZoomTo100Percent.into())
			.disabled(ptz.tilt().abs() < 1e-4 && (ptz.zoom() - 1.).abs() < 1e-4)
			.widget_holder(),
		PopoverButton::new()
			.popover_layout(vec![
				LayoutGroup::Row {
					widgets: vec![TextLabel::new(format!("{tooltip_name} Navigation")).bold(true).widget_holder()],
				},
				LayoutGroup::Row {
					widgets: vec![TextLabel::new({
						let tilt = if tooltip_name == "Canvas" { "Tilt:\n• Alt + Middle Click Drag\n\n" } else { "" };
						format!(
							"
							Interactive controls in this\n\
							menu are coming soon.\n\
							\n\
							Pan:\n\
							• Middle Click Drag\n\
							\n\
							{tilt}Zoom:\n\
							• Shift + Middle Click Drag\n\
							• Ctrl + Scroll Wheel Roll
							"
						)
						.trim()
					})
					.multiline(true)
					.widget_holder()],
				},
			])
			.widget_holder(),
		Separator::new(SeparatorType::Related).widget_holder(),
		NumberInput::new(Some(navigation_handler.snapped_zoom(ptz.zoom()) * 100.))
			.unit("%")
			.min(0.000001)
			.max(1000000.)
			.tooltip(format!("{tooltip_name} Zoom"))
			.on_update(|number_input: &NumberInput| {
				NavigationMessage::CanvasZoomSet {
					zoom_factor: number_input.value.unwrap() / 100.,
				}
				.into()
			})
			.increment_behavior(NumberInputIncrementBehavior::Callback)
			.increment_callback_decrease(|_| NavigationMessage::CanvasZoomDecrease { center_on_mouse: false }.into())
			.increment_callback_increase(|_| NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }.into())
			.widget_holder(),
	]
}

impl<'a> Iterator for ClickXRayIter<'a> {
	type Item = LayerNodeIdentifier;

	fn next(&mut self) -> Option<Self::Item> {
		// While there are still layers in the layer tree
		while let Some(layer) = self.next_layer.take() {
			let XRayResult { clicked, use_children } = self.check_layer(layer);
			let metadata = self.network_interface.document_metadata();
			// If we should use the children and also there is a child, that child is the next layer.
			self.next_layer = use_children.then(|| layer.first_child(metadata)).flatten();

			// If we aren't using children, iterate up the ancestors until there is a layer with a sibling
			for ancestor in layer.ancestors(metadata) {
				if self.next_layer.is_some() {
					break;
				}
				// If there is a clipped area for this ancestor (that we are now exiting), discard it.
				if self.parent_targets.last().is_some_and(|(id, _)| *id == ancestor) {
					self.parent_targets.pop();
				}
				self.next_layer = ancestor.next_sibling(metadata)
			}

			if clicked {
				return Some(layer);
			}
		}
		assert!(self.parent_targets.is_empty(), "The parent targets should always be empty (since we have left all layers)");
		None
	}
}

// TODO: Eventually remove this (probably starting late 2024)
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct OldDocumentMessageHandler {
	// ============================================
	// Fields that are saved in the document format
	// ============================================
	//
	/// The node graph that generates this document's artwork.
	/// It recursively stores its sub-graphs, so this root graph is the whole snapshot of the document content.
	pub network: OldNodeNetwork,
	/// List of the [`NodeId`]s that are currently selected by the user.
	pub selected_nodes: SelectedNodes,
	/// List of the [`LayerNodeIdentifier`]s that are currently collapsed by the user in the Layers panel.
	/// Collapsed means that the expansion arrow isn't set to show the children of these layers.
	pub collapsed: CollapsedLayers,
	/// The name of the document, which is displayed in the tab and title bar of the editor.
	pub name: String,
	/// The full Git commit hash of the Graphite repository that was used to build the editor.
	/// We save this to provide a hint about which version of the editor was used to create the document.
	pub commit_hash: String,
	/// The current pan, tilt, and zoom state of the viewport's view of the document canvas.
	pub document_ptz: PTZ,
	/// The current mode that the document is in, which starts out as Design Mode. This choice affects the editing behavior of the tools.
	pub document_mode: DocumentMode,
	/// The current view mode that the user has set for rendering the document within the viewport.
	/// This is usually "Normal" but can be set to "Outline" or "Pixels" to see the canvas differently.
	pub view_mode: ViewMode,
	/// Sets whether or not all the viewport overlays should be drawn on top of the artwork.
	/// This includes tool interaction visualizations (like the transform cage and path anchors/handles), the grid, and more.
	pub overlays_visible: bool,
	/// Sets whether or not the rulers should be drawn along the top and left edges of the viewport area.
	pub rulers_visible: bool,
	/// Sets whether or not the node graph is drawn (as an overlay) on top of the viewport area, or otherwise if it's hidden.
	pub graph_view_overlay_open: bool,
	/// The current user choices for snapping behavior, including whether snapping is enabled at all.
	pub snapping_state: SnappingState,
}
