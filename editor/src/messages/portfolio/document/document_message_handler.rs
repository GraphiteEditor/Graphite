use super::node_graph::document_node_definitions;
use super::utility_types::error::EditorError;
use super::utility_types::misc::{GroupFolderType, SNAP_FUNCTIONS_FOR_BOUNDING_BOXES, SNAP_FUNCTIONS_FOR_PATHS, SnappingOptions, SnappingState};
use super::utility_types::network_interface::{self, NodeNetworkInterface, TransactionStatus};
use super::utility_types::nodes::{CollapsedLayers, LayerStructureEntry, SelectedNodes};
use crate::application::{GRAPHITE_GIT_COMMIT_HASH, generate_uuid};
use crate::consts::{
	ASYMPTOTIC_EFFECT, BLEND_COUNT_PER_LAYER, COLOR_OVERLAY_GRAY, DEFAULT_DOCUMENT_NAME, FILE_EXTENSION, GDD_FILE_EXTENSION, LAYER_INDENT_OFFSET, NODE_CHAIN_WIDTH, SCALE_EFFECT, SCROLLBAR_SPACING,
	VIEWPORT_ROTATE_SNAP_INTERVAL,
};
use crate::messages::input_mapper::utility_types::macros::action_shortcut;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::data_panel::{DataPanelMessageContext, DataPanelMessageHandler};
use crate::messages::portfolio::document::graph_operation::utility_types::{ModifyInputsContext, TransformIn};
use crate::messages::portfolio::document::node_graph::NodeGraphMessageContext;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::node_graph::utility_types::FrontendGraphDataType;
use crate::messages::portfolio::document::overlays::grid_overlays::{grid_overlay, overlay_options};
use crate::messages::portfolio::document::overlays::utility_types::{OverlaysType, OverlaysVisibilitySettings, Pivot};
use crate::messages::portfolio::document::properties_panel::properties_panel_message_handler::PropertiesPanelMessageContext;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis, PTZ};
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, InputConnector, NodeTemplate, OutputConnector};
use crate::messages::portfolio::utility_types::PanelType;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::{self, get_blend_mode, get_fill, get_opacity};
use crate::messages::tool::common_functionality::utility_functions::nudge_resize_bounds;
use crate::messages::tool::tool_messages::select_tool::SelectToolPointerKeys;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::ToolType;
use crate::node_graph_executor::NodeGraphExecutor;
use glam::{DAffine2, DVec2};
use graph_craft::application_io::resource::ResourceId;
use graph_craft::application_io::wgpu_available;
use graph_craft::descriptor;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput, NodeNetwork, OldNodeNetwork};
use graphene_std::math::quad::Quad;
use graphene_std::path_bool_nodes::boolean_intersect;
use graphene_std::raster::BlendMode;
use graphene_std::subpath::Subpath;
use graphene_std::vector::PointId;
use graphene_std::vector::click_target::{ClickTarget, ClickTargetType};
use graphene_std::vector::misc::dvec2_to_point;
use graphene_std::vector::style::{Fill, RenderMode};
use kurbo::{Affine, BezPath, Line, PathSeg};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(ExtractField)]
pub struct DocumentMessageContext<'a> {
	pub document_id: DocumentId,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub executor: &'a mut NodeGraphExecutor,
	pub current_tool: &'a ToolType,
	pub preferences: &'a PreferencesMessageHandler,
	pub data_panel_open: bool,
	pub layers_panel_open: bool,
	pub properties_panel_open: bool,
	pub viewport: &'a ViewportMessageHandler,
	pub resource_storage: &'a ResourceStorageMessageHandler,
	pub fonts: &'a FontsMessageHandler,
}

#[derive(derivative::Derivative, serde::Serialize, serde::Deserialize, ExtractField)]
#[derivative(Clone, Debug)]
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
	pub overlays_message_handler: OverlaysMessageHandler,
	#[serde(skip)]
	pub properties_panel_message_handler: PropertiesPanelMessageHandler,
	#[serde(skip)]
	pub data_panel_message_handler: DataPanelMessageHandler,

	// ============================================
	// Fields that are saved in the document format
	// ============================================
	//
	// Contains the NodeNetwork and acts an an interface to manipulate the NodeNetwork with custom setters in order to keep NetworkMetadata in sync
	pub network_interface: NodeNetworkInterface,
	/// Resources embedded in the document.
	#[serde(default, skip_serializing_if = "ResourceMessageHandler::is_empty")]
	pub resources: ResourceMessageHandler,
	/// Per-document `Gdd` working copy: owns the CRDT `Session` and mirrors edits to disk. `None`
	/// until the mount future built by `load_document` resolves (the working-copy container is
	/// constructed asynchronously). Not serialized. A clone shares the same working-copy container
	/// (`Gdd` holds an `Arc<AnyContainer>`), so a cloned document still reads the live working copy.
	#[serde(skip, default)]
	#[derivative(Debug = "ignore")]
	pub storage: Option<document_format::GddV1>,
	/// Tracks which layer occurrences are collapsed in the Layers panel, keyed by tree path.
	#[serde(deserialize_with = "deserialize_collapsed_layers", default)]
	pub collapsed: CollapsedLayers,
	/// The node IDs whose section is collapsed in the Properties panel.
	#[serde(default)]
	pub properties_panel_collapsed_sections: Vec<NodeId>,
	/// The full Git commit hash of the Graphite repository that was used to build the editor.
	/// We save this to provide a hint about which version of the editor was used to create the document.
	pub commit_hash: String,
	/// The current pan, tilt, and zoom state of the viewport's view of the document canvas.
	pub document_ptz: PTZ,
	/// The current mode that the user has set for rendering the document within the viewport.
	/// This is usually "Normal" but can be set to "Outline" or "Pixels" to see the canvas differently.
	pub render_mode: RenderMode,
	/// Sets whether or not all the viewport overlays should be drawn on top of the artwork.
	/// This includes tool interaction visualizations (like the transform cage and path anchors/handles), the grid, and more.
	pub overlays_visibility_settings: OverlaysVisibilitySettings,
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
	/// The name of the document, which is displayed in the tab and title bar of the editor.
	#[serde(skip)]
	pub name: String,
	/// The path of the to the document file.
	#[serde(skip)]
	pub(crate) path: Option<PathBuf>,
	// TODO: Eventually remove this document upgrade code
	/// Set when a freshly-opened document still has legacy bounding-box-relative gradients; the deferred gradient
	/// migration converts them to absolute after the first graph run (when geometry bounds are available) and clears this.
	#[serde(skip)]
	pub(crate) pending_gradient_migration: bool,
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
	/// Whether or not the editor has executed the network to render the document yet. If this is opened as an inactive tab, it won't be loaded initially because the active tab is prioritized.
	#[serde(skip)]
	pub is_loaded: bool,
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
			data_panel_message_handler: DataPanelMessageHandler::default(),
			// ============================================
			// Fields that are saved in the document format
			// ============================================
			network_interface: default_document_network_interface(),
			resources: ResourceMessageHandler::default(),
			storage: None,
			collapsed: CollapsedLayers::default(),
			properties_panel_collapsed_sections: Vec::new(),
			commit_hash: GRAPHITE_GIT_COMMIT_HASH.to_string(),
			document_ptz: PTZ::default(),
			render_mode: RenderMode::default(),
			overlays_visibility_settings: OverlaysVisibilitySettings::default(),
			rulers_visible: true,
			graph_view_overlay_open: false,
			snapping_state: SnappingState::default(),
			graph_fade_artwork_percentage: 80.,
			// =============================================
			// Fields omitted from the saved document format
			// =============================================
			name: DEFAULT_DOCUMENT_NAME.to_string(),
			path: None,
			// TODO: Eventually remove this document upgrade code
			pending_gradient_migration: false,
			breadcrumb_network_path: Vec::new(),
			selection_network_path: Vec::new(),
			document_undo_history: VecDeque::new(),
			document_redo_history: VecDeque::new(),
			saved_hash: None,
			auto_saved_hash: None,
			layer_range_selection_reference: None,
			is_loaded: false,
		}
	}
}

#[message_handler_data]
impl MessageHandler<DocumentMessage, DocumentMessageContext<'_>> for DocumentMessageHandler {
	fn process_message(&mut self, message: DocumentMessage, responses: &mut VecDeque<Message>, context: DocumentMessageContext) {
		let DocumentMessageContext {
			document_id,
			ipp,
			executor,
			viewport,
			current_tool,
			preferences,
			data_panel_open,
			layers_panel_open,
			properties_panel_open,
			resource_storage,
			fonts,
		} = context;

		match message {
			// Sub-messages
			DocumentMessage::Navigation(message) => {
				let context = NavigationMessageContext {
					network_interface: &mut self.network_interface,
					breadcrumb_network_path: &self.breadcrumb_network_path,
					ipp,
					document_ptz: &mut self.document_ptz,
					graph_view_overlay_open: self.graph_view_overlay_open,
					preferences,
					viewport,
				};

				self.navigation_handler.process_message(message, responses, context);
			}
			DocumentMessage::Overlays(message) => {
				let visibility_settings = self.overlays_visibility_settings;

				// Send the overlays message to the overlays message handler
				self.overlays_message_handler
					.process_message(message, responses, OverlaysMessageContext { visibility_settings, viewport });
			}
			DocumentMessage::PropertiesPanel(message) => {
				let context = PropertiesPanelMessageContext {
					executor,
					network_interface: &mut self.network_interface,
					resources: &self.resources,
					selection_network_path: &self.selection_network_path,
					document_name: self.name.as_str(),
					fonts,
					properties_panel_open,
					properties_panel_collapsed_sections: &self.properties_panel_collapsed_sections,
				};
				self.properties_panel_message_handler.process_message(message, responses, context);
			}
			DocumentMessage::DataPanel(message) => {
				self.data_panel_message_handler.process_message(
					message,
					responses,
					DataPanelMessageContext {
						network_interface: &mut self.network_interface,
						data_panel_open,
					},
				);
			}
			DocumentMessage::NodeGraph(message) => {
				self.node_graph_handler.process_message(
					message,
					responses,
					NodeGraphMessageContext {
						network_interface: &mut self.network_interface,
						selection_network_path: &self.selection_network_path,
						breadcrumb_network_path: &self.breadcrumb_network_path,
						document_id,
						collapsed: &mut self.collapsed,
						properties_panel_collapsed_sections: &mut self.properties_panel_collapsed_sections,
						ipp,
						graph_view_overlay_open: self.graph_view_overlay_open,
						graph_fade_artwork_percentage: self.graph_fade_artwork_percentage,
						navigation_handler: &self.navigation_handler,
						preferences,
						layers_panel_open,
						viewport,
					},
				);
			}
			DocumentMessage::GraphOperation(message) => {
				let context = GraphOperationMessageContext {
					network_interface: &mut self.network_interface,
					collapsed: &mut self.collapsed,
					node_graph: &mut self.node_graph_handler,
				};
				let mut graph_operation_message_handler = GraphOperationMessageHandler {};
				graph_operation_message_handler.process_message(message, responses, context);
			}
			DocumentMessage::Resource(message) => {
				let context = ResourceMessageContext { document_id, fonts };
				self.resources.process_message(message, responses, context);
			}
			DocumentMessage::AlignSelectedLayers { axis, aggregate } => {
				let axis = match axis {
					AlignAxis::X => DVec2::X,
					AlignAxis::Y => DVec2::Y,
				};
				let Some(combined_box) = self.network_interface.selected_layers_artwork_bounding_box_viewport() else {
					return;
				};

				let aggregated = match aggregate {
					AlignAggregate::Min => combined_box[0],
					AlignAggregate::Max => combined_box[1],
					AlignAggregate::Center => (combined_box[0] + combined_box[1]) / 2.,
				};

				let mut added_transaction = false;
				for layer in self.network_interface.selected_nodes().selected_unlocked_layers(&self.network_interface) {
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
			DocumentMessage::RemoveArtboards => {
				responses.add(GraphOperationMessage::RemoveArtboards);
			}
			DocumentMessage::ClearLayersPanel => {
				// Send an empty layer list
				if layers_panel_open {
					let layer_structure = Self::default().build_layer_structure();
					responses.add(FrontendMessage::UpdateDocumentLayerStructure { layer_structure });
				}

				// Clear the control bar
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::default(),
					layout_target: LayoutTarget::LayersPanelControlLeftBar,
				});
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::default(),
					layout_target: LayoutTarget::LayersPanelControlRightBar,
				});

				// Clear the bottom bar
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::default(),
					layout_target: LayoutTarget::LayersPanelBottomBar,
				});
			}
			DocumentMessage::CreateEmptyFolder => {
				let selected_nodes = self.network_interface.selected_nodes();
				let id = NodeId::new();

				let parent = self
					.network_interface
					.deepest_common_ancestor(&selected_nodes, &self.selection_network_path, true)
					.unwrap_or(LayerNodeIdentifier::ROOT_PARENT);

				let insert_index = DocumentMessageHandler::get_calculated_insert_index(self.metadata(), &self.network_interface.selected_nodes(), parent);
				responses.add(DocumentMessage::AddTransaction);
				responses.add(GraphOperationMessage::NewCustomLayer {
					id,
					nodes: Vec::new(),
					parent,
					insert_index,
				});
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });
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
			DocumentMessage::DocumentHistoryBackward => self.undo_with_history(document_id, viewport, resource_storage, responses),
			DocumentMessage::DocumentHistoryForward => self.redo_with_history(document_id, viewport, resource_storage, responses),
			DocumentMessage::DocumentStructureChanged => {
				if layers_panel_open {
					self.network_interface.load_structure();
					let layer_structure = self.build_layer_structure();

					self.update_layers_panel_control_bar_widgets(layers_panel_open, responses);
					self.update_layers_panel_bottom_bar_widgets(layers_panel_open, responses);

					responses.add(FrontendMessage::UpdateDocumentLayerStructure { layer_structure });
				}
			}
			DocumentMessage::DrawArtboardOverlays { context: overlay_context } => {
				if !overlay_context.visibility_settings.artboard_name() {
					return;
				}

				for layer in self.metadata().all_layers() {
					if !self.network_interface.is_artboard(&layer.to_node(), &[]) {
						continue;
					}
					let Some(bounds) = self.metadata().bounding_box_document(layer) else { continue };
					let min = bounds[0].min(bounds[1]);
					let max = bounds[0].max(bounds[1]);

					let name = self.network_interface.display_name(&layer.to_node(), &[]);

					// Calculate position of the text
					let corner_pos = if !self.document_ptz.flip {
						// Use the top-left corner
						min
					} else {
						// Use the top-right corner, which appears to be the top-left due to being flipped
						DVec2::new(max.x, min.y)
					};

					// When the canvas is flipped, mirror the text so it appears correctly
					let scale = if !self.document_ptz.flip { DVec2::ONE } else { DVec2::new(-1., 1.) };

					// Create a transform that puts the text at the true top-left regardless of flip
					let transform = self.metadata().document_to_viewport
						* DAffine2::from_translation(corner_pos)
						* DAffine2::from_scale(DVec2::splat(self.document_ptz.zoom().recip()))
						* DAffine2::from_translation(-DVec2::Y * 4.)
						* DAffine2::from_scale(scale);

					overlay_context.text(&name, COLOR_OVERLAY_GRAY, None, transform, 0., [Pivot::Start, Pivot::End]);
				}
			}
			DocumentMessage::DuplicateSelectedLayers => {
				responses.add(DocumentMessage::AddTransaction);

				let mut new_dragging = Vec::new();
				let mut layers = self.network_interface.shallowest_unique_layers(&[]).collect::<Vec<_>>();

				layers.sort_by_key(|layer| {
					let Some(parent) = layer.parent(self.metadata()) else { return usize::MAX };
					DocumentMessageHandler::get_calculated_insert_index(self.metadata(), &SelectedNodes(vec![layer.to_node()]), parent)
				});

				for original_layer in layers.into_iter().rev() {
					let Some(parent) = original_layer.parent(self.metadata()) else { continue };
					let insert_index = DocumentMessageHandler::get_calculated_insert_index(self.metadata(), &SelectedNodes(vec![original_layer.to_node()]), parent);

					let Some(new_layer) = self.duplicate_layer(original_layer, responses) else { continue };
					new_dragging.push(new_layer);
					responses.add(NodeGraphMessage::MoveLayerToStack {
						layer: new_layer,
						parent,
						insert_index,
					});
				}
				let nodes = new_dragging.iter().map(|layer| layer.to_node()).collect();
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			DocumentMessage::DuplicateSelectedLayersTo { parent, insert_index } => {
				if !self.selection_network_path.is_empty() {
					log::error!("Duplicating selected layers is only supported for the document network");
					return;
				}

				// Mirror the placement constraints enforced when moving layers so a copy can't land somewhere a move couldn't
				let any_artboards = self
					.network_interface
					.selected_nodes()
					.selected_layers(self.metadata())
					.any(|layer| self.network_interface.is_artboard(&layer.to_node(), &self.selection_network_path));
				if any_artboards && parent != LayerNodeIdentifier::ROOT_PARENT {
					return;
				}

				let selected_any_non_artboards = self
					.network_interface
					.selected_nodes()
					.selected_layers(self.metadata())
					.any(|layer| !self.network_interface.is_artboard(&layer.to_node(), &self.selection_network_path));
				let top_level_artboards = LayerNodeIdentifier::ROOT_PARENT
					.children(self.metadata())
					.any(|layer| self.network_interface.is_artboard(&layer.to_node(), &self.selection_network_path));
				if selected_any_non_artboards && parent == LayerNodeIdentifier::ROOT_PARENT && top_level_artboards {
					return;
				}

				let layers_to_duplicate = self.network_interface.shallowest_unique_layers_sorted(&self.selection_network_path);
				if layers_to_duplicate.is_empty() {
					return;
				}

				responses.add(DocumentMessage::AddTransaction);

				let mut new_layers = Vec::new();
				for layer in layers_to_duplicate {
					let Some(new_layer) = self.duplicate_layer(layer, responses) else { continue };

					// Insert each copy one slot below the previous so the duplicates keep their original top-to-bottom order
					let placement_index = insert_index + new_layers.len();
					new_layers.push(new_layer);
					responses.add(NodeGraphMessage::MoveLayerToStack {
						layer: new_layer,
						parent,
						insert_index: placement_index,
					});

					// Compensate the local transform so a copy dropped into a differently-transformed parent stays put in world space
					if layer.parent(self.metadata()) != Some(parent) {
						let layer_world_transform = self.network_interface.document_metadata().transform_to_viewport(layer);
						let undo_parent_transform = self.network_interface.document_metadata().transform_to_viewport(parent).inverse();

						responses.add(GraphOperationMessage::TransformSet {
							layer: new_layer,
							transform: undo_parent_transform * layer_world_transform,
							transform_in: TransformIn::Local,
							skip_rerender: false,
						});
					}
				}

				let nodes = new_layers.iter().map(|layer| layer.to_node()).collect();
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			DocumentMessage::EnterNestedNetwork { node_id } => {
				self.breadcrumb_network_path.push(node_id);
				self.selection_network_path.clone_from(&self.breadcrumb_network_path);
				responses.add(NodeGraphMessage::UnloadWires);
				responses.add(NodeGraphMessage::SendGraph);
				responses.add(DocumentMessage::ZoomCanvasToFitAll);
				responses.add(NodeGraphMessage::UpdateNodeGraphWidth);
			}
			DocumentMessage::Escape => {
				// Abort dragging nodes
				if self.node_graph_handler.drag_start.is_some() {
					responses.add(DocumentMessage::AbortTransaction);
					self.node_graph_handler.drag_start = None;
					self.node_graph_handler.select_if_not_dragged = None;
				}
				// Abort box selection
				else if self.node_graph_handler.box_selection_start.is_some() {
					self.node_graph_handler.box_selection_start = None;
					responses.add(NodeGraphMessage::SelectedNodesSet {
						nodes: self.node_graph_handler.selection_before_pointer_down.clone(),
					});
					responses.add(FrontendMessage::UpdateBox { box_selection: None });
				}
				// Abort wire in progress of being connected
				else if self.node_graph_handler.wire_in_progress_from_connector.is_some() {
					self.node_graph_handler.wire_in_progress_from_connector = None;
					self.node_graph_handler.wire_in_progress_to_connector = None;
					self.node_graph_handler.wire_in_progress_type = FrontendGraphDataType::General;

					responses.add(FrontendMessage::UpdateWirePathInProgress { wire_path: None });
					responses.add(DocumentMessage::AbortTransaction);
				}
				// Close the context menu if it's open
				else if self
					.node_graph_handler
					.context_menu
					.as_ref()
					.is_some_and(|context_menu| matches!(context_menu.context_menu_data, super::node_graph::utility_types::ContextMenuData::CreateNode { compatible_type: None }))
				{
					self.node_graph_handler.context_menu = None;
					responses.add(FrontendMessage::UpdateContextMenuInformation { context_menu_information: None });
				}
				// Go back up one level in the breadcrumb path if we're in a subgraph
				else if !self.breadcrumb_network_path.is_empty() {
					responses.add(DocumentMessage::ExitNestedNetwork { steps_back: 1 });
				}
				// Close the graph view overlay if it's open
				else {
					responses.add(DocumentMessage::GraphViewOverlay { open: false });
				}
			}
			DocumentMessage::ExitNestedNetwork { steps_back } => {
				for _ in 0..steps_back {
					self.breadcrumb_network_path.pop();
					self.selection_network_path.clone_from(&self.breadcrumb_network_path);
				}
				responses.add(NodeGraphMessage::UnloadWires);
				responses.add(NodeGraphMessage::SendGraph);
				responses.add(DocumentMessage::PTZUpdate);
				responses.add(NodeGraphMessage::UpdateNodeGraphWidth);
			}
			DocumentMessage::FlipSelectedLayers { flip_axis } => {
				let scale = match flip_axis {
					FlipAxis::X => DVec2::new(-1., 1.),
					FlipAxis::Y => DVec2::new(1., -1.),
				};
				if let Some([min, max]) = self.network_interface.selected_unlocked_layers_bounding_box_viewport() {
					let center = (max + min) / 2.;
					let bbox_trans = DAffine2::from_translation(-center);
					let mut added_transaction = false;
					for layer in self.network_interface.selected_nodes().selected_unlocked_layers(&self.network_interface) {
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
			DocumentMessage::RotateSelectedLayers { degrees } => {
				// Get the bounding box of selected layers in viewport space
				if let Some([min, max]) = self.network_interface.selected_unlocked_layers_bounding_box_viewport() {
					// Calculate the center of the bounding box to use as rotation pivot
					let center = (max + min) / 2.;
					// Transform that moves pivot point to origin
					let bbox_trans = DAffine2::from_translation(-center);

					let mut added_transaction = false;
					for layer in self.network_interface.selected_nodes().selected_unlocked_layers(&self.network_interface) {
						if !added_transaction {
							responses.add(DocumentMessage::AddTransaction);
							added_transaction = true;
						}

						responses.add(GraphOperationMessage::TransformChange {
							layer,
							transform: DAffine2::from_angle(degrees.to_radians()),
							transform_in: TransformIn::Scope { scope: bbox_trans },
							skip_rerender: false,
						});
					}
				}
			}
			DocumentMessage::GraphViewOverlay { open } => {
				let opened = !self.graph_view_overlay_open && open;
				self.graph_view_overlay_open = open;

				responses.add(FrontendMessage::UpdateGraphViewOverlay { open });
				responses.add(FrontendMessage::UpdateGraphFadeArtwork {
					percentage: self.graph_fade_artwork_percentage,
				});

				// Update the tilt menu bar buttons to be disabled when the graph is open
				responses.add(MenuBarMessage::SendLayout);

				responses.add(DocumentMessage::RenderRulers);
				responses.add(DocumentMessage::RenderScrollbars);
				if opened {
					responses.add(NodeGraphMessage::UnloadWires);
					responses.add(NodeGraphMessage::UpdateNodeGraphWidth);
				}
				if open {
					responses.add(ToolMessage::DeactivateTools);
					responses.add(OverlaysMessage::Draw); // Clear the overlays
					responses.add(NavigationMessage::CanvasTiltSet { angle_radians: 0. });
					responses.add(NodeGraphMessage::UpdateGraphBarRight);
					responses.add(NodeGraphMessage::SendGraph);
					responses.add(NodeGraphMessage::UpdateHints);
					responses.add(NodeGraphMessage::UpdateEdges);
				} else {
					responses.add(ToolMessage::ActivateTool { tool_type: *current_tool });
					responses.add(OverlaysMessage::Draw); // Redraw overlays when graph is closed
				}
			}
			DocumentMessage::GraphViewOverlayToggle => {
				responses.add(DocumentMessage::GraphViewOverlay { open: !self.graph_view_overlay_open });
			}
			DocumentMessage::GridOptions { options } => {
				self.snapping_state.grid = options;
				self.snapping_state.grid_snapping = true;
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			DocumentMessage::GridOverlays { context: mut overlay_context } => {
				if self.snapping_state.grid_snapping {
					grid_overlay(self, &mut overlay_context)
				}
			}
			DocumentMessage::GridVisibility { visible } => {
				self.snapping_state.grid_snapping = visible;
				responses.add(OverlaysMessage::Draw);
			}
			DocumentMessage::BlendSelectedLayers => {
				self.group_selected_layers(GroupFolderType::Blend, responses);
			}
			DocumentMessage::MorphSelectedLayers => {
				self.group_selected_layers(GroupFolderType::Morph, responses);
			}
			DocumentMessage::ExpandFillStrokeOnSelectedLayers => {
				// Snapshot must be taken before the mutations, so the actual work runs as a separate message
				// queued after AddTransaction (which prepends StartTransaction/CommitTransaction to the queue).
				// All mutations currently target the root document network, so guard against being invoked from inside a nested network.
				if !self.selection_network_path.is_empty() {
					log::error!("Expanding fill/stroke is only supported for the document network");
					return;
				}
				if self.network_interface.selected_nodes().selected_layers(self.metadata()).next().is_none() {
					return;
				}
				responses.add(DocumentMessage::AddTransaction);
				responses.add(DocumentMessage::ExpandFillStrokeOnSelectedLayersNoTransaction);
			}
			DocumentMessage::ExpandFillStrokeOnSelectedLayersNoTransaction => {
				// Mutates the network directly, so it must be queued to run after `AddTransaction` has snapshotted the document
				self.handle_expand_fill_stroke_on_selected_layers(responses);
			}
			DocumentMessage::GroupSelectedLayers { group_folder_type } => {
				self.group_selected_layers(group_folder_type, responses);
			}
			DocumentMessage::MoveSelectedLayersTo { parent, insert_index } => {
				self.move_selected_layers_to(parent, insert_index, responses);
			}
			DocumentMessage::ReorderPropertiesSection { node_id, insert_index } => {
				// The Properties panel shows draggable sections in two cases, disambiguated by the current selection:
				// a single selected layer (reorder within its node chain) or no selection (reorder the pinned nodes).
				let selected_nodes = self.network_interface.selected_nodes_in_nested_network(&self.selection_network_path);
				let Some(selected_nodes) = selected_nodes else { return };

				let (mut layers, mut nodes) = (Vec::new(), Vec::new());

				for selected in selected_nodes.selected_nodes() {
					if self.network_interface.is_layer(selected, &self.selection_network_path) {
						layers.push(*selected);
					} else {
						nodes.push(*selected);
					}
				}

				layers.sort();
				layers.dedup();

				if layers.len() == 1 {
					// Reorder a node within the selected layer's chain by rewiring the graph
					responses.add(DocumentMessage::AddTransaction);
					responses.add(NodeGraphMessage::ReorderChainNode { node_id, insert_index });
					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(NodeGraphMessage::SendGraph);
					responses.add(PropertiesPanelMessage::Refresh);
				} else if layers.is_empty() && nodes.is_empty() {
					// Reorder a pinned node, which is purely a Properties panel display order (no graph rerender needed)
					responses.add(DocumentMessage::AddTransaction);
					responses.add(NodeGraphMessage::ReorderPinnedNode { node_id, insert_index });
					responses.add(PropertiesPanelMessage::Refresh);
				}
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
				resize_opposite,
			} => {
				let resize = ipp.keyboard.key(resize);
				let resize_opposite = ipp.keyboard.key(resize_opposite);
				self.nudge_selected_layers(delta_x, delta_y, resize, resize_opposite, responses);
			}
			DocumentMessage::InsertImage {
				name,
				image,
				mouse,
				parent_and_insert_index,
				place_at_origin,
			} => {
				// All the image's pixels have been converted to 0..=1, linear, and premultiplied by `Color::from_rgba8_srgb`

				let layer_parent = self.new_layer_parent(true);
				let image_size = DVec2::new(image.width as f64, image.height as f64);

				let mut transform = if place_at_origin {
					// File-open flow: place at document origin without centering so `WrapContentInArtboard` can wrap it
					DAffine2::from_scale(image_size)
				} else {
					// Clipboard paste or drag-drop: center at cursor or viewport center.
					// Convert the document-space cursor to the parent's local coordinate space so that
					// an artboard at a non-zero position does not offset the placement.
					let parent_to_document = {
						let metadata = self.metadata();
						metadata.document_to_viewport.inverse() * metadata.transform_to_viewport(layer_parent)
					};
					let cursor_in_parent = parent_to_document.inverse() * self.document_transform_from_mouse(mouse, viewport);
					cursor_in_parent * DAffine2::from_scale_angle_translation(image_size, 0., image_size / -2.)
				};
				transform.translation = transform.translation.round();

				let layer_node_id = NodeId::new();
				let layer_id = LayerNodeIdentifier::new_unchecked(layer_node_id);

				// Emit the transaction as an explicit StartTransaction ... CommitTransaction pair bracketing the
				// mutations, rather than `AddTransaction`. `AddTransaction` prepends its Start/Commit as a tight
				// pair that the dispatcher drains before these sibling mutation messages, so the commit closes
				// before the layer is added and the storage staging sees an empty diff. Interleaving the pair
				// around the mutations makes them all siblings drained in order (Start, mutate, Commit), so the
				// commit observes the paste and stages it.
				responses.add(DocumentMessage::StartTransaction);

				let layer = graph_modification_utils::new_image_layer(image, layer_node_id, layer_parent, responses);

				if let Some(name) = name {
					responses.add(NodeGraphMessage::SetDisplayName {
						node_id: layer.to_node(),
						network_path: Vec::new(),
						alias: name,
						// Fold the name into the paste's single transaction so the whole paste is one undo step.
						skip_adding_history_step: true,
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

				responses.add(DocumentMessage::CommitTransaction);

				// Force chosen tool to be Select Tool after importing image.
				responses.add(ToolMessage::ActivateTool { tool_type: ToolType::Select });
			}
			DocumentMessage::InsertSvg {
				name,
				svg,
				mouse,
				parent_and_insert_index,
				place_at_origin,
			} => {
				let layer_parent = self.new_layer_parent(true);
				let transform = if place_at_origin {
					// File-open flow: place at document origin so `WrapContentInArtboard` can wrap it without extra Transform nodes
					DAffine2::IDENTITY
				} else {
					// Clipboard paste or drag-drop: center at cursor or viewport center.
					// Convert the document-space cursor to the parent's local coordinate space so that
					// an artboard at a non-zero position does not offset the placement.
					let parent_to_document = {
						let metadata = self.metadata();
						metadata.document_to_viewport.inverse() * metadata.transform_to_viewport(layer_parent)
					};
					parent_to_document.inverse() * self.document_transform_from_mouse(mouse, viewport)
				};

				let layer_node_id = NodeId::new();
				let layer_id = LayerNodeIdentifier::new_unchecked(layer_node_id);

				responses.add(DocumentMessage::AddTransaction);

				let layer = graph_modification_utils::new_svg_layer(svg, transform, !place_at_origin, layer_node_id, layer_parent, responses);

				if let Some(name) = name {
					responses.add(NodeGraphMessage::SetDisplayName {
						node_id: layer.to_node(),
						network_path: Vec::new(),
						alias: name,
						// Fold the name into the paste's single transaction so the whole paste is one undo step.
						skip_adding_history_step: true,
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
				responses.add(EventMessage::SelectionChanged);
			}
			DocumentMessage::RenameDocument { new_name } => {
				let new_name = new_name.trim().to_string();

				// No-op when the resolved name is unchangedL committing the rename field without edits (or with
				// only whitespace edits) shouldn't dissociate the document from its file on disk or mark it unsaved.
				if new_name == self.name {
					return;
				}

				self.name = new_name;

				self.path = None;
				self.set_save_state(false);
				self.set_auto_save_state(false);

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
				let document_to_viewport = self.navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), current_ptz);

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

				// Compute the selection bounding box as 4 viewport-space corners preserving orientation
				let selection_quad = if !self.graph_view_overlay_open {
					self.network_interface
						.selected_nodes()
						.0
						.iter()
						.filter(|node| self.network_interface.is_layer(node, &[]))
						.filter_map(|layer| self.metadata().bounding_box_document(LayerNodeIdentifier::new(*layer, &self.network_interface)))
						.reduce(Quad::combine_bounds)
						.map(|[min, max]| {
							let corners = [DVec2::new(min.x, min.y), DVec2::new(max.x, min.y), DVec2::new(max.x, max.y), DVec2::new(min.x, max.y)];
							corners.map(|c| document_to_viewport.transform_point2(c).into())
						})
				} else {
					None
				};

				responses.add(FrontendMessage::UpdateDocumentRulers {
					origin: ruler_origin.into(),
					spacing: ruler_spacing,
					interval: ruler_interval,
					visible: self.rulers_visible,
					tilt: if self.graph_view_overlay_open { 0. } else { current_ptz.tilt() },
					flip: !self.graph_view_overlay_open && current_ptz.flip,
					selection_quad,
				});
			}
			DocumentMessage::RenderScrollbars => {
				let document_transform_scale = self.navigation_handler.snapped_zoom(self.document_ptz.zoom()) / viewport.scale();

				let scale = 0.5 + ASYMPTOTIC_EFFECT + document_transform_scale * SCALE_EFFECT;

				let viewport_size = viewport.size().into_dvec2();
				let viewport_mid = viewport.center_in_viewport_space().into_dvec2();
				let [bounds1, bounds2] = if !self.graph_view_overlay_open {
					self.network_interface.document_bounds_viewport_space(true).unwrap_or([viewport_mid; 2])
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
			DocumentMessage::SaveDocument | DocumentMessage::SaveDocumentAs => {
				responses.add(PortfolioMessage::AutoSaveActiveDocument);

				let name = format!("{}.{}", self.name.clone(), GDD_FILE_EXTENSION);
				let path = if let DocumentMessage::SaveDocumentAs = message { None } else { self.path.clone() };
				if path.is_some() {
					responses.add(DocumentMessage::MarkAsSaved);
				}
				let folder = self.path.as_ref().and_then(|path| path.parent()).map(|parent| parent.to_path_buf());

				// The clone shares the live working-copy container (Arc), so its storage reads the state the
				// queued AutoSaveActiveDocument just committed.
				let mut document = self.clone();
				let resources_load_handle = resource_storage.resources();
				let export_load_handle = resource_storage.resources();

				responses.add(async move {
					document.resources.collect_garbage(document.used_resources(false).as_ref());
					document.resources.embed_resources(resources_load_handle).await;

					// The legacy .graphite blob, resources embedded inline so it stays self-contained as the .gdd recovery fallback.
					let legacy_document = document.serialize_document().into_bytes();

					// Export the working copy as a .gdd with the legacy blob embedded; fall back to the bare legacy blob if there is no working copy or the export fails.
					let content = match &document.storage {
						Some(storage) => storage
							.export_to_bytes(
								document_format::ExportFormat::Xz,
								document_format::ExportOptions::default(),
								export_load_handle.as_ref(),
								Some(&legacy_document),
							)
							.await
							.unwrap_or_else(|error| {
								log::error!("Save: building .gdd export failed, falling back to legacy .graphite: {error}");
								legacy_document
							}),
						None => {
							log::warn!("Save: working copy not mounted yet, saving legacy .graphite only");
							legacy_document
						}
					};

					Message::Frontend(FrontendMessage::TriggerSaveDocument {
						document_id,
						name,
						path,
						folder,
						content: content.into(),
					})
				});
			}
			DocumentMessage::SavedDocument { path } => {
				self.path = path;

				responses.add(PortfolioMessage::AutoSaveActiveDocument);
				responses.add(DocumentMessage::MarkAsSaved);

				// Update the name to match the file stem
				let document_name_from_path = self.path.as_ref().and_then(|path| {
					if path.extension().is_some_and(|e| e == FILE_EXTENSION) {
						path.file_stem().map(|n| n.to_string_lossy().to_string())
					} else {
						None
					}
				});
				if let Some(name) = document_name_from_path {
					self.name = name;

					responses.add(PortfolioMessage::UpdateOpenDocumentsList);
					responses.add(NodeGraphMessage::UpdateNewNodeGraph);
				}
			}
			DocumentMessage::MarkAsSaved => {
				self.set_save_state(true);
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			DocumentMessage::SelectParentLayer => {
				let selected_nodes = self.network_interface.selected_nodes();
				let selected_layers = selected_nodes.selected_layers(self.metadata());

				let mut parent_layers = HashSet::new();

				// Find the parent of each selected layer
				for layer in selected_layers {
					// Get this layer's parent
					let Some(parent) = layer.parent(self.metadata()) else { continue };

					// Either use the parent, or keep the same layer if it's already at the top level
					let to_insert = if parent == LayerNodeIdentifier::ROOT_PARENT { layer } else { parent };

					// Add the layer to the set of those which will become selected
					parent_layers.insert(to_insert.to_node());
				}

				// Select each parent layer
				if !parent_layers.is_empty() {
					let nodes = parent_layers.into_iter().collect();
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
					responses.add(EventMessage::SelectionChanged);
				}
			}
			DocumentMessage::SelectAllLayers => {
				if !self.overlays_visibility_settings.selection_outline() {
					return;
				}

				let metadata = self.metadata();
				let all_layers_except_artboards_invisible_and_locked = metadata.all_layers().filter(|&layer| !self.network_interface.is_artboard(&layer.to_node(), &[])).filter(|&layer| {
					self.network_interface.selected_nodes().layer_visible(layer, &self.network_interface) && !self.network_interface.selected_nodes().layer_locked(layer, &self.network_interface)
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
			DocumentMessage::SelectedLayersReverse => {
				self.selected_layers_reverse(responses);
			}
			DocumentMessage::SelectedLayersReorder { relative_index_offset } => {
				self.selected_layers_reorder(relative_index_offset, responses);
			}
			DocumentMessage::ClipLayer { id } => {
				let layer = LayerNodeIdentifier::new(id, &self.network_interface);

				responses.add(DocumentMessage::AddTransaction);
				responses.add(GraphOperationMessage::ClipModeToggle { layer });
			}
			DocumentMessage::SelectLayer { id, ctrl, shift } => {
				let layer = LayerNodeIdentifier::new(id, &self.network_interface);

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
						if self.network_interface.selected_nodes().selected_layers_contains(layer, self.metadata()) {
							responses.add_front(NodeGraphMessage::SelectedNodesRemove { nodes: vec![id] });
						} else {
							responses.add_front(NodeGraphMessage::SelectedNodesAdd { nodes: vec![id] });
						}
						responses.add(EventMessage::SelectionChanged);
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
			DocumentMessage::SetActivePanel { active_panel } => {
				match active_panel {
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
				responses.add(NodeGraphMessage::UpdateInSelectedNetwork);
			}
			DocumentMessage::SetBlendModeForSelectedLayers { blend_mode } => {
				for layer in self.network_interface.selected_nodes().selected_layers_except_artboards(&self.network_interface) {
					responses.add(GraphOperationMessage::BlendModeSet { layer, blend_mode });
				}
			}
			DocumentMessage::SetGraphFadeArtwork { percentage } => {
				self.graph_fade_artwork_percentage = percentage;
				responses.add(FrontendMessage::UpdateGraphFadeArtwork { percentage });
			}
			DocumentMessage::SetNodePinned { node_id, pinned } => {
				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::SetPinned { node_id, pinned });
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SelectedNodesUpdated);
				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::SetOpacityForSelectedLayers { opacity } => {
				let opacity = opacity.clamp(0., 1.);
				for layer in self.network_interface.selected_nodes().selected_layers_except_artboards(&self.network_interface) {
					responses.add(GraphOperationMessage::OpacitySet { layer, opacity });
				}
			}
			DocumentMessage::SetFillForSelectedLayers { fill } => {
				let fill = fill.clamp(0., 1.);
				for layer in self.network_interface.selected_nodes().selected_layers_except_artboards(&self.network_interface) {
					responses.add(GraphOperationMessage::BlendingFillSet { layer, fill });
				}
			}
			DocumentMessage::SetOverlaysVisibility { visible, overlays_type } => {
				let visibility_settings = &mut self.overlays_visibility_settings;
				let overlays_type = match overlays_type {
					Some(overlays_type) => overlays_type,
					None => {
						visibility_settings.all = visible;
						responses.add(EventMessage::ToolAbort);
						responses.add(OverlaysMessage::Draw);
						return;
					}
				};
				match overlays_type {
					OverlaysType::ArtboardName => visibility_settings.artboard_name = visible,
					OverlaysType::CompassRose => visibility_settings.compass_rose = visible,
					OverlaysType::QuickMeasurement => visibility_settings.quick_measurement = visible,
					OverlaysType::TransformMeasurement => visibility_settings.transform_measurement = visible,
					OverlaysType::TransformCage => visibility_settings.transform_cage = visible,
					OverlaysType::HoverOutline => visibility_settings.hover_outline = visible,
					OverlaysType::SelectionOutline => visibility_settings.selection_outline = visible,
					OverlaysType::LayerOriginCross => visibility_settings.layer_origin_cross = visible,
					OverlaysType::Pivot => visibility_settings.pivot = visible,
					OverlaysType::Origin => visibility_settings.origin = visible,
					OverlaysType::Path => visibility_settings.path = visible,
					OverlaysType::Anchors => {
						visibility_settings.anchors = visible;
						responses.add(PortfolioMessage::UpdateDocumentWidgets);
					}
					OverlaysType::Handles => visibility_settings.handles = visible,
				}

				responses.add(EventMessage::ToolAbort);
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
				responses.add(DocumentMessage::EndTransaction);
			}
			DocumentMessage::SetRenderMode { render_mode } => {
				self.render_mode = render_mode;
				responses.add_front(NodeGraphMessage::RunDocumentGraph);
			}
			DocumentMessage::AddTransaction => {
				// Reverse order since they are added to the front
				responses.add_front(DocumentMessage::CommitTransaction);
				responses.add_front(DocumentMessage::StartTransaction);
			}
			// Note: A transaction should never be started in a scope that mutates the network interface, since it will only be run after that scope ends.
			DocumentMessage::StartTransaction => {
				// A new undo step is beginning, so the previous interaction's staged hot ops are complete:
				// retire them into one durable Gdd interaction. This aligns one Gdd interaction to one legacy
				// undo step (a tool drag fires several `CommitTransaction`s but one `StartTransaction`).
				self.retire_storage_interaction();

				self.network_interface.start_transaction();
				let network_interface_clone = self.network_interface.clone();
				self.document_undo_history.push_back(network_interface_clone);
				if self.document_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
					self.document_undo_history.pop_front();
				}
				// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			// Commits the transaction if the network was mutated since the transaction started, otherwise it cancels the transaction
			DocumentMessage::EndTransaction => match self.network_interface.transaction_status() {
				TransactionStatus::Started => {
					responses.add_front(DocumentMessage::CancelTransaction);
				}
				TransactionStatus::Modified => {
					responses.add_front(DocumentMessage::CommitTransaction);
				}
				TransactionStatus::Finished => {}
			},
			DocumentMessage::CancelTransaction => {
				self.network_interface.finish_transaction();
				self.document_undo_history.pop_back();
			}
			DocumentMessage::CommitTransaction => {
				if self.network_interface.transaction_status() == TransactionStatus::Finished {
					return;
				}
				self.network_interface.finish_transaction();
				self.document_redo_history.clear();

				// Stage this commit into the `Gdd` working copy. Retirement into a durable interaction happens
				// at the undo-step boundary (`StartTransaction`), so several commits in one user action
				// coalesce into one undo unit. Legacy snapshot undo stays authoritative.
				if let Some(byte_store) = resource_storage.storage() {
					self.commit_storage_snapshot(byte_store);
				}

				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			DocumentMessage::AbortTransaction => match self.network_interface.transaction_status() {
				TransactionStatus::Started => {
					responses.add_front(DocumentMessage::CancelTransaction);
				}
				TransactionStatus::Modified => {
					responses.add(DocumentMessage::RepeatedAbortTransaction { undo_count: 1 });
				}
				TransactionStatus::Finished => {}
			},
			DocumentMessage::RepeatedAbortTransaction { undo_count } => {
				if self.network_interface.transaction_status() == TransactionStatus::Finished {
					return;
				}

				for _ in 0..undo_count {
					self.undo(viewport, responses);
				}

				self.network_interface.finish_transaction();
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			DocumentMessage::ToggleLayerExpansion { tree_path, recursive } => {
				let is_collapsed = self.collapsed.0.contains(&tree_path);

				if is_collapsed {
					if recursive {
						// Remove this path and all descendant paths (paths that start with this one)
						self.collapsed.0.retain(|path| !path.starts_with(&tree_path));
					} else {
						self.collapsed.0.retain(|path| *path != tree_path);
					}
				} else {
					if recursive {
						// Collapse all expanded descendant occurrences by collecting their tree paths from the structure tree
						let descendant_paths = self.collect_descendant_tree_paths(&tree_path);
						for path in descendant_paths {
							if !self.collapsed.0.contains(&path) {
								self.collapsed.0.push(path);
							}
						}
					}
					self.collapsed.0.push(tree_path);
				}

				responses.add(NodeGraphMessage::SendGraph);
			}
			DocumentMessage::ToggleNodePropertiesSectionExpanded { node_id } => {
				if let Some(index) = self.properties_panel_collapsed_sections.iter().position(|id| *id == node_id) {
					self.properties_panel_collapsed_sections.remove(index);
				} else {
					self.properties_panel_collapsed_sections.push(node_id);
				}
				responses.add(PropertiesPanelMessage::Refresh);
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
				self.overlays_visibility_settings.all = !self.overlays_visibility_settings.all();
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			DocumentMessage::ToggleSnapping => {
				self.snapping_state.snapping_enabled = !self.snapping_state.snapping_enabled;
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			DocumentMessage::UpdateUpstreamTransforms {
				upstream_footprints,
				local_transforms,
				first_element_source_id,
			} => {
				self.network_interface.update_transforms(upstream_footprints, local_transforms);
				self.network_interface.update_first_element_source_id(first_element_source_id);
			}
			DocumentMessage::UpdateClickTargets { click_targets } => {
				// TODO: Allow non layer nodes to have click targets
				let layer_click_targets = click_targets
					.into_iter()
					.filter(|(node_id, _)|
						// Ensure that the layer is in the document network to prevent logging an error
						self.network_interface.document_network().nodes.contains_key(node_id))
					.filter_map(|(node_id, click_targets)| {
						self.network_interface.is_layer(&node_id, &[]).then(|| {
							let layer = LayerNodeIdentifier::new(node_id, &self.network_interface);
							(layer, click_targets)
						})
					})
					.collect();
				self.network_interface.update_click_targets(layer_click_targets);
			}
			DocumentMessage::UpdateOutlines { outlines } => {
				let layer_outlines = outlines
					.into_iter()
					.filter(|(node_id, _)| self.network_interface.document_network().nodes.contains_key(node_id))
					.filter_map(|(node_id, outlines)| {
						self.network_interface.is_layer(&node_id, &[]).then(|| {
							let layer = LayerNodeIdentifier::new(node_id, &self.network_interface);
							(layer, outlines)
						})
					})
					.collect();
				self.network_interface.update_outlines(layer_outlines);
			}
			DocumentMessage::UpdateTextFrames { text_frames } => {
				let layer_text_frames = text_frames
					.into_iter()
					.filter(|(node_id, _)| self.network_interface.document_network().nodes.contains_key(node_id))
					.filter(|&(node_id, _)| self.network_interface.is_layer(&node_id, &[]))
					.map(|(node_id, frame)| {
						let layer = LayerNodeIdentifier::new(node_id, &self.network_interface);
						(layer, frame)
					})
					.collect();
				self.network_interface.update_text_frames(layer_text_frames);
			}
			DocumentMessage::UpdateClipTargets { clip_targets } => {
				self.network_interface.update_clip_targets(clip_targets);
			}
			DocumentMessage::UpdateVectorData { vector_data } => {
				// Convert NodeId keys to LayerNodeIdentifier keys, filtering to only layers
				let layer_vector_data = vector_data
					.into_iter()
					.filter(|(node_id, _)| self.network_interface.document_network().nodes.contains_key(node_id))
					.filter_map(|(node_id, vector)| {
						self.network_interface.is_layer(&node_id, &[]).then(|| {
							let layer = LayerNodeIdentifier::new(node_id, &self.network_interface);
							(layer, vector)
						})
					})
					.collect();
				self.network_interface.update_vector_data(layer_vector_data);
			}
			DocumentMessage::UpdateFillAttributes { fill_attributes } => {
				// Convert NodeId keys to LayerNodeIdentifier keys, filtering to only layers
				let layer_fill_attributes = fill_attributes
					.into_iter()
					.filter(|(node_id, _)| self.network_interface.document_network().nodes.contains_key(node_id))
					.filter_map(|(node_id, attrs)| {
						self.network_interface.is_layer(&node_id, &[]).then(|| {
							let layer = LayerNodeIdentifier::new(node_id, &self.network_interface);
							(layer, attrs)
						})
					})
					.collect();
				self.network_interface.update_fill_attributes(layer_fill_attributes);
			}
			DocumentMessage::UpdateStrokeAttributes { stroke_attributes } => {
				// Convert NodeId keys to LayerNodeIdentifier keys, filtering to only layers
				let layer_stroke_attributes = stroke_attributes
					.into_iter()
					.filter(|(node_id, _)| self.network_interface.document_network().nodes.contains_key(node_id))
					.filter_map(|(node_id, attrs)| {
						self.network_interface.is_layer(&node_id, &[]).then(|| {
							let layer = LayerNodeIdentifier::new(node_id, &self.network_interface);
							(layer, attrs)
						})
					})
					.collect();
				self.network_interface.update_stroke_attributes(layer_stroke_attributes);
			}
			DocumentMessage::Undo => {
				if self.network_interface.transaction_status() != TransactionStatus::Finished {
					return;
				}
				responses.add(ToolMessage::PreUndo);
				responses.add(DocumentMessage::DocumentHistoryBackward);
				responses.add(OverlaysMessage::Draw);
				responses.add(ToolMessage::Undo);
				responses.add(EventMessage::SelectionChanged);
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

					let metadata = self.network_interface.document_metadata();
					let layer_local_transform = metadata.transform_to_viewport(child);
					let undo_parent_transform = if parent == LayerNodeIdentifier::ROOT_PARENT {
						// This is functionally the same as transform_to_viewport for the root, however to_node cannot run on the root in debug mode.
						metadata.document_to_viewport.inverse()
					} else {
						metadata.transform_to_viewport(parent).inverse()
					};
					let transform = undo_parent_transform * layer_local_transform;
					responses.add(GraphOperationMessage::TransformSet {
						layer: child,
						transform,
						transform_in: TransformIn::Local,
						skip_rerender: false,
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
					let transform = self.navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), &self.document_ptz);
					self.network_interface.set_document_to_viewport_transform(transform);
					// Ensure selection box is kept in sync with the pointer when the PTZ changes
					responses.add(SelectToolMessage::PointerMove {
						modifier_keys: SelectToolPointerKeys {
							axis_align: Key::Shift,
							snap_angle: Key::Shift,
							center: Key::Alt,
							duplicate: Key::Alt,
						},
					});
					responses.add(NodeGraphMessage::RunDocumentGraph);
				} else {
					let Some(network_metadata) = self.network_interface.network_metadata(&self.breadcrumb_network_path) else {
						return;
					};

					let transform = self
						.navigation_handler
						.calculate_offset_transform(viewport.center_in_viewport_space().into(), &network_metadata.persistent_metadata.navigation_metadata.node_graph_ptz);
					self.network_interface.set_transform(transform, &self.breadcrumb_network_path);

					responses.add(DocumentMessage::RenderRulers);
					responses.add(DocumentMessage::RenderScrollbars);
					responses.add(NodeGraphMessage::UpdateEdges);
					responses.add(NodeGraphMessage::UpdateBoxSelection);
					responses.add(NodeGraphMessage::UpdateImportsExports);

					responses.add(FrontendMessage::UpdateNodeGraphTransform {
						translation: transform.translation.into(),
						scale: transform.matrix2.x_axis.x,
					})
				}
			}
			DocumentMessage::SelectionStepBack => {
				self.network_interface.selection_step_back(&self.selection_network_path);
				responses.add(EventMessage::SelectionChanged);
			}
			DocumentMessage::SelectionStepForward => {
				self.network_interface.selection_step_forward(&self.selection_network_path);
				responses.add(EventMessage::SelectionChanged);
			}
			DocumentMessage::WrapContentInArtboard {
				place_artboard_at_origin,
				artboard_canvas,
			} => {
				// Get bounding box of all layers (always needed to confirm there is content)
				let bounds = self.network_interface.document_bounds_document_space(false);
				let Some(bounds) = bounds else { return };

				// When artboard_canvas is provided (SVG file-open flow), use the declared canvas origin and dimensions;
				// no content-shift Transform node needed since the SVG was already placed at its natural coordinates.
				let (artboard_location, artboard_dimensions, content_shift) = if let Some((origin, dimensions)) = artboard_canvas {
					(origin.as_dvec2(), dimensions.as_dvec2(), DVec2::ZERO)
				} else {
					// No declared canvas (image or clipboard paste): derive location and dimensions from the content bounding box.
					let location = if place_artboard_at_origin { DVec2::ZERO } else { bounds[0].round() };
					(location, (bounds[1] - bounds[0]).round(), -bounds[0].round())
				};

				// Create an artboard and set its dimensions to the bounding box size and location
				let node_id = NodeId::new();
				let node_layer_id = LayerNodeIdentifier::new_unchecked(node_id);
				let new_artboard_node = document_node_definitions::resolve_network_node_type("Artboard")
					.expect("Failed to create artboard node")
					.default_node_template();
				responses.add(NodeGraphMessage::InsertNode {
					node_id,
					node_template: Box::new(new_artboard_node),
				});
				let needs_content_transform = !content_shift.abs_diff_eq(DVec2::ZERO, 1e-6);
				// With a content Transform node: shift by the layer indent plus the node width. Without: use just the layer indent.
				responses.add(NodeGraphMessage::ShiftNodePosition {
					node_id,
					x: if needs_content_transform { LAYER_INDENT_OFFSET + NODE_CHAIN_WIDTH } else { LAYER_INDENT_OFFSET },
					y: -3,
				});
				responses.add(GraphOperationMessage::ResizeArtboard {
					layer: LayerNodeIdentifier::new_unchecked(node_id),
					location: artboard_location,
					dimensions: artboard_dimensions,
				});

				// Connect the current output data to the artboard's input data, and the artboard's output to the document output
				responses.add(NodeGraphMessage::InsertNodeBetween {
					node_id,
					input_connector: network_interface::InputConnector::Export(0),
					insert_node_input_index: 1,
				});

				// Shift the content to align its top-left to the artboard's origin (no-op when content is already at origin)
				responses.add(GraphOperationMessage::TransformChange {
					layer: node_layer_id,
					transform: DAffine2::from_translation(content_shift),
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
					if self.document_ptz.flip {
						responses.add(NavigationMessage::CanvasFlip);
					}
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
			SelectParentLayer,
			SelectionStepForward,
			SelectionStepBack,
			ZoomCanvasTo100Percent,
			ZoomCanvasTo200Percent,
			ZoomCanvasToFitAll,
		);

		// Additional actions available on desktop
		#[cfg(not(target_family = "wasm"))]
		common.extend(actions!(DocumentMessageDiscriminant::SaveDocumentAs));

		// Additional actions if there are any selected layers
		if self.network_interface.selected_nodes().selected_layers(self.metadata()).next().is_some() {
			let mut select = actions!(DocumentMessageDiscriminant;
				DeleteSelectedLayers,
				DuplicateSelectedLayers,
				GroupSelectedLayers,
				BlendSelectedLayers,
				MorphSelectedLayers,
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
	/// Build a document handler from a `.gdd` working copy: the runtime `interface` rebuilt from the
	/// stored registry plus the mounted `Gdd`. The caller converts the registry to `interface`; this
	/// restores the document-level `ui::doc::*` settings and resource registry and takes ownership of
	/// the working copy. Post-load fixups (`load_structure`, validation, graph run) are the loader's job.
	pub fn from_storage(interface: NodeNetworkInterface, storage: document_format::GddV1, name: String, path: Option<std::path::PathBuf>) -> Self {
		let mut document = Self {
			network_interface: interface,
			name,
			path,
			..Default::default()
		};

		document.apply_stored_document_settings(&storage.view_settings().clone());
		match storage.registry().to_resource_registry() {
			Ok(resource_registry) => document.resources.registry = resource_registry,
			Err(error) => log::error!("Opening .gdd: failed to rebuild resource registry: {error}"),
		}
		document.storage = Some(storage);

		document
	}

	/// Post-load fixups for a document built from storage: per-node input/output metadata validation
	/// and the layer-structure rebuild. Mirrors the essential part of the legacy load path; the
	/// old-file layer-stacking realignment is skipped (a current-format `.gdd` doesn't need it).
	pub fn finalize_storage_load(&mut self) {
		for (node_id, node, path) in self.network_interface.document_network().clone().recursive_nodes() {
			self.network_interface.validate_input_metadata(node_id, node, &path);
			self.network_interface.validate_output_names(node_id, node, &path);
		}

		self.network_interface.load_structure();
	}

	/// Translates a viewport mouse position to a document-space transform, or uses the viewport center if no mouse position is given.
	fn document_transform_from_mouse(&self, mouse: Option<(f64, f64)>, viewport: &ViewportMessageHandler) -> DAffine2 {
		let viewport_pos: DVec2 = mouse.map_or_else(|| viewport.center_in_viewport_space().into_dvec2() + viewport.offset().into_dvec2(), |pos| pos.into());
		let document_to_viewport = self.navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), &self.document_ptz);
		DAffine2::from_translation(document_to_viewport.inverse().transform_point2(viewport_pos - viewport.offset().into_dvec2()))
	}

	/// Runs an intersection test with all layers and a viewport space quad
	pub fn intersect_quad<'a>(&'a self, viewport_quad: graphene_std::renderer::Quad, viewport: &ViewportMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + use<'a> {
		let document_to_viewport = self.navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), &self.document_ptz);
		let document_quad = document_to_viewport.inverse() * viewport_quad;

		ClickXRayIter::new(&self.network_interface, XRayTarget::Quad(document_quad))
	}

	/// Runs an intersection test with all layers and a viewport space quad; ignoring artboards
	pub fn intersect_quad_no_artboards<'a>(&'a self, viewport_quad: graphene_std::renderer::Quad, viewport: &ViewportMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + use<'a> {
		self.intersect_quad(viewport_quad, viewport).filter(|layer| !self.network_interface.is_artboard(&layer.to_node(), &[]))
	}

	/// Runs an intersection test with all layers and a viewport space subpath
	pub fn intersect_polygon<'a>(&'a self, mut viewport_polygon: Subpath<PointId>, viewport: &ViewportMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + use<'a> {
		let document_to_viewport = self.navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), &self.document_ptz);
		viewport_polygon.apply_transform(document_to_viewport.inverse());

		ClickXRayIter::new(&self.network_interface, XRayTarget::Polygon(viewport_polygon))
	}

	/// Runs an intersection test with all layers and a viewport space subpath; ignoring artboards
	pub fn intersect_polygon_no_artboards<'a>(&'a self, viewport_polygon: Subpath<PointId>, viewport: &ViewportMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + use<'a> {
		self.intersect_polygon(viewport_polygon, viewport)
			.filter(|layer| !self.network_interface.is_artboard(&layer.to_node(), &[]))
	}

	pub fn is_layer_fully_inside(&self, layer: &LayerNodeIdentifier, quad: graphene_std::renderer::Quad) -> bool {
		// Get the bounding box of the layer in document space
		let Some(bounding_box) = self.metadata().bounding_box_viewport(*layer) else { return false };

		// Check if the bounding box is fully within the selection quad
		let [top_left, bottom_right] = bounding_box;

		let quad_bbox = quad.bounding_box();

		let quad_left = quad_bbox[0].x;
		let quad_right = quad_bbox[1].x;
		let quad_top = quad_bbox[0].y.max(quad_bbox[1].y); // Correct top
		let quad_bottom = quad_bbox[0].y.min(quad_bbox[1].y); // Correct bottom

		// Extract layer's bounding box coordinates
		let layer_left = top_left.x;
		let layer_right = bottom_right.x;
		let layer_top = bottom_right.y;
		let layer_bottom = top_left.y;

		layer_left >= quad_left && layer_right <= quad_right && layer_top <= quad_top && layer_bottom >= quad_bottom
	}

	pub fn is_layer_fully_inside_polygon(&self, layer: &LayerNodeIdentifier, viewport: &ViewportMessageHandler, mut viewport_polygon: Subpath<PointId>) -> bool {
		let document_to_viewport = self.navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), &self.document_ptz);
		viewport_polygon.apply_transform(document_to_viewport.inverse());

		let layer_click_targets = self.network_interface.document_metadata().click_targets(*layer);
		let layer_transform = self.network_interface.document_metadata().transform_to_document(*layer);

		layer_click_targets.is_some_and(|targets| {
			targets.iter().all(|target| match target.target_type() {
				ClickTargetType::Subpath(subpath) => {
					let mut subpath = subpath.clone();
					subpath.apply_transform(layer_transform);
					subpath.is_inside_subpath(&viewport_polygon, None, None)
				}
				ClickTargetType::CompoundPath(subpaths) => subpaths.iter().all(|subpath| {
					let mut subpath = subpath.clone();
					subpath.apply_transform(layer_transform);
					subpath.is_inside_subpath(&viewport_polygon, None, None)
				}),
				ClickTargetType::FreePoint(point) => {
					let mut point = *point;
					point.apply_transform(layer_transform);
					viewport_polygon.contains_point(point.position)
				}
			})
		})
	}

	/// Find all of the layers that were clicked on from a viewport space location
	pub fn click_xray(&self, ipp: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + use<'_> {
		let document_to_viewport = self.navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), &self.document_ptz);
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
	pub fn click_list<'a>(&'a self, ipp: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + use<'a> {
		self.click_xray(ipp, viewport)
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

	/// Find layers (including artboards) under the location in viewport space that was clicked, listed by their depth in the layer tree hierarchy.
	pub fn click_list_with_artboards<'a>(&'a self, ipp: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + use<'a> {
		self.click_xray(ipp, viewport)
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

	pub fn click_list_no_parents<'a>(&'a self, ipp: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) -> impl Iterator<Item = LayerNodeIdentifier> + use<'a> {
		self.click_xray(ipp, viewport)
			.filter(move |&layer| !self.network_interface.is_artboard(&layer.to_node(), &[]) && !layer.has_children(self.network_interface.document_metadata()))
	}

	/// Find the deepest layer that has been clicked on from a location in viewport space.
	pub fn click(&self, ipp: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) -> Option<LayerNodeIdentifier> {
		self.click_list(ipp, viewport).last()
	}

	pub fn click_based_on_position(&self, mouse_snapped_positon: DVec2) -> Option<LayerNodeIdentifier> {
		ClickXRayIter::new(&self.network_interface, XRayTarget::Point(mouse_snapped_positon))
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
			.last()
	}

	pub fn document_network(&self) -> &NodeNetwork {
		self.network_interface.document_network()
	}

	pub fn metadata(&self) -> &DocumentMetadata {
		self.network_interface.document_metadata()
	}

	/// Path to the subnetwork that the user's selection is currently scoped to.
	/// Empty when the selection lives in the root document network.
	pub fn selection_network_path(&self) -> &[NodeId] {
		&self.selection_network_path
	}

	/// Retire the pending staged hot ops (the current interaction) into durable Gdd history as one undo
	/// unit. Called at each undo-step boundary (a new `StartTransaction`) and before undo/redo, so the
	/// per-`CommitTransaction` staging coalesces into one retired interaction aligned with the legacy step.
	pub(crate) fn retire_storage_interaction(&mut self) {
		let Some(storage) = self.storage.as_mut() else { return };
		if let Err(error) = storage.retire_pending_interaction() {
			log::error!("Storage interaction retirement failed: {error}");
		}
	}

	/// Stage the runtime network into the document's `Gdd` working copy at each `CommitTransaction`.
	/// No-op while the working copy is still unmounted (its container is built asynchronously on
	/// document open); the mount picks up the current runtime state once it attaches. The staged hot ops
	/// are retired into durable history by [`retire_storage_interaction`](Self::retire_storage_interaction) at
	/// undo-step boundaries. Proto-node declaration bytes are persisted into `byte_store` (the app-global
	/// resource cache).
	pub fn commit_storage_snapshot(&mut self, byte_store: &dyn graph_craft::application_io::resource::ResourceStorage) {
		use crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::{DocumentSettings, StorageMetadataView};

		if self.storage.is_none() {
			return;
		}

		use crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::collect_network_view_settings;

		let network = self.network_interface.document_network().clone();
		let view = StorageMetadataView::new(&self.network_interface);

		// Per-network view state (node-graph nav + previewing) is per-peer too, so collect it (keyed by the
		// stable `NetworkId` the storage layer resolves) for `session.json`. Computed before the mutable
		// `storage` borrow below.
		let network_view_settings = self
			.storage
			.as_ref()
			.and_then(|storage| storage.network_ids(&network, &view).ok())
			.map(|network_ids| collect_network_view_settings(&self.network_interface, &network_ids));

		// Per-peer view settings (PTZ, rulers, ...) persist in `session.json`, not the registry, so they
		// stay out of the CRDT/history (not undoable, not synced, may differ per viewer).
		let view_settings = DocumentSettings {
			document_ptz: &self.document_ptz,
			render_mode: &self.render_mode,
			overlays_visibility: &self.overlays_visibility_settings,
			rulers_visible: self.rulers_visible,
			snapping_state: &self.snapping_state,
			collapsed: &self.collapsed,
		}
		.to_view_map();

		// Serialize the legacy document from the same `self` state captured above, so the embedded
		// blob and the registry snapshot describe one consistent document (no interleaved edit).
		let legacy_document = self.serialize_document();

		// `view` borrows disjoint `self` fields, so the mutable `storage` borrow is independent.
		let storage = self.storage.as_mut().expect("checked present above");
		// Stage into the working copy without retiring: a tool drag fires several `CommitTransaction`s
		// but is one legacy undo step, so the deltas accumulate as hot ops and coalesce into one retired
		// interaction at the next undo-step boundary (`retire_pending_interaction`).
		if let Err(error) = storage.stage_runtime_snapshot(&network, &view, &self.resources.registry, byte_store) {
			log::error!("Storage snapshot staging failed: {error}");
			return;
		}

		if let Err(error) = storage.set_view_settings(view_settings) {
			log::error!("Persisting view settings failed: {error}");
		}

		if let Some(network_view_settings) = network_view_settings
			&& let Err(error) = storage.set_network_view_settings(network_view_settings)
		{
			log::error!("Persisting per-network view settings failed: {error}");
		}

		// Dual-write soak: embed the legacy `.graphite` bytes inside the `.gdd` working copy so the new
		// format can be validated against (and recovered from) the old one on open.
		if let Err(error) = storage.store_legacy_document(legacy_document.as_bytes()) {
			log::error!("Embedding legacy document into working copy failed: {error}");
		}

		#[cfg(debug_assertions)]
		self.verify_storage_round_trip(&network, &view);
	}

	/// Restore the per-peer view settings persisted in `session.json` (the `ui::doc::*`-keyed
	/// `view_settings` map) into the runtime handler fields. Each setting is applied only if present and
	/// decodable; missing or undecodable keys leave the current field untouched. Inverse of the view-
	/// settings half of `commit_storage_snapshot`; used when the `.gdd` is the load source.
	pub fn apply_stored_document_settings(&mut self, view_settings: &std::collections::BTreeMap<String, serde_json::Value>) {
		use graph_storage::attr::session::doc;

		fn decode<T: serde::de::DeserializeOwned>(view_settings: &std::collections::BTreeMap<String, serde_json::Value>, key: &str) -> Option<T> {
			view_settings.get(key).and_then(|value| serde_json::from_value(value.clone()).ok())
		}

		if let Some(value) = decode(view_settings, doc::PTZ) {
			self.document_ptz = value;
		}
		if let Some(value) = decode(view_settings, doc::RENDER_MODE) {
			self.render_mode = value;
		}
		if let Some(value) = decode(view_settings, doc::OVERLAYS) {
			self.overlays_visibility_settings = value;
		}
		if let Some(value) = decode(view_settings, doc::RULERS_VISIBLE) {
			self.rulers_visible = value;
		}
		if let Some(value) = decode(view_settings, doc::SNAPPING) {
			self.snapping_state = value;
		}
		if let Some(value) = decode(view_settings, doc::COLLAPSED) {
			self.collapsed = value;
		}
	}

	/// Debug-only: stored registry should equal a fresh `from_runtime`, and a `to_runtime` of the
	/// stored registry should equal the original network. Panics on drift so the dual-write soak
	/// fails loud in dev (and in tests); release builds skip this entirely and autosave never crashes.
	#[cfg(debug_assertions)]
	fn verify_storage_round_trip(
		&self,
		network: &graph_craft::document::NodeNetwork,
		view: &crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::StorageMetadataView,
	) {
		let Some(storage) = &self.storage else { return };
		let peer = storage.session().peer();

		let conversion = graph_storage::Registry::convert_from_runtime(network, view, &self.resources.registry, peer).expect("storage round-trip: from_runtime failed");
		let target = &conversion.registry;
		let declarations = conversion.declarations().expect("storage round-trip: declaration rebuild failed");

		let stored = storage.registry();
		assert!(stored.value_equal(target), "storage round-trip: registry value drift after commit\n{}", diff_registries(stored, target));
		assert!(stored.order_consistent(target), "storage round-trip: timestamp order inconsistent between stored and target");

		let (round_tripped, _entries) = stored.to_runtime_with_metadata(&declarations).expect("storage round-trip: to_runtime failed");
		assert!(
			&round_tripped == network,
			"storage round-trip: network drift after to_runtime\n{}",
			diff_networks(network, &round_tripped)
		);
	}

	/// Move the `Gdd` undo/redo cursor (the authoritative path) and spawn the async future that rebuilds the
	/// interface from the cursor and swaps it in. Synchronous: flushes the open interaction, moves the cursor
	/// (persisting `session.json`), then clones the post-move `Gdd` and queues the rebuild. `had_oracle`
	/// records whether the legacy snapshot already applied, so the completion can compare; it travels with
	/// the spawned message. Returns whether the cursor moved (so callers know a rebuild is pending).
	fn drive_storage_undo_redo(&mut self, document_id: DocumentId, resource_storage: &ResourceStorageMessageHandler, had_oracle: bool, undo: bool, responses: &mut VecDeque<Message>) -> bool {
		// Flush any open interaction into retired history first: undo/redo operate on the retired chain, so
		// the most recent edit must be a committed interaction before the cursor moves.
		self.retire_storage_interaction();

		let Some(storage) = self.storage.as_mut() else { return false };

		let moved = if undo {
			if !storage.can_undo() {
				return false;
			}
			storage.undo().map(|_| ())
		} else {
			if !storage.can_redo() {
				return false;
			}
			storage.redo().map(|_| ())
		};
		if let Err(error) = moved {
			log::error!("Storage undo/redo cursor move failed: {error}");
			return false;
		}

		let Some(store_handle) = resource_storage.store_handle() else {
			log::error!("Storage undo/redo: resource storage not initialized; cannot rebuild interface");
			return false;
		};

		// Clone the post-move `Gdd` (a `Session` snapshot at the new cursor; the container is `Arc`-shared)
		// so the `'static` rebuild future reads the rewound state while the live document keeps its cursor.
		let gdd = storage.clone();
		responses.add(crate::messages::portfolio::portfolio_message_handler::rebuild_gdd_cursor_future(
			gdd,
			store_handle,
			document_id,
			had_oracle,
		));
		true
	}

	/// Swap in the interface rebuilt from the `Gdd` cursor (authoritative), preserving transient state the
	/// registry doesn't model (navigation metadata, resolved types) by copying it from the current live
	/// interface. When `had_oracle` is set (the legacy snapshot applied synchronously), debug builds compare
	/// the rebuilt network against the legacy-restored one and log drift before overwriting. Always
	/// overwrites: the `Gdd` cursor is the source of truth, including the across-reopen case where no legacy
	/// snapshot exists.
	pub(crate) fn apply_gdd_cursor_rebuild(&mut self, mut rebuilt: NodeNetworkInterface, had_oracle: bool, responses: &mut VecDeque<Message>) {
		// The registry models document content only, so the rebuilt interface has default view and selection
		// state. Carry the user's current view, selection, and resolved types from the live interface so undo
		// changes the document without moving the camera, clearing the selection, or jumping the node graph.
		rebuilt.copy_all_transient_view_state(&self.network_interface);
		std::mem::swap(&mut rebuilt.resolved_types, &mut self.network_interface.resolved_types);
		rebuilt.load_structure();

		#[cfg(debug_assertions)]
		if had_oracle {
			self.compare_rebuild_against_legacy(&rebuilt);
		}

		self.network_interface = rebuilt;

		// The live `Gdd` cursor's registry should match a fresh `from_runtime` of the now-swapped interface.
		#[cfg(debug_assertions)]
		self.verify_storage_cursor_matches_runtime();

		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		responses.add(NodeGraphMessage::SelectedNodesUpdated);
		responses.add(NodeGraphMessage::ForceRunDocumentGraph);
		responses.add(NodeGraphMessage::UnloadWires);
		responses.add(NodeGraphMessage::SendWires);
	}

	/// Debug-only: the rebuilt network should equal the legacy-restored one. Logs drift (does not panic) so
	/// cursor/conversion bugs surface in dev without crashing the live editor while the path hardens.
	#[cfg(debug_assertions)]
	fn compare_rebuild_against_legacy(&self, rebuilt: &NodeNetworkInterface) {
		let legacy = self.network_interface.document_network();
		let candidate = rebuilt.document_network();
		if candidate != legacy {
			log::error!("undo/redo rebuild diverged from the legacy snapshot\n{}", diff_networks(legacy, candidate));
			#[cfg(test)]
			panic!()
		}
	}

	/// Debug-only: after a `Gdd` cursor move, the cursor's registry should equal a fresh `from_runtime`
	/// of the current (legacy-restored) interface. Logs drift (see the call site for why it does not
	/// panic) so cursor bugs surface in dev without crashing the editor.
	#[cfg(debug_assertions)]
	fn verify_storage_cursor_matches_runtime(&self) {
		use crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::StorageMetadataView;

		let Some(storage) = &self.storage else { return };
		let peer = storage.session().peer();

		let network = self.network_interface.document_network().clone();
		let view = StorageMetadataView::new(&self.network_interface);
		let Ok(mut conversion) = graph_storage::Registry::convert_from_runtime(&network, &view, &self.resources.registry, peer) else {
			log::error!("undo/redo shadow: from_runtime failed");
			return;
		};

		let stored = storage.registry();

		// The runtime keeps resources alive while they're referenced by undo/redo history (so legacy redo
		// can restore them), but the `Gdd` cursor reverts the interaction's `AddResource`. So a resource the
		// runtime carries which the cursor dropped is expected, not drift, as long as it's history-only
		// (not referenced by the current network). Drop those from the conversion before comparing.
		let current_resources: std::collections::HashSet<_> = self.used_resources(false).iter().copied().collect();
		conversion.registry.resources.retain(|id, _| stored.resources.contains_key(id) || current_resources.contains(id));

		if !stored.value_equal(&conversion.registry) {
			// Logged, not panicked: any remaining drift is a real cursor or conversion bug to triage, but
			// the shadow must not crash the live editor while we harden it toward a hard panic.
			log::error!(
				"undo/redo shadow: cursor registry diverged from the restored interface\n{}",
				diff_registries(stored, &conversion.registry)
			);
		}
	}

	pub fn serialize_document(&self) -> String {
		let val = serde_json::to_string(self);
		// We fully expect the serialization to succeed
		val.unwrap()
	}

	pub fn deserialize_document(serialized_content: &str) -> Result<Self, EditorError> {
		let document_message_handler = serde_json::from_str::<DocumentMessageHandler>(serialized_content)
			.or_else(|e| {
				log::warn!("Failed to directly load document with the following error: {e}. Trying old DocumentMessageHandler.");
				// TODO: Eventually remove this document upgrade code
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
					/// The current mode that the user has set for rendering the document within the viewport.
					/// This is usually "Normal" but can be set to "Outline" or "Pixels" to see the canvas differently.
					pub view_mode: RenderMode,
					/// Sets whether or not all the viewport overlays should be drawn on top of the artwork.
					/// This includes tool interaction visualizations (like the transform cage and path anchors/handles), the grid, and more.
					pub overlays_visibility_settings: OverlaysVisibilitySettings,
					/// Sets whether or not the rulers should be drawn along the top and left edges of the viewport area.
					pub rulers_visible: bool,
					/// Sets whether or not the node graph is drawn (as an overlay) on top of the viewport area, or otherwise if it's hidden.
					pub graph_view_overlay_open: bool,
					/// The current user choices for snapping behavior, including whether snapping is enabled at all.
					pub snapping_state: SnappingState,
				}

				serde_json::from_str::<OldDocumentMessageHandler>(serialized_content).map(|old_message_handler| DocumentMessageHandler {
					network_interface: NodeNetworkInterface::from_old_network(old_message_handler.network),
					collapsed: old_message_handler.collapsed,
					commit_hash: old_message_handler.commit_hash,
					document_ptz: old_message_handler.document_ptz,
					render_mode: old_message_handler.view_mode,
					overlays_visibility_settings: old_message_handler.overlays_visibility_settings,
					rulers_visible: old_message_handler.rulers_visible,
					graph_view_overlay_open: old_message_handler.graph_view_overlay_open,
					snapping_state: old_message_handler.snapping_state,
					..Default::default()
				})
			})
			.map_err(|e| EditorError::DocumentDeserialization(e.to_string()))?;
		Ok(document_message_handler)
	}

	/// Builds the layer structure tree by traversing the node graph directly.
	/// Unlike the canonical `structure` field of [`DocumentMetadata`] (which stores single-parent relationships), this allows
	/// the same layer to appear under multiple parents when the graph feeds the same child content into separate parent layers.
	fn build_layer_structure(&self) -> Vec<LayerStructureEntry> {
		let network = &self.network_interface;

		let Some(root_node) = network.root_node(&[]) else { return Vec::new() };
		let Some(first_root_layer_id) = network
			.upstream_flow_back_from_nodes(vec![root_node.node_id], &[], FlowType::PrimaryFlow)
			.find(|node_id| network.is_layer(node_id, &[]))
		else {
			return Vec::new();
		};

		let selected_layers: HashSet<NodeId> = network.selected_nodes().selected_layers(self.metadata()).map(LayerNodeIdentifier::to_node).collect();

		let ancestors = HashSet::new();
		let tree_path = Vec::new();
		let mut root_entries = Vec::new();

		// The first root layer is the topmost entry
		root_entries.push(self.build_layer_entry(first_root_layer_id, &ancestors, &selected_layers, &tree_path));

		// Layers in the primary flow (input[0] chain) from the first root layer are root-level siblings
		let mut root_ancestors = HashSet::new();
		root_ancestors.insert(first_root_layer_id);

		for sibling_id in network.upstream_flow_back_from_nodes(vec![first_root_layer_id], &[], FlowType::PrimaryFlow).skip(1) {
			if network.is_layer(&sibling_id, &[]) && !root_ancestors.contains(&sibling_id) {
				root_entries.push(self.build_layer_entry(sibling_id, &root_ancestors, &selected_layers, &tree_path));
			}
		}

		root_entries
	}

	/// Builds a single `LayerStructureEntry` for the given layer, including its `children_present` flag,
	/// `descendant_selected` flag, and (if expanded) its children collected from the graph.
	fn build_layer_entry(&self, layer_id: NodeId, ancestors: &HashSet<NodeId>, selected_layers: &HashSet<NodeId>, parent_tree_path: &[NodeId]) -> LayerStructureEntry {
		let mut tree_path = parent_tree_path.to_vec();
		tree_path.push(layer_id);

		let mut child_ancestors = ancestors.clone();
		child_ancestors.insert(layer_id);

		let children_present = self.has_layer_children_in_graph(layer_id, &child_ancestors);

		let collapsed = self.collapsed.0.contains(&tree_path);

		let children = if children_present && !collapsed {
			self.collect_layer_children(layer_id, &child_ancestors, selected_layers, &tree_path)
		} else {
			Vec::new()
		};

		// Compute whether any descendant is selected (checking expanded children and, if collapsed, via graph traversal)
		let descendant_selected = if !children.is_empty() {
			children.iter().any(|child| child.descendant_selected || selected_layers.contains(&child.layer_id))
		} else if children_present {
			// Layer is collapsed but has children, so check via graph traversal
			self.has_selected_descendant_in_graph(layer_id, &child_ancestors, selected_layers)
		} else {
			false
		};

		LayerStructureEntry {
			layer_id,
			children,
			children_present,
			descendant_selected,
		}
	}

	/// Checks whether a layer has any child layers reachable via horizontal flow in the graph.
	fn has_layer_children_in_graph(&self, layer_id: NodeId, child_ancestors: &HashSet<NodeId>) -> bool {
		let network = &self.network_interface;

		network
			.upstream_flow_back_from_nodes(vec![layer_id], &[], FlowType::HorizontalFlow)
			.skip(1)
			.any(|id| network.is_layer(&id, &[]) && !child_ancestors.contains(&id))
	}

	/// Checks whether any descendant layer in the graph (via horizontal + primary flow) is selected.
	/// Used when a layer is collapsed to determine if the ancestor-of-selected indicator should show.
	fn has_selected_descendant_in_graph(&self, layer_id: NodeId, ancestors: &HashSet<NodeId>, selected_layers: &HashSet<NodeId>) -> bool {
		let network = &self.network_interface;

		// Find child layers via horizontal flow
		let mut stack: Vec<NodeId> = network
			.upstream_flow_back_from_nodes(vec![layer_id], &[], FlowType::HorizontalFlow)
			.skip(1)
			.filter(|node_id| network.is_layer(node_id, &[]) && !ancestors.contains(node_id))
			.collect();

		let mut visited = ancestors.clone();

		// Iteratively explore all descendant layers via a depth-first traversal
		while let Some(current_id) = stack.pop() {
			// Skip already-visited layers to avoid infinite loops from graph cycles
			if !visited.insert(current_id) {
				continue;
			}

			// Found a selected descendant, the ancestor indicator should be shown
			if selected_layers.contains(&current_id) {
				return true;
			}

			// Check this layer's children via horizontal flow
			for node_id in network.upstream_flow_back_from_nodes(vec![current_id], &[], FlowType::HorizontalFlow).skip(1) {
				if network.is_layer(&node_id, &[]) && !visited.contains(&node_id) {
					stack.push(node_id);
				}
			}

			// Check stacked siblings via primary flow
			for node_id in network.upstream_flow_back_from_nodes(vec![current_id], &[], FlowType::PrimaryFlow).skip(1) {
				if network.is_layer(&node_id, &[]) && !visited.contains(&node_id) {
					stack.push(node_id);
				}
			}
		}

		false
	}

	/// Collects the child entries for a given layer by traversing its horizontal and primary flows.
	/// The horizontal flow (a layer's secondary input chain) finds nested content layers, and the
	/// primary flow from those (their stack's top output) finds stacked siblings at the same depth.
	/// `ancestors` contains layer IDs in the current path from root, used for cycle prevention.
	fn collect_layer_children(&self, layer_id: NodeId, ancestors: &HashSet<NodeId>, selected_layers: &HashSet<NodeId>, tree_path: &[NodeId]) -> Vec<LayerStructureEntry> {
		let network = &self.network_interface;

		// Find the first nested layer via horizontal flow (content inside this layer)
		let Some(nested_id) = network
			.upstream_flow_back_from_nodes(vec![layer_id], &[], FlowType::HorizontalFlow)
			.skip(1)
			.find(|id| network.is_layer(id, &[]))
		else {
			return Vec::new();
		};

		// Cycle detected, this layer is already an ancestor in the current branch
		if ancestors.contains(&nested_id) {
			return Vec::new();
		}

		// The nested layer is the first child at this depth level
		let mut children = vec![self.build_layer_entry(nested_id, ancestors, selected_layers, tree_path)];

		// Primary flow from the nested layer finds stacked siblings (more children of this layer)
		for sibling_id in network.upstream_flow_back_from_nodes(vec![nested_id], &[], FlowType::PrimaryFlow).skip(1) {
			if network.is_layer(&sibling_id, &[]) && !ancestors.contains(&sibling_id) {
				children.push(self.build_layer_entry(sibling_id, ancestors, selected_layers, tree_path));
			}
		}

		children
	}

	/// Collects tree paths for all descendant layers of the given tree path by traversing the graph.
	/// Used for recursive collapse to find all expandable descendants.
	fn collect_descendant_tree_paths(&self, tree_path: &[NodeId]) -> Vec<Vec<NodeId>> {
		let Some(&layer_id) = tree_path.last() else { return Vec::new() };
		let network = &self.network_interface;

		let mut paths = Vec::new();
		let mut stack: Vec<(NodeId, Vec<NodeId>)> = Vec::new();

		// Seed with child layers via horizontal flow
		for node_id in network.upstream_flow_back_from_nodes(vec![layer_id], &[], FlowType::HorizontalFlow).skip(1) {
			if network.is_layer(&node_id, &[]) {
				let mut child_path = tree_path.to_vec();
				child_path.push(node_id);
				stack.push((node_id, child_path));
			}
		}

		let mut visited = HashSet::new();

		// Depth-first traversal collecting all unique descendant tree paths
		while let Some((current_id, current_path)) = stack.pop() {
			// Skip paths we've already visited to prevent cycles
			if !visited.insert(current_path.clone()) {
				continue;
			}

			// Record this descendant's tree path for collapsing
			paths.push(current_path.clone());

			// Add nested content layers found via horizontal flow
			for node_id in network.upstream_flow_back_from_nodes(vec![current_id], &[], FlowType::HorizontalFlow).skip(1) {
				if network.is_layer(&node_id, &[]) {
					let mut child_path = current_path.clone();
					child_path.push(node_id);
					stack.push((node_id, child_path));
				}
			}

			// Add stacked sibling layers found via primary flow
			for node_id in network.upstream_flow_back_from_nodes(vec![current_id], &[], FlowType::PrimaryFlow).skip(1) {
				if network.is_layer(&node_id, &[]) {
					// Siblings share the same parent path (everything up to the last element of current_path)
					let mut sibling_path = current_path[..current_path.len() - 1].to_vec();
					sibling_path.push(node_id);
					stack.push((node_id, sibling_path));
				}
			}
		}

		paths
	}

	pub fn undo_with_history(&mut self, document_id: DocumentId, viewport: &ViewportMessageHandler, resource_storage: &ResourceStorageMessageHandler, responses: &mut VecDeque<Message>) {
		// Apply the legacy snapshot synchronously so its `VecDeque` stays consistent and can serve as the
		// rebuild's oracle. The `Gdd` cursor then moves + persists and spawns the async rebuild that becomes
		// authoritative, overwriting the live interface when it lands. When no legacy snapshot applied (e.g.
		// an across-reopen undo where the legacy stack is empty but the persisted `Gdd` cursor can still
		// move), the rebuild overwrites with no comparison.
		let legacy_applied = if let Some(previous_network) = self.undo(viewport, responses) {
			self.document_redo_history.push_back(previous_network);
			if self.document_redo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
				self.document_redo_history.pop_front();
			}
			true
		} else {
			false
		};

		self.drive_storage_undo_redo(document_id, resource_storage, legacy_applied, true, responses);
	}

	pub fn undo(&mut self, viewport: &ViewportMessageHandler, responses: &mut VecDeque<Message>) -> Option<NodeNetworkInterface> {
		// If there is no history return and don't broadcast SelectionChanged
		let mut network_interface = self.document_undo_history.pop_back()?;

		// Set the previous network navigation metadata to the current navigation metadata
		network_interface.copy_all_navigation_metadata(&self.network_interface);
		std::mem::swap(&mut network_interface.resolved_types, &mut self.network_interface.resolved_types);

		//Update the metadata transform based on document PTZ
		let transform = self.navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), &self.document_ptz);
		network_interface.set_document_to_viewport_transform(transform);

		// Ensure document structure is loaded so that updating the selected nodes has the correct metadata
		network_interface.load_structure();

		let previous_network = std::mem::replace(&mut self.network_interface, network_interface);

		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		responses.add(NodeGraphMessage::SelectedNodesUpdated);
		responses.add(NodeGraphMessage::ForceRunDocumentGraph);

		// TODO: Remove once the footprint is used to load the imports/export distances from the edge
		responses.add(NodeGraphMessage::UnloadWires);

		Some(previous_network)
	}
	pub fn redo_with_history(&mut self, document_id: DocumentId, viewport: &ViewportMessageHandler, resource_storage: &ResourceStorageMessageHandler, responses: &mut VecDeque<Message>) {
		// Mirror `undo_with_history`: apply the legacy snapshot synchronously (oracle), then move the `Gdd`
		// cursor and spawn the authoritative async rebuild.
		let legacy_applied = if let Some(previous_network) = self.redo(viewport, responses) {
			self.document_undo_history.push_back(previous_network);
			if self.document_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
				self.document_undo_history.pop_front();
			}
			true
		} else {
			false
		};

		self.drive_storage_undo_redo(document_id, resource_storage, legacy_applied, false, responses);
	}

	pub fn redo(&mut self, viewport: &ViewportMessageHandler, responses: &mut VecDeque<Message>) -> Option<NodeNetworkInterface> {
		// If there is no history return and don't broadcast SelectionChanged
		let mut network_interface = self.document_redo_history.pop_back()?;

		// Set the previous network navigation metadata to the current navigation metadata
		network_interface.copy_all_navigation_metadata(&self.network_interface);
		std::mem::swap(&mut network_interface.resolved_types, &mut self.network_interface.resolved_types);

		//Update the metadata transform based on document PTZ
		let transform = self.navigation_handler.calculate_offset_transform(viewport.center_in_viewport_space().into(), &self.document_ptz);
		network_interface.set_document_to_viewport_transform(transform);

		let previous_network = std::mem::replace(&mut self.network_interface, network_interface);
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		responses.add(NodeGraphMessage::SelectedNodesUpdated);
		responses.add(NodeGraphMessage::ForceRunDocumentGraph);
		responses.add(NodeGraphMessage::UnloadWires);
		responses.add(NodeGraphMessage::SendWires);
		Some(previous_network)
	}

	pub fn current_hash(&self) -> u64 {
		self.network_interface.document_network().current_hash()
	}

	pub fn is_auto_saved(&self) -> bool {
		Some(self.current_hash()) == self.auto_saved_hash
	}

	pub fn is_saved(&self) -> bool {
		Some(self.current_hash()) == self.saved_hash
	}

	pub fn is_graph_overlay_open(&self) -> bool {
		self.graph_view_overlay_open
	}

	pub fn set_auto_save_state(&mut self, is_saved: bool) {
		if is_saved {
			self.auto_saved_hash = Some(self.current_hash());
		} else {
			self.auto_saved_hash = None;
		}
	}

	pub fn set_save_state(&mut self, is_saved: bool) {
		if is_saved {
			self.saved_hash = Some(self.current_hash());
		} else {
			self.saved_hash = None;
		}
	}

	/// Finds the artboard that bounds the point in viewport space and be the container of any newly added layers.
	pub fn new_layer_bounding_artboard(&self, ipp: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) -> LayerNodeIdentifier {
		let container_based_on_selection = self.new_layer_parent(true);

		let container_based_on_clicked_artboard = self
			.click_xray(ipp, viewport)
			.find(|layer| self.network_interface.is_artboard(&layer.to_node(), &[]))
			.unwrap_or(LayerNodeIdentifier::ROOT_PARENT);

		if container_based_on_selection.ancestors(self.metadata()).any(|ancestor| ancestor == container_based_on_clicked_artboard) {
			container_based_on_selection
		} else {
			container_based_on_clicked_artboard
		}
	}

	/// Finds the parent folder which, based on the current selections, should be the container of any newly added layers.
	pub fn new_layer_parent(&self, include_self: bool) -> LayerNodeIdentifier {
		let Some(selected_nodes) = self.network_interface.selected_nodes_in_nested_network(&self.selection_network_path) else {
			warn!("No selected nodes found in new_layer_parent. Defaulting to ROOT_PARENT.");
			return LayerNodeIdentifier::ROOT_PARENT;
		};

		self.network_interface
			.deepest_common_ancestor(&selected_nodes, &self.selection_network_path, include_self)
			.unwrap_or_else(|| self.network_interface.all_artboards().iter().next().copied().unwrap_or(LayerNodeIdentifier::ROOT_PARENT))
	}

	pub fn get_calculated_insert_index(metadata: &DocumentMetadata, selected_nodes: &SelectedNodes, parent: LayerNodeIdentifier) -> usize {
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

	/// Copies `layer` together with its full upstream node chain, queueing the new nodes with freshly minted IDs.
	/// Returns the new layer's identifier; the caller places it into the stack with a `MoveLayerToStack` response.
	fn duplicate_layer(&mut self, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> Option<LayerNodeIdentifier> {
		let mut copy_ids = HashMap::new();
		copy_ids.insert(layer.to_node(), NodeId(0));
		self.network_interface
			.upstream_flow_back_from_nodes(vec![layer.to_node()], &[], FlowType::LayerChildrenUpstreamFlow)
			.enumerate()
			.for_each(|(index, node_id)| {
				copy_ids.insert(node_id, NodeId((index + 1) as u64));
			});

		let nodes = self.network_interface.copy_nodes(&copy_ids, &[]).collect::<Vec<(NodeId, NodeTemplate)>>();
		let new_ids: HashMap<_, _> = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect();

		let Some(&new_layer_id) = new_ids.get(&NodeId(0)) else {
			log::error!("Could not duplicate layer because its root node copy is missing");
			return None;
		};
		responses.add(NodeGraphMessage::AddNodes { nodes, new_ids });

		Some(LayerNodeIdentifier::new_unchecked(new_layer_id))
	}

	pub fn group_layers(
		responses: &mut VecDeque<Message>,
		insert_index: usize,
		parent: LayerNodeIdentifier,
		group_folder_type: GroupFolderType,
		network_interface: &mut NodeNetworkInterface,
	) -> NodeId {
		let folder_id = NodeId(generate_uuid());

		match group_folder_type {
			GroupFolderType::Layer => responses.add(GraphOperationMessage::NewCustomLayer {
				id: folder_id,
				nodes: Vec::new(),
				parent,
				insert_index,
			}),
			GroupFolderType::BooleanOperation(operation) => {
				// Get the ID of the one selected layer, if exactly one is selected
				let only_selected_layer = {
					let selected_nodes = network_interface.selected_nodes();
					let mut layers = selected_nodes.selected_layers(network_interface.document_metadata());

					match (layers.next(), layers.next()) {
						(Some(id), None) => Some(id),
						_ => None,
					}
				};

				// If there is a single selected layer, check if there is a boolean operation upstream from it
				let upstream_boolean_op = only_selected_layer.and_then(|selected_id| {
					network_interface.upstream_flow_back_from_nodes(vec![selected_id.to_node()], &[], FlowType::HorizontalFlow).find(|id| {
						network_interface
							.reference(id, &[])
							.is_some_and(|reference| reference == DefinitionIdentifier::ProtoNode(graphene_std::path_bool_nodes::boolean_operation::IDENTIFIER))
					})
				});

				// If there's already a boolean operation on the selected layer, update it with the new operation
				if let (Some(upstream_boolean_op), Some(only_selected_layer)) = (upstream_boolean_op, only_selected_layer) {
					network_interface.set_input(&InputConnector::node(upstream_boolean_op, 1), NodeInput::value(TaggedValue::BooleanOperation(operation), false), &[]);

					responses.add(NodeGraphMessage::RunDocumentGraph);

					return only_selected_layer.to_node();
				}
				// Otherwise, create a new boolean operation node group
				else {
					responses.add(GraphOperationMessage::NewBooleanOperationLayer {
						id: folder_id,
						operation,
						parent,
						insert_index,
					});
				}
			}
			GroupFolderType::Blend | GroupFolderType::Morph => {
				let control_path_id = NodeId(generate_uuid());
				let all_layers_to_group = network_interface.shallowest_unique_layers_sorted(&[]);
				let blend_count = matches!(group_folder_type, GroupFolderType::Blend).then(|| all_layers_to_group.len() * BLEND_COUNT_PER_LAYER);

				responses.add(GraphOperationMessage::NewInterpolationLayer {
					id: folder_id,
					control_path_id,
					parent,
					insert_index,
					blend_count,
				});

				let new_group_folder = LayerNodeIdentifier::new_unchecked(folder_id);

				// Move selected layers into the group as children
				for layer_to_group in all_layers_to_group.into_iter().rev() {
					responses.add(NodeGraphMessage::MoveLayerToStack {
						layer: layer_to_group,
						parent: new_group_folder,
						insert_index: 0,
					});
				}

				// Connect the child stack to the control path layer as a co-parent
				responses.add(GraphOperationMessage::ConnectInterpolationControlPathToChildren {
					interpolation_layer_id: folder_id,
					control_path_id,
				});

				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![folder_id] });
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(DocumentMessage::DocumentStructureChanged);
				responses.add(NodeGraphMessage::SendGraph);

				// The control path layer (Blend Path / Morph Path) should start collapsed.
				let tree_path = {
					// Build tree path from root down to the control path layer, which is a sibling of the main layer under `parent`.
					let mut tree_path: Vec<NodeId> = parent
						.ancestors(network_interface.document_metadata())
						.take_while(|&ancestor| ancestor != LayerNodeIdentifier::ROOT_PARENT)
						.map(LayerNodeIdentifier::to_node)
						.collect();
					tree_path.reverse();
					tree_path.push(control_path_id);
					tree_path
				};
				responses.add(DocumentMessage::ToggleLayerExpansion { tree_path, recursive: false });

				return folder_id;
			}
		}

		let new_group_folder = LayerNodeIdentifier::new_unchecked(folder_id);
		// Move the new folder to the correct position
		responses.add(NodeGraphMessage::MoveLayerToStack {
			layer: new_group_folder,
			parent,
			insert_index,
		});
		responses.add(DocumentMessage::MoveSelectedLayersToGroup { parent: new_group_folder });

		folder_id
	}

	fn group_selected_layers(&mut self, group_folder_type: GroupFolderType, responses: &mut VecDeque<Message>) {
		responses.add(DocumentMessage::AddTransaction);

		let mut parent_per_selected_nodes: HashMap<LayerNodeIdentifier, Vec<NodeId>> = HashMap::new();
		let artboards = LayerNodeIdentifier::ROOT_PARENT
			.children(self.metadata())
			.filter(|x| self.network_interface.is_artboard(&x.to_node(), &self.selection_network_path))
			.collect::<Vec<_>>();
		let selected_nodes = self.network_interface.selected_nodes();

		// Non-artboard (infinite canvas) workflow
		if artboards.is_empty() {
			let Some(parent) = self.network_interface.deepest_common_ancestor(&selected_nodes, &self.selection_network_path, false) else {
				return;
			};
			let Some(selected_nodes) = &self.network_interface.selected_nodes_in_nested_network(&self.selection_network_path) else {
				return;
			};
			let insert_index = DocumentMessageHandler::get_calculated_insert_index(self.metadata(), selected_nodes, parent);

			DocumentMessageHandler::group_layers(responses, insert_index, parent, group_folder_type, &mut self.network_interface);
		}
		// Artboard workflow
		else {
			for artboard in artboards {
				let selected_descendants = artboard.descendants(self.metadata()).filter(|x| selected_nodes.selected_layers_contains(*x, self.metadata()));
				for selected_descendant in selected_descendants {
					parent_per_selected_nodes.entry(artboard).or_default().push(selected_descendant.to_node());
				}
			}

			let mut new_folders: Vec<NodeId> = Vec::new();

			for children in parent_per_selected_nodes.into_values() {
				let child_selected_nodes = SelectedNodes(children);
				let Some(parent) = self.network_interface.deepest_common_ancestor(&child_selected_nodes, &self.selection_network_path, false) else {
					continue;
				};
				let insert_index = DocumentMessageHandler::get_calculated_insert_index(self.metadata(), &child_selected_nodes, parent);

				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: child_selected_nodes.0 });

				new_folders.push(DocumentMessageHandler::group_layers(responses, insert_index, parent, group_folder_type, &mut self.network_interface));
			}

			responses.add(NodeGraphMessage::SelectedNodesSet { nodes: new_folders });
		}
	}

	/// For each selected layer, splits its fill and stroke into two stacked layers connected
	/// to a shared `Solidify Stroke` node via two `Index Elements` nodes (indices 0 and 1).
	/// Layers with only a stroke get just a `Solidify Stroke` added.
	/// Layers with only a fill, or neither, are left untouched.
	fn handle_expand_fill_stroke_on_selected_layers(&mut self, responses: &mut VecDeque<Message>) {
		let selected_layers: Vec<LayerNodeIdentifier> = self.network_interface.selected_nodes().selected_layers(self.metadata()).collect();
		if selected_layers.is_empty() {
			return;
		}

		let solidify_stroke_definition = document_node_definitions::resolve_proto_node_type(graphene_std::vector::solidify_stroke::IDENTIFIER).expect("Solidify Stroke node should exist");
		let index_elements_definition = document_node_definitions::resolve_proto_node_type(graphene_std::graphic::index_elements::IDENTIFIER).expect("Index Elements node should exist");

		let mut resulting_layers: Vec<NodeId> = Vec::new();

		for layer in selected_layers {
			let style = self.network_interface.document_metadata().layer_vector_data.get(&layer).map(|arc| arc.style.clone());
			let Some(style) = style else {
				resulting_layers.push(layer.to_node());
				continue;
			};

			let fill_graphic_list = self.network_interface.document_metadata().layer_fill_attributes.get(&layer);
			let stroke_graphic_list = self.network_interface.document_metadata().layer_stroke_attributes.get(&layer);

			let has_fill = if let Some(list) = fill_graphic_list {
				list.element(0).is_some()
			} else {
				!matches!(style.fill, Fill::None)
			};
			// `style.stroke` is `Some` whenever a `Stroke` node is in the chain, even with weight 0 or a transparent color.
			// So `is_some()` would treat invisibly-stroked fill-only layers as having a stroke.
			// `ATTR_STROKE` is the source of truth when set; fall back to `style.stroke.color` only when no attribute is present.
			let stroke_visible = if let Some(list) = stroke_graphic_list {
				list.element(0).is_some_and(|g| !g.is_fully_transparent())
			} else {
				style.stroke.as_ref().and_then(|s| s.color()).is_some_and(|c| c.a() != 0.)
			};
			let has_stroke = style.stroke.as_ref().is_some_and(|s| s.has_renderable_stroke()) && stroke_visible;

			// No stroke means there's nothing to solidify. Fill-only layers are already in the desired form, so skip.
			if !has_stroke {
				resulting_layers.push(layer.to_node());
				continue;
			}

			let solidify_id = NodeId::new();
			self.network_interface.insert_node(solidify_id, solidify_stroke_definition.default_node_template(), &[]);
			self.network_interface.move_node_to_chain_start(&solidify_id, layer, &[], false);

			if has_fill && has_stroke {
				let (existing_index, new_index) = (0_f64, 1_f64);

				let existing_index_template = index_elements_definition.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(existing_index), false))]);
				let existing_index_id = NodeId::new();
				self.network_interface.insert_node(existing_index_id, existing_index_template, &[]);
				self.network_interface.move_node_to_chain_start(&existing_index_id, layer, &[], false);

				let parent = layer.parent(self.metadata()).unwrap_or(LayerNodeIdentifier::ROOT_PARENT);
				let insert_index = parent.children(self.metadata()).position(|c| c == layer).unwrap_or(0);

				let new_layer_id = NodeId::new();
				let new_layer = ModifyInputsContext::new(&mut self.network_interface, responses).create_layer(new_layer_id);
				self.network_interface.move_layer_to_stack(new_layer, parent, insert_index, &[]);

				// Copy the original layer's stored name so the new layer shares it
				let original_name = self
					.network_interface
					.node_metadata(&layer.to_node(), &[])
					.map(|m| m.persistent_metadata.display_name.clone())
					.unwrap_or_default();
				if !original_name.is_empty() {
					self.network_interface.set_display_name(&new_layer_id, original_name, &[]);
				}

				let new_index_template = index_elements_definition.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(new_index), false))]);
				let new_index_id = NodeId::new();
				self.network_interface.insert_node(new_index_id, new_index_template, &[]);
				self.network_interface.move_node_to_chain_start(&new_index_id, new_layer, &[], false);

				self.network_interface.create_wire(&OutputConnector::node(solidify_id, 0), &InputConnector::node(new_index_id, 0), &[]);

				resulting_layers.push(layer.to_node());
				resulting_layers.push(new_layer.to_node());
			} else {
				resulting_layers.push(layer.to_node());
			}
		}

		responses.add(NodeGraphMessage::SelectedNodesSet { nodes: resulting_layers });
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	fn move_selected_layers_to(&mut self, parent: LayerNodeIdentifier, insert_index: usize, responses: &mut VecDeque<Message>) {
		if !self.selection_network_path.is_empty() {
			log::error!("Moving selected layers is only supported for the Document Network");
			return;
		}

		// Disallow trying to insert into self.
		if self
			.network_interface
			.selected_nodes()
			.selected_layers(self.metadata())
			.any(|layer| parent.ancestors(self.metadata()).any(|ancestor| ancestor == layer))
		{
			return;
		}
		// Artboards can only have `ROOT_PARENT` as the parent.
		let any_artboards = self
			.network_interface
			.selected_nodes()
			.selected_layers(self.metadata())
			.any(|layer| self.network_interface.is_artboard(&layer.to_node(), &self.selection_network_path));
		if any_artboards && parent != LayerNodeIdentifier::ROOT_PARENT {
			return;
		}

		// Non-artboards cannot be put at the top level if artboards also exist there
		let selected_any_non_artboards = self
			.network_interface
			.selected_nodes()
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
					return (*layer, 0);
				}

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
			})
			.collect::<Vec<_>>();

		responses.add(DocumentMessage::AddTransaction);

		for (layer_index, (layer_to_move, insert_offset)) in layers_to_move_with_insert_offset.into_iter().enumerate() {
			responses.add(NodeGraphMessage::MoveLayerToStack {
				layer: layer_to_move,
				parent,
				insert_index: insert_index + layer_index - insert_offset,
			});

			if layer_to_move.parent(self.metadata()) != Some(parent) {
				// TODO: Fix this so it works when dragging a layer into a group parent which has a Transform node, which used to work before #2689 caused this regression by removing the empty `List<Vector>` item.
				// TODO: See #2688 for this issue.
				let layer_local_transform = self.network_interface.document_metadata().transform_to_viewport(layer_to_move);
				let undo_transform = self.network_interface.document_metadata().transform_to_viewport(parent).inverse();
				let transform = undo_transform * layer_local_transform;

				responses.add(GraphOperationMessage::TransformSet {
					layer: layer_to_move,
					transform,
					transform_in: TransformIn::Local,
					skip_rerender: false,
				});
			}
		}

		responses.add(NodeGraphMessage::RunDocumentGraph);
		responses.add(NodeGraphMessage::SendGraph);
	}

	fn nudge_selected_layers(&mut self, delta_x: f64, delta_y: f64, resize: bool, resize_opposite: bool, responses: &mut VecDeque<Message>) {
		let can_move = |layer| {
			let selected = self.network_interface.selected_nodes();
			selected.layer_visible(layer, &self.network_interface) && !selected.layer_locked(layer, &self.network_interface)
		};

		if resize {
			let layers: Vec<_> = self.network_interface.shallowest_unique_layers(&[]).filter(|&layer| can_move(layer)).collect();
			// Combine only finite bounds (a non-finite box would poison the scale), bailing before opening a transaction if none remain
			let Some([min, max]) = layers
				.iter()
				.filter_map(|&layer| self.metadata().bounding_box_document(layer))
				.filter(|[min, max]| min.is_finite() && max.is_finite())
				.reduce(Quad::combine_bounds)
			else {
				return;
			};

			let resized = nudge_resize_bounds(min, max, DVec2::new(delta_x, delta_y), self.document_ptz.tilt(), resize_opposite);

			// Express the document-space scale in viewport space so it composes with each layer like the rest of the transform pipeline
			let document_to_viewport = self.metadata().document_to_viewport;
			let transform = document_to_viewport * resized.transform * document_to_viewport.inverse();

			responses.add(DocumentMessage::AddTransaction);

			for layer in layers {
				responses.add(GraphOperationMessage::TransformChange {
					layer,
					transform,
					transform_in: TransformIn::Viewport,
					skip_rerender: false,
				});
			}

			return;
		}

		responses.add(DocumentMessage::AddTransaction);

		let transform = DAffine2::from_translation(DVec2::from_angle(-self.document_ptz.tilt()).rotate(DVec2::new(delta_x, delta_y)));
		responses.add(SelectToolMessage::ShiftSelectedNodes { offset: transform.translation });

		for layer in self.network_interface.shallowest_unique_layers(&[]).filter(|layer| can_move(*layer)) {
			responses.add(GraphOperationMessage::TransformChange {
				layer,
				transform,
				transform_in: TransformIn::Local,
				skip_rerender: false,
			});
		}
	}

	pub fn update_document_widgets(&self, responses: &mut VecDeque<Message>, animation_is_playing: bool, time: Duration) {
		let mut snapping_state = self.snapping_state.clone();
		let mut snapping_state2 = self.snapping_state.clone();

		let mut widgets = vec![
			IconButton::new("PlaybackToStart", 24)
				.tooltip_label("Restart Animation")
				.tooltip_shortcut(action_shortcut!(AnimationMessageDiscriminant::RestartAnimation))
				.on_update(|_| AnimationMessage::RestartAnimation.into())
				.disabled(time == Duration::ZERO)
				.widget_instance(),
			IconButton::new(if animation_is_playing { "PlaybackPause" } else { "PlaybackPlay" }, 24)
				.tooltip_label(if animation_is_playing { "Pause Animation" } else { "Play Animation" })
				.tooltip_shortcut(action_shortcut!(AnimationMessageDiscriminant::ToggleLivePreview))
				.on_update(|_| AnimationMessage::ToggleLivePreview.into())
				.widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			CheckboxInput::new(self.overlays_visibility_settings.all)
				.icon("Overlays")
				.tooltip_label("Overlays")
				.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::ToggleOverlaysVisibility))
				.on_update(|optional_input: &CheckboxInput| {
					DocumentMessage::SetOverlaysVisibility {
						visible: optional_input.checked,
						overlays_type: None,
					}
					.into()
				})
				.widget_instance(),
			PopoverButton::new()
				.popover_layout(Layout(vec![
					LayoutGroup::row(vec![TextLabel::new("Overlays").bold(true).widget_instance()]),
					LayoutGroup::row(vec![TextLabel::new("General").widget_instance()]),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.artboard_name)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::ArtboardName),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Artboard Name".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.transform_measurement)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::TransformMeasurement),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("G/R/S Measurement".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row(vec![TextLabel::new("Select Tool").widget_instance()]),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.quick_measurement)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::QuickMeasurement),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Quick Measurement".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.transform_cage)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::TransformCage),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Transform Cage".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.compass_rose)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::CompassRose),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Transform Dial".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.pivot)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::Pivot),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Transform Pivot".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.origin)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::Origin),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Transform Origin".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.hover_outline)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::HoverOutline),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Hover Outline".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.selection_outline)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::SelectionOutline),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Selection Outline".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.layer_origin_cross)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::LayerOriginCross),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Layer Origin".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row(vec![TextLabel::new("Pen & Path Tools").widget_instance()]),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.path)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::Path),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Path".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.anchors)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::Anchors),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Anchors".to_string()).for_checkbox(checkbox_id).widget_instance(),
						]
					}),
					LayoutGroup::row({
						let checkbox_id = CheckboxId::new();
						vec![
							CheckboxInput::new(self.overlays_visibility_settings.handles)
								.disabled(!self.overlays_visibility_settings.anchors)
								.on_update(|optional_input: &CheckboxInput| {
									DocumentMessage::SetOverlaysVisibility {
										visible: optional_input.checked,
										overlays_type: Some(OverlaysType::Handles),
									}
									.into()
								})
								.for_label(checkbox_id)
								.widget_instance(),
							TextLabel::new("Handles".to_string())
								.disabled(!self.overlays_visibility_settings.anchors)
								.for_checkbox(checkbox_id)
								.widget_instance(),
						]
					}),
				]))
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			CheckboxInput::new(snapping_state.snapping_enabled)
				.icon("Snapping")
				.tooltip_label("Snapping")
				.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::ToggleSnapping))
				.on_update(move |optional_input: &CheckboxInput| {
					DocumentMessage::SetSnapping {
						closure: Some(|snapping_state| &mut snapping_state.snapping_enabled),
						snapping_state: optional_input.checked,
					}
					.into()
				})
				.widget_instance(),
			PopoverButton::new()
				.popover_layout(Layout(
					[
						LayoutGroup::row(vec![TextLabel::new("Snapping").bold(true).widget_instance()]),
						LayoutGroup::row(vec![TextLabel::new(SnappingOptions::BoundingBoxes.to_string()).widget_instance()]),
					]
					.into_iter()
					.chain(SNAP_FUNCTIONS_FOR_BOUNDING_BOXES.into_iter().map(|(name, closure, description)| {
						LayoutGroup::row({
							let checkbox_id = CheckboxId::new();
							vec![
								CheckboxInput::new(*closure(&mut snapping_state))
									.on_update(move |input: &CheckboxInput| {
										DocumentMessage::SetSnapping {
											closure: Some(closure),
											snapping_state: input.checked,
										}
										.into()
									})
									.tooltip_label(name)
									.tooltip_description(description)
									.for_label(checkbox_id)
									.widget_instance(),
								TextLabel::new(name).tooltip_label(name).tooltip_description(description).for_checkbox(checkbox_id).widget_instance(),
							]
						})
					}))
					.chain([LayoutGroup::row(vec![TextLabel::new(SnappingOptions::Paths.to_string()).widget_instance()])])
					.chain(SNAP_FUNCTIONS_FOR_PATHS.into_iter().map(|(name, closure, description)| {
						LayoutGroup::row({
							let checkbox_id = CheckboxId::new();
							vec![
								CheckboxInput::new(*closure(&mut snapping_state2))
									.on_update(move |input: &CheckboxInput| {
										DocumentMessage::SetSnapping {
											closure: Some(closure),
											snapping_state: input.checked,
										}
										.into()
									})
									.tooltip_label(name)
									.tooltip_description(description)
									.for_label(checkbox_id)
									.widget_instance(),
								TextLabel::new(name).tooltip_label(name).tooltip_description(description).for_checkbox(checkbox_id).widget_instance(),
							]
						})
					}))
					.collect(),
				))
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			CheckboxInput::new(self.snapping_state.grid_snapping)
				.icon("Grid")
				.tooltip_label("Grid")
				.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::ToggleGridVisibility))
				.on_update(|optional_input: &CheckboxInput| DocumentMessage::GridVisibility { visible: optional_input.checked }.into())
				.widget_instance(),
			PopoverButton::new()
				.popover_layout(Layout(overlay_options(&self.snapping_state.grid)))
				.popover_min_width(Some(320))
				.widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			{
				let disabled = cfg!(target_family = "wasm") && wgpu_available() == Some(false);

				let mut entries = vec![
					RadioEntryData::new("Normal")
						.icon("RenderModeNormal")
						.tooltip_label("Render Mode: Normal")
						.on_update(|_| DocumentMessage::SetRenderMode { render_mode: RenderMode::Normal }.into()),
					RadioEntryData::new("Outline")
						.icon("RenderModeOutline")
						.tooltip_label("Render Mode: Outline")
						.on_update(|_| DocumentMessage::SetRenderMode { render_mode: RenderMode::Outline }.into()),
					RadioEntryData::new("PixelPreview").icon("RenderModePixels").tooltip_label("Render Mode: Pixel Preview").on_update(|_| {
						DocumentMessage::SetRenderMode {
							render_mode: RenderMode::PixelPreview,
						}
						.into()
					}),
					RadioEntryData::new("SvgPreview")
						.icon("RenderModeSvg")
						.tooltip_label("Render Mode: SVG Preview")
						.on_update(|_| DocumentMessage::SetRenderMode { render_mode: RenderMode::SvgPreview }.into()),
				];
				let mut selected_index = self.render_mode as u32;

				if disabled {
					for entry in &mut entries {
						entry.tooltip_description = "
							*Normal*, *Outline*, and *Pixel Preview* render modes are not available in this browser. For compatibility, *SVG Preview* mode is active as a fallback.\n\
							\n\
							This functionality requires WebGPU support. Check webgpu.org for browser implementation status.
							"
						.trim()
						.into();
					}

					selected_index = entries.iter().position(|entry| entry.value == "SvgPreview").unwrap() as u32;
				}

				RadioInput::new(entries).selected_index(Some(selected_index)).disabled(disabled).narrow(true).widget_instance()
			},
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		];

		widgets.extend(navigation_controls(&self.document_ptz, &self.navigation_handler, false));

		let tilt_value = self.navigation_handler.snapped_tilt(self.document_ptz.tilt()) / (std::f64::consts::PI / 180.);
		if tilt_value.abs() > 0.00001 {
			widgets.extend([
				Separator::new(SeparatorStyle::Related).widget_instance(),
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
					.tooltip_label("Canvas Tilt")
					.on_update(|number_input: &NumberInput| {
						NavigationMessage::CanvasTiltSet {
							angle_radians: number_input.value.unwrap().to_radians(),
						}
						.into()
					})
					.widget_instance(),
			]);
		}

		widgets.extend([
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextButton::new("Node Graph")
				.icon(if self.graph_view_overlay_open { "GraphViewOpen" } else { "GraphViewClosed" })
				.hover_icon(if self.graph_view_overlay_open { "GraphViewClosed" } else { "GraphViewOpen" })
				.tooltip_label(if self.graph_view_overlay_open { "Hide Node Graph" } else { "Show Node Graph" })
				.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::GraphViewOverlayToggle))
				.on_update(move |_| DocumentMessage::GraphViewOverlayToggle.into())
				.widget_instance(),
		]);

		responses.add(LayoutMessage::SendLayout {
			layout: Layout(vec![LayoutGroup::row(widgets)]),
			layout_target: LayoutTarget::DocumentBar,
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn update_layers_panel_control_bar_widgets(&self, layers_panel_open: bool, responses: &mut VecDeque<Message>) {
		if !layers_panel_open {
			return;
		}

		// Get an iterator over the selected layers (excluding artboards which don't have an opacity or blend mode).
		let selected_nodes = self.network_interface.selected_nodes();
		let selected_layers_except_artboards = selected_nodes.selected_layers_except_artboards(&self.network_interface);

		// Look up the current opacity and blend mode of the selected layers (if any), and split the iterator into the first tuple and the rest.
		let mut blending_options = selected_layers_except_artboards.map(|layer| {
			(
				get_opacity(layer, &self.network_interface).unwrap_or(100.),
				get_fill(layer, &self.network_interface).unwrap_or(100.),
				get_blend_mode(layer, &self.network_interface).unwrap_or_default(),
			)
		});
		let first_blending_options = blending_options.next();
		let result_blending_options = blending_options;

		// If there are no selected layers, disable the opacity and blend mode widgets.
		let disabled = first_blending_options.is_none();

		// Amongst the selected layers, check if the opacities and blend modes are identical across all layers.
		// The result is setting `option` and `blend_mode` to Some value if all their values are identical, or None if they are not.
		// If identical, we display the value in the widget. If not, we display a dash indicating dissimilarity.
		let (opacity, fill, blend_mode) = first_blending_options
			.map(|(first_opacity, first_fill, first_blend_mode)| {
				let mut opacity_identical = true;
				let mut fill_identical = true;
				let mut blend_mode_identical = true;

				for (opacity, fill, blend_mode) in result_blending_options {
					if (opacity - first_opacity).abs() > (f64::EPSILON * 100.) {
						opacity_identical = false;
					}
					if (fill - first_fill).abs() > (f64::EPSILON * 100.) {
						fill_identical = false;
					}
					if blend_mode != first_blend_mode {
						blend_mode_identical = false;
					}
				}

				(
					opacity_identical.then_some(first_opacity),
					fill_identical.then_some(first_fill),
					blend_mode_identical.then_some(first_blend_mode),
				)
			})
			.unwrap_or((None, None, None));

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

		let has_selection = self.network_interface.selected_nodes().selected_layers(self.metadata()).next().is_some();
		let selection_all_visible = self
			.network_interface
			.selected_nodes()
			.selected_layers(self.metadata())
			.all(|layer| self.network_interface.is_visible(&layer.to_node(), &[]));
		let selection_all_locked = self
			.network_interface
			.selected_nodes()
			.selected_layers(self.metadata())
			.all(|layer| self.network_interface.is_locked(&layer.to_node(), &[]));

		let widgets = vec![
			DropdownInput::new(blend_mode_menu_entries)
				.selected_index(blend_mode.and_then(|blend_mode| blend_mode.index_in_list_svg_subset()).map(|index| index as u32))
				.disabled(disabled)
				.draw_icon(false)
				.max_width(100)
				.tooltip_label("Blend Mode")
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			NumberInput::new(opacity)
				.label("Opacity")
				.unit("%")
				.display_decimal_places(0)
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
				.max_width(100)
				.tooltip_label("Opacity")
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			NumberInput::new(fill)
				.label("Fill")
				.unit("%")
				.display_decimal_places(0)
				.disabled(disabled)
				.min(0.)
				.max(100.)
				.range_min(Some(0.))
				.range_max(Some(100.))
				.mode_range()
				.on_update(|number_input: &NumberInput| {
					if let Some(value) = number_input.value {
						DocumentMessage::SetFillForSelectedLayers { fill: value / 100. }.into()
					} else {
						Message::NoOp
					}
				})
				.on_commit(|_| DocumentMessage::AddTransaction.into())
				.max_width(100)
				.tooltip_label("Fill")
				.widget_instance(),
		];
		let layers_panel_control_bar_left = Layout(vec![LayoutGroup::row(widgets)]);

		let widgets = vec![
			IconButton::new(if selection_all_locked { "PadlockLocked" } else { "PadlockUnlocked" }, 24)
				.hover_icon(if selection_all_locked { "PadlockUnlocked" } else { "PadlockLocked" })
				.tooltip_label(if selection_all_locked { "Unlock Selected" } else { "Lock Selected" })
				.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::ToggleSelectedLocked))
				.on_update(|_| NodeGraphMessage::ToggleSelectedLocked.into())
				.disabled(!has_selection)
				.widget_instance(),
			IconButton::new(if selection_all_visible { "EyeVisible" } else { "EyeHidden" }, 24)
				.hover_icon(if selection_all_visible { "EyeHide" } else { "EyeShow" })
				.tooltip_label(if selection_all_visible { "Hide Selected" } else { "Show Selected" })
				.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::ToggleSelectedVisibility))
				.on_update(|_| DocumentMessage::ToggleSelectedVisibility.into())
				.disabled(!has_selection)
				.widget_instance(),
		];
		let layers_panel_control_bar_right = Layout(vec![LayoutGroup::row(widgets)]);

		responses.add(LayoutMessage::SendLayout {
			layout: layers_panel_control_bar_left,
			layout_target: LayoutTarget::LayersPanelControlLeftBar,
		});
		responses.add(LayoutMessage::SendLayout {
			layout: layers_panel_control_bar_right,
			layout_target: LayoutTarget::LayersPanelControlRightBar,
		});
	}

	pub fn update_layers_panel_bottom_bar_widgets(&mut self, layers_panel_open: bool, responses: &mut VecDeque<Message>) {
		if !layers_panel_open {
			return;
		}

		let selected_nodes = self.network_interface.selected_nodes();
		let mut selected_layers = selected_nodes.selected_layers(self.metadata());
		let selected_layer = selected_layers.next();
		let has_selection = selected_layer.is_some();
		let has_multiple_selection = selected_layers.next().is_some();
		for _ in selected_layers {}

		let widgets = vec![
			PopoverButton::new()
				.icon("Node")
				.menu_direction(Some(MenuDirection::Top))
				.tooltip_description("Add an operation to the end of this layer's chain of nodes.")
				.disabled(!has_selection || has_multiple_selection)
				.popover_layout({
					// Showing only compatible types for the layer based on the output type of the node upstream from its horizontal input
					let compatible_type = selected_layer.and_then(|layer| {
						self.network_interface
							.upstream_output_connector(&InputConnector::node(layer.to_node(), 1), &[])
							.and_then(|upstream_output| self.network_interface.output_type(&upstream_output, &[]).add_node_string())
					});

					let mut node_chooser = NodeCatalog::new();
					node_chooser.intial_search = compatible_type.unwrap_or("".to_string());

					let node_chooser = node_chooser
						.on_update(move |node_type| {
							if let Some(layer) = selected_layer {
								NodeGraphMessage::CreateNodeInLayerWithTransaction {
									node_type: node_type.clone(),
									layer: LayerNodeIdentifier::new_unchecked(layer.to_node()),
								}
								.into()
							} else {
								Message::NoOp
							}
						})
						.widget_instance();
					Layout(vec![LayoutGroup::row(vec![node_chooser])])
				})
				.widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			IconButton::new("Folder", 24)
				.tooltip_label("Group Selected")
				.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::GroupSelectedLayers))
				.on_update(|_| {
					let group_folder_type = GroupFolderType::Layer;
					DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
				})
				.on_drag_drop(|_| {
					let group_folder_type = GroupFolderType::Layer;
					DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
				})
				.disabled(!has_selection)
				.widget_instance(),
			IconButton::new("NewLayer", 24)
				.tooltip_label("New Layer")
				.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::CreateEmptyFolder))
				.on_update(|_| DocumentMessage::CreateEmptyFolder.into())
				.on_drag_drop(|_| DocumentMessage::DuplicateSelectedLayers.into())
				.widget_instance(),
			IconButton::new("Trash", 24)
				.tooltip_label("Delete Selected")
				.tooltip_shortcut(action_shortcut!(DocumentMessageDiscriminant::DeleteSelectedLayers))
				.on_update(|_| DocumentMessage::DeleteSelectedLayers.into())
				.on_drag_drop(|_| DocumentMessage::DeleteSelectedLayers.into())
				.disabled(!has_selection)
				.widget_instance(),
		];
		responses.add(LayoutMessage::SendLayout {
			layout: Layout(vec![LayoutGroup::row(widgets)]),
			layout_target: LayoutTarget::LayersPanelBottomBar,
		});
	}

	pub fn selected_layers_reverse(&mut self, responses: &mut VecDeque<Message>) {
		let selected_layers = self.network_interface.selected_nodes();
		let metadata = self.metadata();
		let selected_layer_set = selected_layers.selected_layers(metadata).collect::<HashSet<_>>();

		// Ignore those with selected ancestors
		let mut top_level_layers = Vec::new();
		for &layer in &selected_layer_set {
			let mut is_top_level = true;
			let mut current_layer = layer;

			while let Some(parent) = current_layer.parent(metadata) {
				if selected_layer_set.contains(&parent) {
					is_top_level = false;
					break;
				}
				current_layer = parent;
			}

			if is_top_level {
				top_level_layers.push(layer);
			}
		}

		// Group selected layers by their parent
		let mut grouped_layers: HashMap<LayerNodeIdentifier, Vec<(usize, LayerNodeIdentifier)>> = HashMap::new();
		for &layer in &top_level_layers {
			if let Some(parent) = layer.parent(metadata) {
				let index = parent.children(metadata).position(|child| child == layer).unwrap_or(usize::MAX);

				grouped_layers.entry(parent).or_default().push((index, layer));
			}
		}

		let mut modified = false;

		// Process each group separately
		for (parent, mut layers) in grouped_layers {
			// Retrieve all children under the parent
			let all_children = parent.children(metadata).collect::<Vec<_>>();

			// Separate unselected layers with their original indices
			let unselected_layers = all_children
				.iter()
				.enumerate()
				.filter_map(|(index, &layer)| if !selected_layer_set.contains(&layer) { Some((index, layer)) } else { None })
				.collect::<Vec<_>>();

			layers.sort_by_key(|(index, _)| *index);

			let reversed_layers = layers.iter().rev().map(|(_, layer)| *layer).collect::<Vec<_>>();
			let selected_positions = layers.iter().map(|(index, _)| *index).collect::<Vec<_>>();
			let selected_iter = reversed_layers.into_iter();
			let mut merged_layers = vec![None; all_children.len()];

			for (&original_index, new_layer) in selected_positions.iter().zip(selected_iter) {
				merged_layers[original_index] = Some(new_layer);
			}

			// Place unselected layers at their original positions
			for (index, layer) in unselected_layers {
				if merged_layers[index].is_none() {
					merged_layers[index] = Some(layer);
				}
			}

			let final_layers = merged_layers.into_iter().flatten().collect::<Vec<_>>();
			if final_layers.is_empty() {
				continue;
			}

			if !modified {
				responses.add(DocumentMessage::AddTransaction);
			}

			for (index, layer) in final_layers.iter().enumerate() {
				responses.add(NodeGraphMessage::MoveLayerToStack {
					layer: *layer,
					parent,
					insert_index: index,
				});
			}

			modified = true;
		}

		if modified {
			responses.add(NodeGraphMessage::RunDocumentGraph);
			responses.add(NodeGraphMessage::SendGraph);
		}
	}

	pub fn selected_layers_reorder(&mut self, relative_index_offset: isize, responses: &mut VecDeque<Message>) {
		let selected_nodes = self.network_interface.selected_nodes();
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

	pub fn graph_view_overlay_open(&self) -> bool {
		self.graph_view_overlay_open
	}

	pub fn garbage_collect_resources(&mut self) {
		let used_resources = self.used_resources(true);
		self.resources.collect_garbage(&used_resources);
	}

	pub fn used_resources(&self, include_history: bool) -> Box<[ResourceId]> {
		let mut resources = HashSet::new();
		self.network_interface.collect_used_resources(&mut resources);
		if include_history {
			self.document_undo_history.iter().for_each(|interface| interface.collect_used_resources(&mut resources));
			self.document_redo_history.iter().for_each(|interface| interface.collect_used_resources(&mut resources));
		}
		resources.into_iter().collect::<Vec<_>>().into_boxed_slice()
	}
}

#[cfg(debug_assertions)]
pub(crate) fn diff_registries(stored: &graph_storage::Registry, target: &graph_storage::Registry) -> String {
	use std::fmt::Write;
	let mut out = String::new();

	let stored_node_ids: std::collections::BTreeSet<_> = stored.node_instances.keys().copied().collect();
	let target_node_ids: std::collections::BTreeSet<_> = target.node_instances.keys().copied().collect();
	let missing_nodes: Vec<_> = target_node_ids.difference(&stored_node_ids).collect();
	let extra_nodes: Vec<_> = stored_node_ids.difference(&target_node_ids).collect();
	let shared_node_diffs: Vec<_> = stored_node_ids
		.intersection(&target_node_ids)
		.filter(|id| !stored.node_instances[id].value_equal(&target.node_instances[id]))
		.collect();

	let stored_network_ids: std::collections::BTreeSet<_> = stored.networks.keys().copied().collect();
	let target_network_ids: std::collections::BTreeSet<_> = target.networks.keys().copied().collect();
	let missing_networks: Vec<_> = target_network_ids.difference(&stored_network_ids).collect();
	let extra_networks: Vec<_> = stored_network_ids.difference(&target_network_ids).collect();
	let shared_network_diffs: Vec<_> = stored_network_ids
		.intersection(&target_network_ids)
		.filter(|id| !stored.networks[id].value_equal(&target.networks[id]))
		.collect();

	let _ = writeln!(out, "  nodes:    stored={} target={}", stored_node_ids.len(), target_node_ids.len());
	if !missing_nodes.is_empty() {
		let _ = writeln!(out, "    missing from stored: {missing_nodes:?}");
	}
	if !extra_nodes.is_empty() {
		let _ = writeln!(out, "    extra in stored:     {extra_nodes:?}");
	}
	if !shared_node_diffs.is_empty() {
		let _ = writeln!(out, "    differing payloads:  {shared_node_diffs:?}");
		for id in &shared_node_diffs {
			let stored_node = &stored.node_instances[id];
			let target_node = &target.node_instances[id];
			let _ = writeln!(out, "      node {id}:");
			diff_node(&mut out, stored_node, target_node);
		}
	}

	let _ = writeln!(out, "  networks: stored={} target={}", stored_network_ids.len(), target_network_ids.len());
	if !missing_networks.is_empty() {
		let _ = writeln!(out, "    missing from stored: {missing_networks:?}");
	}
	if !extra_networks.is_empty() {
		let _ = writeln!(out, "    extra in stored:     {extra_networks:?}");
	}
	if !shared_network_diffs.is_empty() {
		let _ = writeln!(out, "    differing payloads:  {shared_network_diffs:?}");
		for id in &shared_network_diffs {
			let stored_network = &stored.networks[id];
			let target_network = &target.networks[id];
			let _ = writeln!(out, "      network {id}:");
			diff_network(&mut out, stored_network, target_network);
		}
	}

	let stored_resources: std::collections::BTreeSet<_> = stored.resources.keys().copied().collect();
	let target_resources: std::collections::BTreeSet<_> = target.resources.keys().copied().collect();
	let missing_resources: Vec<_> = target_resources.difference(&stored_resources).collect();
	let extra_resources: Vec<_> = stored_resources.difference(&target_resources).collect();
	if !missing_resources.is_empty() || !extra_resources.is_empty() {
		let _ = writeln!(out, "  resources: stored={} target={}", stored_resources.len(), target_resources.len());
		if !missing_resources.is_empty() {
			let _ = writeln!(out, "    missing from stored: {missing_resources:?}");
		}
		if !extra_resources.is_empty() {
			let _ = writeln!(out, "    extra in stored:     {extra_resources:?}");
		}
	}

	if stored.attributes != target.attributes {
		let stored_keys: std::collections::BTreeSet<_> = stored.attributes.keys().collect();
		let target_keys: std::collections::BTreeSet<_> = target.attributes.keys().collect();
		let _ = writeln!(out, "  document attributes differ: stored_keys={stored_keys:?} target_keys={target_keys:?}");
	}

	out
}

#[cfg(debug_assertions)]
fn diff_node(out: &mut String, stored: &graph_storage::Node, target: &graph_storage::Node) {
	use std::fmt::Write;

	if stored.implementation() != target.implementation() {
		let _ = writeln!(out, "        implementation: stored={:?} target={:?}", stored.implementation(), target.implementation());
	}
	if stored.network() != target.network() {
		let _ = writeln!(out, "        network back-pointer: stored={} target={}", stored.network(), target.network());
	}

	let stored_inputs = stored.inputs();
	let target_inputs = target.inputs();
	if stored_inputs.len() != target_inputs.len() {
		let _ = writeln!(out, "        inputs.len: stored={} target={}", stored_inputs.len(), target_inputs.len());
	}
	for (i, (s, t)) in stored_inputs.iter().zip(target_inputs.iter()).enumerate() {
		if s != t {
			let value_differs = s.input != t.input;
			let timestamp_differs = s.timestamp != t.timestamp;
			let _ = writeln!(out, "        input[{i}]: value_differs={value_differs} timestamp_differs={timestamp_differs}");
			if value_differs {
				let _ = writeln!(out, "          stored.value={:?}\n          target.value={:?}", s.input, t.input);
			}
			if s.attributes != t.attributes {
				diff_attributes(out, &format!("        input[{i}].attributes"), &s.attributes, &t.attributes);
			}
		}
	}

	if stored.attributes() != target.attributes() {
		diff_attributes(out, "        attributes", stored.attributes(), target.attributes());
	}
}

#[cfg(debug_assertions)]
fn diff_network(out: &mut String, stored: &graph_storage::Network, target: &graph_storage::Network) {
	use std::fmt::Write;

	if stored.exports.len() != target.exports.len() {
		let _ = writeln!(out, "        exports.len: stored={} target={}", stored.exports.len(), target.exports.len());
	}
	for (i, (s, t)) in stored.exports.iter().zip(target.exports.iter()).enumerate() {
		if s != t {
			let target_differs = s.target != t.target;
			let timestamp_differs = s.timestamp != t.timestamp;
			let _ = writeln!(out, "        export[{i}]: target_differs={target_differs} timestamp_differs={timestamp_differs}");
			if target_differs {
				let _ = writeln!(out, "          stored.target={:?}\n          target.target={:?}", s.target, t.target);
			}
		}
	}

	if stored.attributes != target.attributes {
		diff_attributes(out, "        attributes", &stored.attributes, &target.attributes);
	}
}

#[cfg(debug_assertions)]
fn diff_attributes(out: &mut String, label: &str, stored: &graph_storage::Attributes, target: &graph_storage::Attributes) {
	use std::fmt::Write;

	let stored_keys: std::collections::BTreeSet<_> = stored.keys().collect();
	let target_keys: std::collections::BTreeSet<_> = target.keys().collect();
	let missing: Vec<_> = target_keys.difference(&stored_keys).collect();
	let extra: Vec<_> = stored_keys.difference(&target_keys).collect();
	let differing: Vec<_> = stored_keys.intersection(&target_keys).filter(|k| stored.get(**k) != target.get(**k)).collect();

	let _ = writeln!(out, "{label}: missing_from_stored={missing:?} extra_in_stored={extra:?} differing_values={differing:?}");
}

/// Human-readable summary of how two networks differ (exports, node set, per-node payloads, scope
/// injections). Used by the debug-only `verify_storage_round_trip` and by compare-on-open, which runs
/// in release too, so this is not debug-gated.
pub(crate) fn diff_networks(expected: &graph_craft::document::NodeNetwork, actual: &graph_craft::document::NodeNetwork) -> String {
	use std::fmt::Write;
	let mut out = String::new();

	if expected.exports != actual.exports {
		let _ = writeln!(out, "  exports differ: expected={} actual={}", expected.exports.len(), actual.exports.len());
		for (i, (exp, act)) in expected.exports.iter().zip(actual.exports.iter()).enumerate() {
			if exp != act {
				let _ = writeln!(out, "    [{i}] expected={exp:?}\n        actual=  {act:?}");
			}
		}
	}

	let expected_ids: std::collections::BTreeSet<_> = expected.nodes.keys().copied().collect();
	let actual_ids: std::collections::BTreeSet<_> = actual.nodes.keys().copied().collect();
	let missing: Vec<_> = expected_ids.difference(&actual_ids).collect();
	let extra: Vec<_> = actual_ids.difference(&expected_ids).collect();
	let differing: Vec<_> = expected_ids.intersection(&actual_ids).filter(|id| expected.nodes.get(id) != actual.nodes.get(id)).collect();

	if !missing.is_empty() || !extra.is_empty() || !differing.is_empty() {
		let _ = writeln!(out, "  nodes: expected={} actual={}", expected_ids.len(), actual_ids.len());
		if !missing.is_empty() {
			let _ = writeln!(out, "    missing from actual: {missing:?}");
		}
		if !extra.is_empty() {
			let _ = writeln!(out, "    extra in actual:     {extra:?}");
		}
		if !differing.is_empty() {
			let _ = writeln!(out, "    differing payloads:  {differing:?}");
			for id in &differing {
				if let (Some(exp), Some(act)) = (expected.nodes.get(id), actual.nodes.get(id)) {
					let _ = writeln!(out, "    node {id}:");
					diff_document_node(&mut out, exp, act);
				}
			}
		}
	}

	if expected.scope_injections != actual.scope_injections {
		let _ = writeln!(out, "  scope_injections differ");
	}

	out
}

/// Field-level diff between two runtime `DocumentNode`s with the same ID, so the compare-on-open log
/// names *which* field diverged rather than just the node ID. `original_location` is `#[serde(skip)]`
/// and recomputed at load, so it's a likely culprit for a payload mismatch that doesn't affect behavior.
fn diff_document_node(out: &mut String, expected: &graph_craft::document::DocumentNode, actual: &graph_craft::document::DocumentNode) {
	use std::fmt::Write;

	if expected.inputs != actual.inputs {
		let _ = writeln!(out, "      inputs differ (len expected={} actual={})", expected.inputs.len(), actual.inputs.len());
		for (i, (e, a)) in expected.inputs.iter().zip(actual.inputs.iter()).enumerate() {
			if e != a {
				let _ = writeln!(out, "        input[{i}]: expected={e:?}\n                   actual=  {a:?}");
			}
		}
	}
	if expected.call_argument != actual.call_argument {
		let _ = writeln!(out, "      call_argument: expected={:?} actual={:?}", expected.call_argument, actual.call_argument);
	}
	if expected.implementation != actual.implementation {
		let _ = writeln!(out, "      implementation: expected={:?} actual={:?}", expected.implementation, actual.implementation);
	}
	if expected.visible != actual.visible {
		let _ = writeln!(out, "      visible: expected={} actual={}", expected.visible, actual.visible);
	}
	if expected.skip_deduplication != actual.skip_deduplication {
		let _ = writeln!(out, "      skip_deduplication: expected={} actual={}", expected.skip_deduplication, actual.skip_deduplication);
	}
	if expected.context_features != actual.context_features {
		let _ = writeln!(out, "      context_features: expected={:?} actual={:?}", expected.context_features, actual.context_features);
	}
	if expected.original_location != actual.original_location {
		let _ = writeln!(out, "      original_location differs (recomputed at load, not stored):");
		let _ = writeln!(out, "        expected={:?}\n        actual=  {:?}", expected.original_location, actual.original_location);
	}
}

/// Create a network interface with a single export
fn default_document_network_interface() -> NodeNetworkInterface {
	let mut network_interface = NodeNetworkInterface::default();
	network_interface.add_export(TaggedValue::TypeDefault(descriptor!(graphene_std::list::List<graphene_std::Artboard>)), -1, "", &[]);
	network_interface
}

/// Targets for the [`ClickXRayIter`]. In order to reduce computation, we prefer just a point/path test where possible.
#[derive(Clone)]
enum XRayTarget {
	Point(DVec2),
	Quad(Quad),
	Path(BezPath),
	Polygon(Subpath<PointId>),
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

fn quad_to_kurbo(quad: Quad) -> BezPath {
	BezPath::from_path_segments(quad.all_edges().into_iter().map(|[start, end]| PathSeg::Line(Line::new(dvec2_to_point(start), dvec2_to_point(end)))))
}

fn click_targets_to_kurbo<'a>(click_targets: impl Iterator<Item = &'a ClickTarget>, transform: DAffine2) -> BezPath {
	let segments = click_targets
		.filter_map(|target| {
			if let ClickTargetType::Subpath(subpath) = target.target_type() {
				Some(subpath.iter())
			} else {
				None
			}
		})
		.flatten()
		.map(|bezier| Affine::new(transform.to_cols_array()) * bezier);
	BezPath::from_path_segments(segments)
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
	fn check_layer_area_target(&mut self, click_targets: Option<&[Arc<ClickTarget>]>, clip: bool, layer: LayerNodeIdentifier, path: BezPath, transform: DAffine2) -> XRayResult {
		let get_clip = || path.segments();

		let intersects = click_targets.is_some_and(|targets| targets.iter().any(|target| target.intersect_path(get_clip, transform)));
		let clicked = intersects;
		let mut use_children = !clip || intersects;

		// In the case of a clip path where the area partially intersects, it is necessary to do a boolean operation.
		// We do this on this using the target area to reduce computation (as the target area is usually very simple).
		if clip && intersects {
			let clip_path = click_targets_to_kurbo(click_targets.iter().flat_map(|x| x.iter()).map(|x| x.as_ref()), transform);
			let intersection = boolean_intersect(&path, &clip_path);
			let subtracted = BezPath::from_path_segments(intersection.iter().flat_map(|p| p.segments()));
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
		let selected_layers = self.network_interface.selected_nodes();
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
				let intersects = click_targets.is_some_and(|targets| targets.iter().any(|target| target.intersect_point(*point, transform)));
				XRayResult {
					clicked: intersects,
					use_children: !clip || intersects,
				}
			}
			XRayTarget::Quad(quad) => self.check_layer_area_target(click_targets, clip, layer, quad_to_kurbo(*quad), transform),
			XRayTarget::Path(path) => self.check_layer_area_target(click_targets, clip, layer, path.clone(), transform),
			XRayTarget::Polygon(polygon) => {
				let polygon = BezPath::from_path_segments(polygon.iter_closed());
				self.check_layer_area_target(click_targets, clip, layer, polygon, transform)
			}
		}
	}
}

pub fn navigation_controls(ptz: &PTZ, navigation_handler: &NavigationMessageHandler, node_graph: bool) -> Vec<WidgetInstance> {
	let mut list = vec![
		IconButton::new("ZoomIn", 24)
			.tooltip_label("Zoom In")
			.tooltip_shortcut(action_shortcut!(NavigationMessageDiscriminant::CanvasZoomIncrease))
			.on_update(|_| NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }.into())
			.widget_instance(),
		IconButton::new("ZoomOut", 24)
			.tooltip_label("Zoom Out")
			.tooltip_shortcut(action_shortcut!(NavigationMessageDiscriminant::CanvasZoomDecrease))
			.on_update(|_| NavigationMessage::CanvasZoomDecrease { center_on_mouse: false }.into())
			.widget_instance(),
		IconButton::new("ZoomReset", 24)
			.tooltip_label("Reset Tilt and Zoom to 100%")
			.tooltip_shortcut(action_shortcut!(NavigationMessageDiscriminant::CanvasTiltResetAndZoomTo100Percent))
			.on_update(|_| NavigationMessage::CanvasTiltResetAndZoomTo100Percent.into())
			.disabled(ptz.tilt().abs() < 1e-4 && (ptz.zoom() - 1.).abs() < 1e-4)
			.widget_instance(),
	];
	if ptz.flip && !node_graph {
		list.push(
			IconButton::new("Reverse", 24)
				.tooltip_label("Unflip Canvas")
				.tooltip_description("Flip the canvas back to its standard orientation.")
				.tooltip_shortcut(action_shortcut!(NavigationMessageDiscriminant::CanvasFlip))
				.on_update(|_| NavigationMessage::CanvasFlip.into())
				.widget_instance(),
		);
	}
	list.extend([
		Separator::new(SeparatorStyle::Related).widget_instance(),
		NumberInput::new(Some(navigation_handler.snapped_zoom(ptz.zoom()) * 100.))
			.unit("%")
			.min(0.000001)
			.max(1000000.)
			.tooltip_label(if node_graph { "Node Graph Zoom" } else { "Canvas Zoom" })
			.on_update(|number_input: &NumberInput| {
				NavigationMessage::CanvasZoomSet {
					zoom_factor: number_input.value.unwrap() / 100.,
				}
				.into()
			})
			.increment_behavior(NumberInputIncrementBehavior::Callback)
			.increment_callback_decrease(|_| NavigationMessage::CanvasZoomDecrease { center_on_mouse: false }.into())
			.increment_callback_increase(|_| NavigationMessage::CanvasZoomIncrease { center_on_mouse: false }.into())
			.widget_instance(),
	]);
	list
}

impl Iterator for ClickXRayIter<'_> {
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

/// Deserializes `CollapsedLayers` with backwards compatibility for the old format
/// (flat list of layer node IDs) by consuming the entire value first, then attempting
/// to interpret it as the new format. Falls back to an empty default for old documents.
fn deserialize_collapsed_layers<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<CollapsedLayers, D::Error> {
	use serde::Deserialize;
	// Buffer the entire value to avoid leaving the deserializer in a bad state on type mismatch
	let value = serde_json::Value::deserialize(deserializer)?;
	Ok(serde_json::from_value(value).unwrap_or_default())
}

#[cfg(test)]
mod document_message_handler_tests {
	use super::*;
	use crate::test_utils::test_prelude::*;

	#[tokio::test]
	async fn test_layer_selection_with_shift_and_ctrl() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		// Three rectangle layers
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Rectangle, 50., 50., 150., 150., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Rectangle, 100., 100., 200., 200., ModifierKeys::empty()).await;

		let layers: Vec<_> = editor.active_document().metadata().all_layers().collect();

		// Case 1: Basic selection (no modifier)
		editor
			.handle_message(DocumentMessage::SelectLayer {
				id: layers[0].to_node(),
				ctrl: false,
				shift: false,
			})
			.await;
		// Fresh document reference for verification
		let document = editor.active_document();
		let selected_nodes = document.network_interface.selected_nodes();
		assert_eq!(selected_nodes.selected_nodes_ref().len(), 1);
		assert!(selected_nodes.selected_layers_contains(layers[0], document.metadata()));

		// Case 2: Ctrl + click to add another layer
		editor
			.handle_message(DocumentMessage::SelectLayer {
				id: layers[2].to_node(),
				ctrl: true,
				shift: false,
			})
			.await;
		let document = editor.active_document();
		let selected_nodes = document.network_interface.selected_nodes();
		assert_eq!(selected_nodes.selected_nodes_ref().len(), 2);
		assert!(selected_nodes.selected_layers_contains(layers[0], document.metadata()));
		assert!(selected_nodes.selected_layers_contains(layers[2], document.metadata()));

		// Case 3: Shift + click to select a range
		editor
			.handle_message(DocumentMessage::SelectLayer {
				id: layers[1].to_node(),
				ctrl: false,
				shift: true,
			})
			.await;
		let document = editor.active_document();
		let selected_nodes = document.network_interface.selected_nodes();
		// We expect 2 layers to be selected (layers 1 and 2) - not 3
		assert_eq!(selected_nodes.selected_nodes_ref().len(), 2);
		assert!(!selected_nodes.selected_layers_contains(layers[0], document.metadata()));
		assert!(selected_nodes.selected_layers_contains(layers[1], document.metadata()));
		assert!(selected_nodes.selected_layers_contains(layers[2], document.metadata()));

		// Case 4: Ctrl + click to toggle selection (deselect)
		editor
			.handle_message(DocumentMessage::SelectLayer {
				id: layers[1].to_node(),
				ctrl: true,
				shift: false,
			})
			.await;

		// Final fresh document reference
		let document = editor.active_document();
		let selected_nodes = document.network_interface.selected_nodes();
		assert_eq!(selected_nodes.selected_nodes_ref().len(), 1);
		assert!(!selected_nodes.selected_layers_contains(layers[0], document.metadata()));
		assert!(!selected_nodes.selected_layers_contains(layers[1], document.metadata()));
		assert!(selected_nodes.selected_layers_contains(layers[2], document.metadata()));
	}

	#[tokio::test]
	async fn test_layer_rearrangement() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		// Create three rectangle layers
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Rectangle, 50., 50., 150., 150., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Rectangle, 100., 100., 200., 200., ModifierKeys::empty()).await;

		// Helper function to identify layers by bounds
		async fn get_layer_by_bounds(editor: &mut EditorTestUtils, min_x: f64, min_y: f64) -> Option<LayerNodeIdentifier> {
			let document = editor.active_document();
			for layer in document.metadata().all_layers() {
				if let Some(bbox) = document.metadata().bounding_box_viewport(layer) {
					if (bbox[0].x - min_x).abs() < 1. && (bbox[0].y - min_y).abs() < 1. {
						return Some(layer);
					}
				}
			}
			None
		}

		async fn get_layer_index(editor: &mut EditorTestUtils, layer: LayerNodeIdentifier) -> Option<usize> {
			let document = editor.active_document();
			let parent = layer.parent(document.metadata())?;
			parent.children(document.metadata()).position(|child| child == layer)
		}

		let layer_middle = get_layer_by_bounds(&mut editor, 50., 50.).await.unwrap();
		let layer_top = get_layer_by_bounds(&mut editor, 100., 100.).await.unwrap();

		let initial_index_top = get_layer_index(&mut editor, layer_top).await.unwrap();
		let initial_index_middle = get_layer_index(&mut editor, layer_middle).await.unwrap();

		// Test 1: Lower the top layer
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer_top.to_node()] }).await;
		editor.handle_message(DocumentMessage::SelectedLayersLower).await;
		let new_index_top = get_layer_index(&mut editor, layer_top).await.unwrap();
		assert!(new_index_top > initial_index_top, "Top layer should have moved down");

		// Test 2: Raise the middle layer
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer_middle.to_node()] }).await;
		editor.handle_message(DocumentMessage::SelectedLayersRaise).await;
		let new_index_middle = get_layer_index(&mut editor, layer_middle).await.unwrap();
		assert!(new_index_middle < initial_index_middle, "Middle layer should have moved up");
	}

	#[tokio::test]
	async fn test_move_folder_into_itself_doesnt_crash() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// Creating a parent folder
		editor.handle_message(DocumentMessage::CreateEmptyFolder).await;
		let parent_folder = editor.active_document().metadata().all_layers().next().unwrap();

		// Creating a child folder inside the parent folder
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![parent_folder.to_node()] }).await;
		editor.handle_message(DocumentMessage::CreateEmptyFolder).await;
		let child_folder = editor.active_document().metadata().all_layers().next().unwrap();

		// Attempt to move parent folder into child folder
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![parent_folder.to_node()] }).await;
		editor
			.handle_message(DocumentMessage::MoveSelectedLayersTo {
				parent: child_folder,
				insert_index: 0,
			})
			.await;

		// The operation completed without crashing
		// Verifying application still functions by performing another operation
		editor.handle_message(DocumentMessage::CreateEmptyFolder).await;
		assert!(true, "Application didn't crash after folder move operation");
	}

	// Merging nodes whose output isn't wired downstream produces an encapsulating subnetwork with no exports.
	// Inspecting it via the Data panel (which splices in a monitor node) must not leave a dangling reference that crashes compilation.
	#[tokio::test]
	async fn merge_selected_nodes_while_inspecting_does_not_crash() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 100., 100.).await;

		let node_a = editor.create_node_by_name(DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::transform::IDENTIFIER)).await;
		let node_b = editor.create_node_by_name(DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::transform::IDENTIFIER)).await;
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![node_a, node_b] }).await;
		editor.handle_message(NodeGraphMessage::MergeSelectedNodes).await;

		let merged = editor.active_document().network_interface.selected_nodes_in_nested_network(&[]).unwrap().0.clone();
		assert_eq!(merged.len(), 1, "merge should leave one encapsulating node selected");

		// Simulate the Data panel inspecting the merged node, which compiles the graph with a monitor node spliced in
		let portfolio = &mut editor.editor.dispatcher.message_handlers.portfolio_message_handler;
		let document_id = portfolio.active_document_id.unwrap();
		let document = portfolio.documents.get_mut(&document_id).unwrap();
		portfolio
			.executor
			.submit_node_graph_evaluation(document, document_id, glam::UVec2::ONE, 1., Default::default(), merged, true, DVec2::ZERO)
			.unwrap();
		editor.runtime.run().await;

		let mut messages = VecDeque::new();
		editor.editor.poll_node_graph_evaluation(&mut messages).unwrap();
	}

	#[tokio::test]
	async fn test_moving_folder_with_children() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// Creating two folders at root level
		editor.handle_message(DocumentMessage::CreateEmptyFolder).await;
		editor.handle_message(DocumentMessage::CreateEmptyFolder).await;

		let folder1 = editor.active_document().metadata().all_layers().next().unwrap();
		let folder2 = editor.active_document().metadata().all_layers().nth(1).unwrap();

		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let rect_layer = editor.active_document().metadata().all_layers().next().unwrap();

		// First move rectangle into folder1
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![rect_layer.to_node()] }).await;
		editor.handle_message(DocumentMessage::MoveSelectedLayersTo { parent: folder1, insert_index: 0 }).await;

		// Verifying rectagle is now in folder1
		let rect_parent = rect_layer.parent(editor.active_document().metadata()).unwrap();
		assert_eq!(rect_parent, folder1, "Rectangle should be inside folder1");

		// Moving folder1 into folder2
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![folder1.to_node()] }).await;
		editor.handle_message(DocumentMessage::MoveSelectedLayersTo { parent: folder2, insert_index: 0 }).await;

		// Verifing hirarchy: folder2 > folder1 > rectangle
		let document = editor.active_document();
		let folder1_parent = folder1.parent(document.metadata()).unwrap();
		assert_eq!(folder1_parent, folder2, "Folder1 should be inside folder2");

		// Verifing rectangle moved with its parent
		let rect_parent = rect_layer.parent(document.metadata()).unwrap();
		assert_eq!(rect_parent, folder1, "Rectangle should still be inside folder1");

		let rect_grandparent = rect_parent.parent(document.metadata()).unwrap();
		assert_eq!(rect_grandparent, folder2, "Rectangle's grandparent should be folder2");
	}

	// TODO: Fix https://github.com/GraphiteEditor/Graphite/issues/2688 and reenable this as part of that fix.
	#[ignore]
	#[tokio::test]
	async fn test_moving_layers_retains_transforms() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.handle_message(DocumentMessage::CreateEmptyFolder).await;
		editor.handle_message(DocumentMessage::CreateEmptyFolder).await;

		let folder2 = editor.active_document().metadata().all_layers().next().unwrap();
		let folder1 = editor.active_document().metadata().all_layers().nth(1).unwrap();

		// Applying transform to folder1 (translation)
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![folder1.to_node()] }).await;
		editor.handle_message(TransformLayerMessage::BeginGrab).await;
		editor.move_mouse(100., 50., ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		// Applying different transform to folder2 (translation)
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![folder2.to_node()] }).await;
		editor.handle_message(TransformLayerMessage::BeginGrab).await;
		editor.move_mouse(200., 100., ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		// Creating rectangle in folder1
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![folder1.to_node()] }).await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let rect_layer = editor.active_document().metadata().all_layers().next().unwrap();

		// Moving the rectangle to folder1 to ensure it's inside
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![rect_layer.to_node()] }).await;
		editor.handle_message(DocumentMessage::MoveSelectedLayersTo { parent: folder1, insert_index: 0 }).await;

		editor.handle_message(TransformLayerMessage::BeginGrab).await;
		editor.move_mouse(50., 25., ModifierKeys::empty(), MouseKeys::NONE).await;
		editor
			.handle_message(TransformLayerMessage::PointerMove {
				slow_key: Key::Shift,
				increments_key: Key::Control,
			})
			.await;
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		// Rectangle's viewport position before moving
		let document = editor.active_document();
		let rect_bbox_before = document.metadata().bounding_box_viewport(rect_layer).unwrap();

		// Moving rectangle from folder1 to folder2
		editor.handle_message(DocumentMessage::MoveSelectedLayersTo { parent: folder2, insert_index: 0 }).await;

		// Rectangle's viewport position after moving
		let document = editor.active_document();
		let rect_bbox_after = document.metadata().bounding_box_viewport(rect_layer).unwrap();

		// Verifing the rectangle maintains approximately the same position in viewport space
		let before_center = (rect_bbox_before[0] + rect_bbox_before[1]) / 2.; // TODO: Should be: DVec2(0., -25.), regression (#2688) causes it to be: DVec2(100., 25.)
		let after_center = (rect_bbox_after[0] + rect_bbox_after[1]) / 2.; // TODO:    Should be: DVec2(0., -25.), regression (#2688) causes it to be: DVec2(200., 75.)
		let distance = before_center.distance(after_center); // TODO:                    Should be: 0.,               regression (#2688) causes it to be: 111.80339887498948

		assert!(
			distance < 1.,
			"Rectangle should maintain its viewport position after moving between transformed groups.\n\
			Before: {before_center:?}\n\
			After:  {after_center:?}\n\
			Dist:   {distance} (should be < 1)"
		);
	}
}
