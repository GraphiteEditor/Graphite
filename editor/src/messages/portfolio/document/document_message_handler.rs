use super::utility_types::error::EditorError;
use super::utility_types::misc::{SnappingOptions, SnappingState};
use super::utility_types::nodes::{CollapsedLayers, SelectedNodes};
use crate::application::{generate_uuid, GRAPHITE_GIT_COMMIT_HASH};
use crate::consts::{ASYMPTOTIC_EFFECT, DEFAULT_DOCUMENT_NAME, FILE_SAVE_SUFFIX, SCALE_EFFECT, SCROLLBAR_SPACING};
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::{GraphOperationHandlerData, NodeGraphHandlerData};
use crate::messages::portfolio::document::overlays::grid_overlays::{grid_overlay, overlay_options};
use crate::messages::portfolio::document::properties_panel::utility_types::PropertiesPanelMessageHandlerData;
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
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

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::vec;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentMessageHandler {
	// ======================
	// Child message handlers
	// ======================
	#[serde(skip)]
	node_graph_handler: NodeGraphMessageHandler,
	#[serde(skip)]
	navigation_handler: NavigationMessageHandler,
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

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			// ======================
			// Child message handlers
			// ======================
			node_graph_handler: Default::default(),
			navigation_handler: NavigationMessageHandler::default(),
			overlays_message_handler: OverlaysMessageHandler::default(),
			properties_panel_message_handler: PropertiesPanelMessageHandler,
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
				inputs: vec![NodeId(3), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(3), 0)],
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

pub struct DocumentInputs<'a> {
	pub document_id: DocumentId,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub persistent_data: &'a PersistentData,
	pub executor: &'a mut NodeGraphExecutor,
}

impl MessageHandler<DocumentMessage, DocumentInputs<'_>> for DocumentMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: DocumentMessage, responses: &mut VecDeque<Message>, document_inputs: DocumentInputs) {
		let DocumentInputs {
			document_id,
			ipp,
			persistent_data,
			executor,
		} = document_inputs;
		use DocumentMessage::*;

		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			Navigation(message) => {
				let document_bounds = self.metadata().document_bounds_viewport_space();
				self.navigation_handler.process_message(
					message,
					responses,
					(&self.metadata, document_bounds, ipp, self.selected_visible_layers_bounding_box_viewport(), &mut self.navigation),
				);
			}
			#[remain::unsorted]
			Overlays(message) => {
				self.overlays_message_handler.process_message(message, responses, (self.overlays_visible, ipp));
			}
			#[remain::unsorted]
			PropertiesPanel(message) => {
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
			#[remain::unsorted]
			NodeGraph(message) => {
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
			#[remain::unsorted]
			GraphOperation(message) => GraphOperationMessageHandler.process_message(
				message,
				responses,
				GraphOperationHandlerData {
					document_network: &mut self.network,
					document_metadata: &mut self.metadata,
					selected_nodes: &mut self.selected_nodes,
					collapsed: &mut self.collapsed,
					node_graph: &mut self.node_graph_handler,
				},
			),

			// Messages
			AbortTransaction => {
				if !self.undo_in_progress {
					self.undo(responses);
					responses.add(OverlaysMessage::Draw);
				}
			}
			AlignSelectedLayers { axis, aggregate } => {
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
				for layer in self.selected_nodes.selected_layers(self.metadata()) {
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
			BackupDocument { network } => self.backup_with_document(network, responses),
			ClearLayersPanel => {
				// Send an empty layer list
				let data_buffer: RawBuffer = Self::default().serialize_root();
				responses.add(FrontendMessage::UpdateDocumentLayerStructure { data_buffer });

				// Clear the options bar
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(Default::default()),
					layout_target: LayoutTarget::LayersPanelOptions,
				});
			}
			CommitTransaction => (),
			CreateEmptyFolder { parent } => {
				let id = NodeId(generate_uuid());

				responses.add(GraphOperationMessage::NewCustomLayer {
					id,
					nodes: HashMap::new(),
					parent,
					insert_index: -1,
				});
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });
			}
			DebugPrintDocument => {
				info!("{:#?}", self.network);
			}
			DeleteLayer { id } => {
				responses.add(GraphOperationMessage::DeleteLayer { id });
				responses.add_front(BroadcastEvent::ToolAbort);
			}
			DeleteSelectedLayers => {
				self.backup(responses);

				responses.add_front(BroadcastEvent::SelectionChanged);
				for path in self.metadata().shallowest_unique_layers(self.selected_nodes.selected_layers(self.metadata())) {
					responses.add_front(DocumentMessage::DeleteLayer { id: path.last().unwrap().to_node() });
				}
			}
			DeselectAllLayers => {
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				self.layer_range_selection_reference = None;
			}
			DocumentHistoryBackward => self.undo(responses),
			DocumentHistoryForward => self.redo(responses),
			DocumentStructureChanged => {
				self.update_layers_panel_options_bar_widgets(responses);

				self.metadata.load_structure(&self.network, &mut self.selected_nodes);
				let data_buffer: RawBuffer = self.serialize_root();
				responses.add(FrontendMessage::UpdateDocumentLayerStructure { data_buffer });
			}
			DuplicateSelectedLayers => {
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
							.upstream_flow_back_from_nodes(vec![node], false)
							.enumerate()
							.map(|(index, (_, node_id))| (node_id, NodeId(index as u64)))
							.collect(),
					)
					.collect();

					let id = NodeId(generate_uuid());
					let insert_index = -1;
					responses.add(GraphOperationMessage::NewCustomLayer { id, nodes, parent, insert_index });
				}
			}
			FlipSelectedLayers { flip_axis } => {
				self.backup(responses);
				let scale = match flip_axis {
					FlipAxis::X => DVec2::new(-1., 1.),
					FlipAxis::Y => DVec2::new(1., -1.),
				};
				if let Some([min, max]) = self.selected_visible_layers_bounding_box_viewport() {
					let center = (max + min) / 2.;
					let bbox_trans = DAffine2::from_translation(-center);
					for layer in self.selected_nodes.selected_layers(self.metadata()) {
						responses.add(GraphOperationMessage::TransformChange {
							layer,
							transform: DAffine2::from_scale(scale),
							transform_in: TransformIn::Scope { scope: bbox_trans },
							skip_rerender: false,
						});
					}
				}
			}
			GraphViewOverlay { open } => {
				self.graph_view_overlay_open = open;

				if open {
					responses.add(NodeGraphMessage::SendGraph);
				}
				responses.add(FrontendMessage::TriggerGraphViewOverlay { open });
			}
			GraphViewOverlayToggle => {
				responses.add(DocumentMessage::GraphViewOverlay { open: !self.graph_view_overlay_open });
			}
			GridOptions(grid) => {
				self.snapping_state.grid = grid;
				self.snapping_state.grid_snapping = true;
				responses.add(OverlaysMessage::Draw);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			GridOverlays(mut overlay_context) => {
				if self.snapping_state.grid_snapping {
					grid_overlay(self, &mut overlay_context)
				}
			}
			GridVisible(enabled) => {
				self.snapping_state.grid_snapping = enabled;
				responses.add(OverlaysMessage::Draw);
			}
			GroupSelectedLayers => {
				let parent = self
					.metadata()
					.deepest_common_ancestor(self.selected_nodes.selected_layers(self.metadata()), true)
					.unwrap_or(LayerNodeIdentifier::ROOT);

				let calculated_insert_index = parent.children(self.metadata()).enumerate().find_map(|(index, direct_child)| {
					if self.selected_nodes.selected_layers(self.metadata()).any(|selected| selected == direct_child) {
						return Some(index as isize);
					}

					for descendant in direct_child.decendants(self.metadata()) {
						if self.selected_nodes.selected_layers(self.metadata()).any(|selected| selected == descendant) {
							return Some(index as isize);
						}
					}

					None
				});

				let folder_id = NodeId(generate_uuid());

				responses.add(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
				responses.add(DocumentMessage::DeleteSelectedLayers);

				responses.add(GraphOperationMessage::NewCustomLayer {
					id: folder_id,
					nodes: HashMap::new(),
					parent,
					insert_index: calculated_insert_index.unwrap_or(-1),
				});
				responses.add(PortfolioMessage::PasteIntoFolder {
					clipboard: Clipboard::Internal,
					parent: LayerNodeIdentifier::new_unchecked(folder_id),
					insert_index: -1,
				});
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![folder_id] });
			}
			ImaginateGenerate => responses.add(PortfolioMessage::SubmitGraphRender { document_id }),
			ImaginateRandom { imaginate_node, then_generate } => {
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
			MoveSelectedLayersTo { parent, insert_index } => {
				let selected_layers = self.selected_nodes.selected_layers(self.metadata()).collect::<Vec<_>>();

				// Disallow trying to insert into self
				if selected_layers.iter().any(|&layer| parent.ancestors(self.metadata()).any(|ancestor| ancestor == layer)) {
					return;
				}

				let insert_index = self.update_insert_index(&selected_layers, parent, insert_index);

				responses.add(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
				responses.add(DocumentMessage::DeleteSelectedLayers);
				responses.add(PortfolioMessage::PasteIntoFolder {
					clipboard: Clipboard::Internal,
					parent,
					insert_index,
				});
			}
			NudgeSelectedLayers {
				delta_x,
				delta_y,
				resize,
				resize_opposite_corner,
			} => {
				self.backup(responses);

				let opposite_corner = ipp.keyboard.key(resize_opposite_corner);
				let delta = DVec2::new(delta_x, delta_y);

				for layer in self.selected_nodes.selected_layers(self.metadata()) {
					// Nudge translation
					if !ipp.keyboard.key(resize) {
						responses.add(GraphOperationMessage::TransformChange {
							layer,
							transform: DAffine2::from_translation(delta),
							transform_in: TransformIn::Local,
							skip_rerender: false,
						});
					}
					// Nudge resize
					else if let Some([existing_top_left, existing_bottom_right]) = self.metadata.bounding_box_document(layer) {
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

						let to = self.metadata().document_to_viewport.inverse() * self.metadata().downstream_transform_to_viewport(layer);
						let original_transform = self.metadata().upstream_transform(layer.to_node());
						let new = to.inverse() * transformation * to * original_transform;
						responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform: new,
							transform_in: TransformIn::Local,
							skip_rerender: false,
						});
					};
				}
			}
			PasteImage { image, mouse } => {
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
			PasteSvg { svg, mouse } => {
				use crate::messages::tool::common_functionality::graph_modification_utils;
				let viewport_location = mouse.map_or(ipp.viewport_bounds.center() + ipp.viewport_bounds.top_left, |pos| pos.into());
				let center_in_viewport = DAffine2::from_translation(self.metadata().document_to_viewport.inverse().transform_point2(viewport_location - ipp.viewport_bounds.top_left));
				let layer = graph_modification_utils::new_svg_layer(svg, center_in_viewport, NodeId(generate_uuid()), self.new_layer_parent(), responses);
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
				responses.add(ToolMessage::ActivateTool { tool_type: ToolType::Select });
			}
			Redo => {
				responses.add(SelectToolMessage::Abort);
				responses.add(DocumentMessage::DocumentHistoryForward);
				responses.add(ToolMessage::Redo);
				responses.add(OverlaysMessage::Draw);
			}
			RenameDocument { new_name } => {
				self.name = new_name;
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				responses.add(NodeGraphMessage::UpdateNewNodeGraph);
			}
			RenderRulers => {
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
			RenderScrollbars => {
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
			SaveDocument => {
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
			SelectAllLayers => {
				let metadata = self.metadata();
				let all_layers_except_artboards = metadata.all_layers().filter(move |&layer| !metadata.is_artboard(layer));
				let nodes = all_layers_except_artboards.map(|layer| layer.to_node()).collect();
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
			}
			SelectedLayersLower => {
				responses.add(DocumentMessage::SelectedLayersReorder { relative_index_offset: 1 });
			}
			SelectedLayersLowerToBack => {
				responses.add(DocumentMessage::SelectedLayersReorder { relative_index_offset: isize::MAX });
			}
			SelectedLayersRaise => {
				responses.add(DocumentMessage::SelectedLayersReorder { relative_index_offset: -1 });
			}
			SelectedLayersRaiseToFront => {
				responses.add(DocumentMessage::SelectedLayersReorder { relative_index_offset: isize::MIN });
			}
			SelectedLayersReorder { relative_index_offset } => {
				self.selected_layers_reorder(relative_index_offset, responses);
			}
			SelectLayer { id, ctrl, shift } => {
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
			SetBlendModeForSelectedLayers { blend_mode } => {
				self.backup(responses);
				for layer in self.selected_nodes.selected_layers_except_artboards(self.metadata()) {
					responses.add(GraphOperationMessage::BlendModeSet { layer, blend_mode });
				}
			}
			SetOpacityForSelectedLayers { opacity } => {
				self.backup(responses);
				let opacity = opacity.clamp(0., 1.);

				for layer in self.selected_nodes.selected_layers_except_artboards(self.metadata()) {
					responses.add(GraphOperationMessage::OpacitySet { layer, opacity });
				}
			}
			SetOverlaysVisibility { visible } => {
				self.overlays_visible = visible;
				responses.add(BroadcastEvent::ToolAbort);
				responses.add(OverlaysMessage::Draw);
			}
			SetRangeSelectionLayer { new_layer } => {
				self.layer_range_selection_reference = new_layer;
			}
			SetSnapping {
				snapping_enabled,
				bounding_box_snapping,
				geometry_snapping,
			} => {
				if let Some(state) = snapping_enabled {
					self.snapping_state.snapping_enabled = state
				};
				if let Some(state) = bounding_box_snapping {
					self.snapping_state.bounding_box_snapping = state
				}
				if let Some(state) = geometry_snapping {
					self.snapping_state.geometry_snapping = state
				};
			}
			SetViewMode { view_mode } => {
				self.view_mode = view_mode;
				responses.add_front(NodeGraphMessage::RunDocumentGraph);
			}
			StartTransaction => self.backup(responses),
			ToggleLayerExpansion { id } => {
				let layer = LayerNodeIdentifier::new(id, self.network());
				if self.collapsed.0.contains(&layer) {
					self.collapsed.0.retain(|&collapsed_layer| collapsed_layer != layer);
				} else {
					self.collapsed.0.push(layer);
				}
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			Undo => {
				self.undo_in_progress = true;
				responses.add(ToolMessage::PreUndo);
				responses.add(DocumentMessage::DocumentHistoryBackward);
				responses.add(OverlaysMessage::Draw);
				responses.add(DocumentMessage::UndoFinished);
				responses.add(ToolMessage::Undo);
			}
			UndoFinished => self.undo_in_progress = false,
			UngroupSelectedLayers => {
				responses.add(DocumentMessage::StartTransaction);

				let folder_paths = self.metadata().folders_sorted_by_most_nested(self.selected_nodes.selected_layers(self.metadata()));

				for folder in folder_paths {
					// Select all the children of the folder
					responses.add(NodeGraphMessage::SelectedNodesSet {
						nodes: folder.children(self.metadata()).map(LayerNodeIdentifier::to_node).collect(),
					});

					// Copy them
					responses.add(PortfolioMessage::Copy { clipboard: Clipboard::Internal });

					// Paste them into the folder above
					responses.add(PortfolioMessage::PasteIntoFolder {
						clipboard: Clipboard::Internal,
						parent: folder.parent(self.metadata()).unwrap_or(LayerNodeIdentifier::ROOT),
						insert_index: -1,
					});
					// Delete the parent folder
					responses.add(GraphOperationMessage::DeleteLayer { id: folder.to_node() });
				}
				responses.add(DocumentMessage::CommitTransaction);
			}
			UpdateDocumentTransform { transform } => {
				self.metadata.document_to_viewport = transform;
				responses.add(DocumentMessage::RenderRulers);
				responses.add(DocumentMessage::RenderScrollbars);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			ZoomCanvasTo100Percent => {
				responses.add_front(NavigationMessage::SetCanvasZoom { zoom_factor: 1. });
			}
			ZoomCanvasTo200Percent => {
				responses.add_front(NavigationMessage::SetCanvasZoom { zoom_factor: 2. });
			}
			ZoomCanvasToFitAll => {
				if let Some(bounds) = self.metadata().document_bounds_document_space(true) {
					responses.add(NavigationMessage::SetCanvasTilt { angle_radians: 0. });
					responses.add(NavigationMessage::FitViewportToBounds { bounds, prevent_zoom_past_100: true });
				}
			}
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
			.decendants(&self.metadata)
			.filter(|&layer| self.selected_nodes.layer_visible(layer, self.network(), self.metadata()))
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
			.decendants(&self.metadata)
			.filter(|&layer| self.selected_nodes.layer_visible(layer, self.network(), self.metadata()))
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
			.selected_visible_layers(self.network(), self.metadata())
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

	/// Returns the bounding boxes for all visible layers.
	pub fn bounding_boxes(&self) -> impl Iterator<Item = [DVec2; 2]> + '_ {
		// TODO: Remove this function entirely?
		// self.visible_layers().filter_map(|path| self.document_legacy.viewport_bounding_box(path, font_cache).ok()?)
		std::iter::empty()
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

	pub fn undo(&mut self, responses: &mut VecDeque<Message>) {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);

		let Some(network) = self.document_undo_history.pop_back() else { return };

		responses.add(BroadcastEvent::SelectionChanged);

		let previous_network = std::mem::replace(&mut self.network, network);
		self.document_redo_history.push_back(previous_network);
		if self.document_redo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			self.document_redo_history.pop_front();
		}
	}

	pub fn redo(&mut self, responses: &mut VecDeque<Message>) {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);

		let Some(network) = self.document_redo_history.pop_back() else { return };

		responses.add(BroadcastEvent::SelectionChanged);

		let previous_network = std::mem::replace(&mut self.network, network);
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

	/// When working with an insert index, deleting the layers may cause the insert index to point to a different location (if the layer being deleted was located before the insert index).
	///
	/// This function updates the insert index so that it points to the same place after the specified `layers` are deleted.
	fn update_insert_index(&self, layers: &[LayerNodeIdentifier], parent: LayerNodeIdentifier, insert_index: isize) -> isize {
		let take_amount = if insert_index < 0 { usize::MAX } else { insert_index as usize };
		let layer_ids_above = parent.children(self.metadata()).take(take_amount);
		layer_ids_above.filter(|layer_id| !layers.contains(layer_id)).count() as isize
	}

	pub fn new_layer_parent(&self) -> LayerNodeIdentifier {
		self.metadata()
			.deepest_common_ancestor(self.selected_nodes.selected_layers(self.metadata()), false)
			.unwrap_or_else(|| self.metadata().active_artboard())
	}

	/// Loads layer resources such as creating the blob URLs for the images and loading all of the fonts in the document
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
						MenuListEntry::new(DocumentMode::DesignMode.to_string()).icon(DocumentMode::DesignMode.icon_name()),
						MenuListEntry::new(DocumentMode::SelectMode.to_string())
							.icon(DocumentMode::SelectMode.icon_name())
							.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(330) }.into()),
						MenuListEntry::new(DocumentMode::GuideMode.to_string())
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
			PopoverButton::new("Overlays", "Coming soon").widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			CheckboxInput::new(snapping_state.snapping_enabled)
				.icon("Snapping")
				.tooltip("Snapping")
				.on_update(move |optional_input: &CheckboxInput| {
					let snapping_enabled = optional_input.checked;
					DocumentMessage::SetSnapping {
						snapping_enabled: Some(snapping_enabled),
						bounding_box_snapping: Some(snapping_state.bounding_box_snapping),
						geometry_snapping: Some(snapping_state.geometry_snapping),
					}
					.into()
				})
				.widget_holder(),
			PopoverButton::new("Snapping", "Snap customization settings")
				.options_widget(vec![
					LayoutGroup::Row {
						widgets: vec![
							CheckboxInput::new(snapping_state.bounding_box_snapping)
								.tooltip(SnappingOptions::BoundingBoxes.to_string())
								.on_update(move |input: &CheckboxInput| {
									DocumentMessage::SetSnapping {
										snapping_enabled: None,
										bounding_box_snapping: Some(input.checked),
										geometry_snapping: None,
									}
									.into()
								})
								.widget_holder(),
							TextLabel::new(SnappingOptions::BoundingBoxes.to_string()).widget_holder(),
						],
					},
					LayoutGroup::Row {
						widgets: vec![
							CheckboxInput::new(self.snapping_state.geometry_snapping)
								.tooltip(SnappingOptions::Geometry.to_string())
								.on_update(|input: &CheckboxInput| {
									DocumentMessage::SetSnapping {
										snapping_enabled: None,
										bounding_box_snapping: None,
										geometry_snapping: Some(input.checked),
									}
									.into()
								})
								.widget_holder(),
							TextLabel::new(SnappingOptions::Geometry.to_string()).widget_holder(),
						],
					},
				])
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			CheckboxInput::new(self.snapping_state.grid_snapping)
				.icon("Grid")
				.tooltip("Grid")
				.on_update(|optional_input: &CheckboxInput| DocumentMessage::GridVisible(optional_input.checked).into())
				.widget_holder(),
			PopoverButton::new("Grid", "Grid customization settings")
				.options_widget(overlay_options(&self.snapping_state.grid))
				.popover_min_width(Some(320))
				.widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(vec![
				RadioEntryData::default()
					.value("normal")
					.icon("ViewModeNormal")
					.tooltip("View Mode: Normal")
					.on_update(|_| DocumentMessage::SetViewMode { view_mode: ViewMode::Normal }.into()),
				RadioEntryData::default()
					.value("outline")
					.icon("ViewModeOutline")
					.tooltip("View Mode: Outline")
					.on_update(|_| DocumentMessage::SetViewMode { view_mode: ViewMode::Outline }.into()),
				RadioEntryData::default()
					.value("pixels")
					.icon("ViewModePixels")
					.tooltip("View Mode: Pixels")
					.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(320) }.into()),
			])
			.selected_index(match self.view_mode {
				ViewMode::Normal => Some(0),
				_ => Some(1),
			})
			.widget_holder(),
			PopoverButton::new("View Mode", "Coming soon").widget_holder(),
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
			PopoverButton::new(
				"Canvas Navigation",
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
				.icon(Some(if self.graph_view_overlay_open { "GraphViewOpen".into() } else { "GraphViewClosed".into() }))
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
						MenuListEntry::new(blend_mode.to_string())
							.value(blend_mode.to_string())
							.on_update(move |_| DocumentMessage::SetBlendModeForSelectedLayers { blend_mode }.into())
					})
					.collect()
			})
			.collect();

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
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				IconButton::new("Folder", 24)
					.tooltip("New Folder")
					.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::CreateEmptyFolder))
					.on_update(|_| DocumentMessage::CreateEmptyFolder { parent: LayerNodeIdentifier::ROOT }.into())
					.widget_holder(),
				IconButton::new("Trash", 24)
					.tooltip("Delete Selected")
					.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::DeleteSelectedLayers))
					.on_update(|_| DocumentMessage::DeleteSelectedLayers.into())
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

		responses.add(DocumentMessage::MoveSelectedLayersTo { parent, insert_index });
	}

	pub fn actions_with_graph_open(&self) -> ActionList {
		let mut common = actions!(DocumentMessageDiscriminant;
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
