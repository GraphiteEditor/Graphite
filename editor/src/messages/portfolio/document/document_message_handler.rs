use super::utility_types::error::EditorError;
use super::utility_types::misc::{DocumentRenderMode, SnappingOptions, SnappingState};
use crate::application::generate_uuid;
use crate::consts::{ASYMPTOTIC_EFFECT, DEFAULT_DOCUMENT_NAME, FILE_SAVE_SUFFIX, GRAPHITE_DOCUMENT_VERSION, SCALE_EFFECT, SCROLLBAR_SPACING, VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR};
use crate::messages::frontend::utility_types::ExportBounds;
use crate::messages::frontend::utility_types::FileType;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::NodeGraphHandlerData;
use crate::messages::portfolio::document::properties_panel::utility_types::PropertiesPanelMessageHandlerData;
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::portfolio::document::utility_types::layer_panel::{LayerMetadata, LayerPanelEntry, RawBuffer};
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, DocumentMode, DocumentSave, FlipAxis};
use crate::messages::portfolio::document::utility_types::vectorize_layer_metadata;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::ToolType;
use crate::node_graph_executor::NodeGraphExecutor;

use document_legacy::document::Document as DocumentLegacy;
use document_legacy::document_metadata::LayerNodeIdentifier;
use document_legacy::layers::layer_info::{LayerDataType, LayerDataTypeDiscriminant};
use document_legacy::layers::style::{RenderData, ViewMode};
use document_legacy::{DocumentError, DocumentResponse, LayerId, Operation as DocumentOperation};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeInput, NodeNetwork};
use graphene_core::raster::BlendMode;
use graphene_core::raster::ImageFrame;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentMessageHandler {
	pub document_legacy: DocumentLegacy,
	pub saved_document_identifier: u64,
	pub auto_saved_document_identifier: u64,
	pub name: String,
	pub version: String,

	pub document_mode: DocumentMode,
	pub view_mode: ViewMode,
	#[serde(skip)]
	pub snapping_state: SnappingState,
	pub overlays_visible: bool,

	#[serde(skip)]
	pub document_undo_history: VecDeque<DocumentSave>,
	#[serde(skip)]
	pub document_redo_history: VecDeque<DocumentSave>,
	/// Don't allow aborting transactions whilst undoing to avoid #559
	#[serde(skip)]
	undo_in_progress: bool,

	#[serde(with = "vectorize_layer_metadata")]
	pub layer_metadata: HashMap<Vec<LayerId>, LayerMetadata>,
	layer_range_selection_reference: Option<LayerNodeIdentifier>,

	navigation_handler: NavigationMessageHandler,
	#[serde(skip)]
	overlays_message_handler: OverlaysMessageHandler,
	properties_panel_message_handler: PropertiesPanelMessageHandler,
	#[serde(skip)]
	node_graph_handler: NodeGraphMessageHandler,
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		let document_legacy = DocumentLegacy {
			commit_hash: crate::application::GRAPHITE_GIT_COMMIT_HASH.to_string(),
			..Default::default()
		};
		Self {
			document_legacy,
			saved_document_identifier: 0,
			auto_saved_document_identifier: 0,
			name: String::from("Untitled Document"),
			version: GRAPHITE_DOCUMENT_VERSION.to_string(),

			document_mode: DocumentMode::DesignMode,
			view_mode: ViewMode::default(),
			snapping_state: SnappingState::default(),
			overlays_visible: true,

			document_undo_history: VecDeque::new(),
			document_redo_history: VecDeque::new(),
			undo_in_progress: false,

			layer_metadata: vec![(vec![], LayerMetadata::new(true))].into_iter().collect(),
			layer_range_selection_reference: None,

			navigation_handler: NavigationMessageHandler::default(),
			overlays_message_handler: OverlaysMessageHandler::default(),
			properties_panel_message_handler: PropertiesPanelMessageHandler::default(),
			node_graph_handler: Default::default(),
		}
	}
}

pub struct DocumentInputs<'a> {
	pub document_id: u64,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub persistent_data: &'a PersistentData,
	pub preferences: &'a PreferencesMessageHandler,
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
			..
		} = document_inputs;
		use DocumentMessage::*;

		let render_data = RenderData::new(&persistent_data.font_cache, self.view_mode, Some(ipp.document_bounds()));

		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			DispatchOperation(op) => {
				match self.document_legacy.handle_operation(*op) {
					Ok(Some(document_responses)) => {
						for response in document_responses {
							match &response {
								DocumentResponse::FolderChanged { path } => responses.add(FolderChanged { affected_folder_path: path.clone() }),
								DocumentResponse::DeletedLayer { path } => {
									self.layer_metadata.remove(path);
								}
								DocumentResponse::LayerChanged { path } => responses.add(LayerChanged { affected_layer_path: path.clone() }),
								DocumentResponse::CreatedLayer { .. } => {
									unimplemented!("We should no longer be creating layers in the document and should instead be using the node graph.")
								}
								DocumentResponse::DocumentChanged => responses.add(RenderDocument),
								DocumentResponse::DeletedSelectedManipulatorPoints => {
									// Clear Properties panel after deleting all points by updating backend widget state.
									responses.add(LayoutMessage::SendLayout {
										layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
										layout_target: LayoutTarget::PropertiesOptions,
									});
									responses.add(LayoutMessage::SendLayout {
										layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
										layout_target: LayoutTarget::PropertiesSections,
									});
								}
							};
							responses.add(BroadcastEvent::DocumentIsDirty);
						}
					}
					Err(e) => error!("DocumentError: {e:?}"),
					Ok(_) => (),
				}
			}
			#[remain::unsorted]
			Navigation(message) => {
				let document_bounds = self.metadata().document_bounds();
				self.navigation_handler.process_message(
					message,
					responses,
					(&self.document_legacy, document_bounds, ipp, self.metadata().selected_visible_layers_bounding_box_viewport()),
				);
			}
			#[remain::unsorted]
			Overlays(message) => {
				self.overlays_message_handler.process_message(message, responses, (self.overlays_visible, persistent_data, ipp));
			}
			#[remain::unsorted]
			PropertiesPanel(message) => {
				let properties_panel_message_handler_data = PropertiesPanelMessageHandlerData {
					document_name: self.name.as_str(),
					artwork_document: &self.document_legacy,
					selected_layers: &mut self.layer_metadata.iter().filter_map(|(path, data)| data.selected.then_some(path.as_slice())),
					node_graph_message_handler: &self.node_graph_handler,
					executor,
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
						document: &mut self.document_legacy,
						document_id,
						document_name: self.name.as_str(),
						input: ipp,
					},
				);
			}
			#[remain::unsorted]
			GraphOperation(message) => GraphOperationMessageHandler.process_message(message, responses, (&mut self.document_legacy, &mut self.node_graph_handler)),

			// Messages
			AbortTransaction => {
				if !self.undo_in_progress {
					self.undo(responses).unwrap_or_else(|e| warn!("{e}"));
					responses.extend([RenderDocument.into(), DocumentStructureChanged.into()]);
				}
			}
			AddSelectedLayers { additional_layers } => {
				for layer_path in &additional_layers {
					responses.extend(self.select_layer(layer_path, &render_data));
				}

				// TODO: Correctly update layer panel in clear_selection instead of here
				responses.add(FolderChanged { affected_folder_path: vec![] });
				responses.add(BroadcastEvent::SelectionChanged);

				self.update_layer_tree_options_bar_widgets(responses, &render_data);
			}
			AlignSelectedLayers { axis, aggregate } => {
				self.backup(responses);

				let axis = match axis {
					AlignAxis::X => DVec2::X,
					AlignAxis::Y => DVec2::Y,
				};
				let Some(combined_box) = self.metadata().selected_visible_layers_bounding_box_viewport() else {
					return;
				};

				let aggregated = match aggregate {
					AlignAggregate::Min => combined_box[0],
					AlignAggregate::Max => combined_box[1],
					AlignAggregate::Center => (combined_box[0] + combined_box[1]) / 2.,
				};
				for layer in self.metadata().selected_layers() {
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
						layer: layer.to_path(),
						transform: DAffine2::from_translation(translation),
						transform_in: TransformIn::Viewport,
						skip_rerender: false,
					});
				}
				responses.add(BroadcastEvent::DocumentIsDirty);
			}
			BackupDocument { document, layer_metadata } => self.backup_with_document(document, layer_metadata, responses),
			ClearLayerTree => {
				// Send an empty layer tree
				let data_buffer: RawBuffer = Self::default().serialize_root().as_slice().into();
				responses.add(FrontendMessage::UpdateDocumentLayerTreeStructure { data_buffer });

				// Clear the options bar
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(Default::default()),
					layout_target: LayoutTarget::LayerTreeOptions,
				});
			}
			CommitTransaction => (),
			CopyToClipboardLayerImageOutput { layer_path } => {
				let layer = self.document_legacy.layer(&layer_path).ok();

				let blob_url = layer.and_then(|layer| layer.as_layer().ok()).and_then(|layer_layer| layer_layer.as_blob_url()).cloned();

				if let Some(blob_url) = blob_url {
					responses.add(FrontendMessage::TriggerCopyToClipboardBlobUrl { blob_url });
				}
			}
			CreateEmptyFolder { parent } => {
				let id = generate_uuid();

				responses.add(DocumentMessage::DeselectAllLayers);
				responses.add(GraphOperationMessage::NewCustomLayer {
					id,
					nodes: HashMap::new(),
					parent,
					insert_index: -1,
				});
			}
			DebugPrintDocument => {
				info!("{:#?}\n{:#?}", self.document_legacy, self.layer_metadata);
			}
			DeleteLayer { layer_path } => {
				responses.add_front(DocumentOperation::DeleteLayer { path: layer_path.clone() });
				responses.add(GraphOperationMessage::DeleteLayer { id: layer_path[0] });
				responses.add_front(BroadcastEvent::ToolAbort);
				responses.add(PropertiesPanelMessage::CheckSelectedWasDeleted { path: layer_path });
			}
			DeleteSelectedLayers => {
				self.backup(responses);

				responses.add_front(BroadcastEvent::SelectionChanged);
				for path in self.metadata().shallowest_unique_layers(self.metadata().selected_layers()) {
					responses.add_front(DocumentMessage::DeleteLayer {
						layer_path: path.last().unwrap().to_path(),
					});
				}

				responses.add(BroadcastEvent::DocumentIsDirty);
			}
			DeselectAllLayers => {
				responses.add_front(SetSelectedLayers { replacement_selected_layers: vec![] });
				self.layer_range_selection_reference = None;
			}
			DirtyRenderDocument => {
				// Mark all non-overlay caches as dirty
				DocumentLegacy::mark_children_as_dirty(&mut self.document_legacy.root);
				responses.add(DocumentMessage::RenderDocument);
			}
			DirtyRenderDocumentInOutlineView => {
				if self.view_mode == ViewMode::Outline {
					responses.add_front(DocumentMessage::DirtyRenderDocument);
				}
			}
			DocumentHistoryBackward => self.undo(responses).unwrap_or_else(|e| warn!("{e}")),
			DocumentHistoryForward => self.redo(responses).unwrap_or_else(|e| warn!("{e}")),
			DocumentStructureChanged => {
				let data_buffer: RawBuffer = self.serialize_root().as_slice().into();
				responses.add(FrontendMessage::UpdateDocumentLayerTreeStructure { data_buffer })
			}
			DownloadLayerImageOutput { layer_path } => {
				let layer = self.document_legacy.layer(&layer_path).ok();

				let layer_name = layer.map(|layer| layer.name.clone().unwrap_or_else(|| "Untitled Layer".to_string()));

				let blob_url = layer.and_then(|layer| layer.as_layer().ok()).and_then(|layer_layer| layer_layer.as_blob_url()).cloned();

				if let (Some(layer_name), Some(blob_url)) = (layer_name, blob_url) {
					responses.add(FrontendMessage::TriggerDownloadBlobUrl { layer_name, blob_url });
				}
			}
			DuplicateSelectedLayers => {
				self.backup(responses);
				responses.add_front(SetSelectedLayers { replacement_selected_layers: vec![] });
				self.layer_range_selection_reference = None;
				for path in self.selected_layers_sorted() {
					responses.add(DocumentOperation::DuplicateLayer { path: path.to_vec() });
				}
			}
			ExportDocument {
				file_name,
				file_type,
				scale_factor,
				bounds,
				transparent_background,
			} => {
				let old_artwork_transform = self.remove_document_transform();

				// Calculate the bounding box of the region to be exported
				let bounds = match bounds {
					ExportBounds::AllArtwork => self.all_layer_bounds(&render_data),
					ExportBounds::Selection => self.metadata().selected_visible_layers_bounding_box_viewport(),
					ExportBounds::Artboard(id) => self.metadata().bounding_box_document(id),
				}
				.unwrap_or_default();
				let size = bounds[1] - bounds[0];
				let transform = (DAffine2::from_translation(bounds[0]) * DAffine2::from_scale(size)).inverse();

				let document = self.render_document(size, transform, transparent_background, persistent_data, DocumentRenderMode::Root);

				self.restore_document_transform(old_artwork_transform);

				let file_suffix = &format!(".{file_type:?}").to_lowercase();
				let name = match file_name.ends_with(FILE_SAVE_SUFFIX) {
					true => file_name.replace(FILE_SAVE_SUFFIX, file_suffix),
					false => file_name + file_suffix,
				};

				if file_type == FileType::Svg {
					responses.add(FrontendMessage::TriggerDownloadTextFile { document, name });
				} else {
					let mime = file_type.to_mime().to_string();
					let size = (size * scale_factor).into();
					responses.add(FrontendMessage::TriggerDownloadRaster { svg: document, name, mime, size });
				}
			}
			FlipSelectedLayers { flip_axis } => {
				self.backup(responses);
				let scale = match flip_axis {
					FlipAxis::X => DVec2::new(-1., 1.),
					FlipAxis::Y => DVec2::new(1., -1.),
				};
				if let Some([min, max]) = self.metadata().selected_visible_layers_bounding_box_viewport() {
					let center = (max + min) / 2.;
					let bbox_trans = DAffine2::from_translation(-center);
					for layer in self.metadata().selected_layers() {
						responses.add(GraphOperationMessage::TransformChange {
							layer: layer.to_path(),
							transform: DAffine2::from_scale(scale),
							transform_in: TransformIn::Scope { scope: bbox_trans },
							skip_rerender: false,
						});
					}
					responses.add(BroadcastEvent::DocumentIsDirty);
				}
			}
			FolderChanged { affected_folder_path } => {
				let affected_layer_path = affected_folder_path;
				responses.extend([LayerChanged { affected_layer_path }.into(), DocumentStructureChanged.into()]);
			}
			FrameClear => {
				let mut selected_frame_layers = self.selected_layers_with_type(LayerDataTypeDiscriminant::Layer);
				// Get what is hopefully the only selected Layer layer
				let layer_path = selected_frame_layers.next();
				// Abort if we didn't have any Layer layer, or if there are additional ones also selected
				if layer_path.is_none() || selected_frame_layers.next().is_some() {
					return;
				}
				let layer_path = layer_path.unwrap();

				let layer = self.document_legacy.layer(layer_path).expect("Clearing Layer image for invalid layer");
				let previous_blob_url = match &layer.data {
					LayerDataType::Layer(layer) => layer.as_blob_url(),
					x => panic!("Cannot find blob url for layer type {}", LayerDataTypeDiscriminant::from(x)),
				};

				if let Some(url) = previous_blob_url {
					responses.add(FrontendMessage::TriggerRevokeBlobUrl { url: url.clone() });
				}
				responses.add(DocumentOperation::ClearBlobURL { path: layer_path.into() });
			}
			GroupSelectedLayers => {
				// TODO: Add code that changes the insert index of the new folder based on the selected layer
				let parent = self.metadata().deepest_common_ancestor(self.metadata().selected_layers()).unwrap_or(LayerNodeIdentifier::ROOT);

				let folder_id = generate_uuid();

				responses.add(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
				responses.add(DocumentMessage::DeleteSelectedLayers);

				responses.add(GraphOperationMessage::NewCustomLayer {
					id: folder_id,
					nodes: HashMap::new(),
					parent,
					insert_index: -1,
				});
				responses.add(PortfolioMessage::PasteIntoFolder {
					clipboard: Clipboard::Internal,
					parent: LayerNodeIdentifier::new_unchecked(folder_id),
					insert_index: -1,
				});

				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![folder_id] });
			}
			ImaginateClear { layer_path } => responses.add(InputFrameRasterizeRegionBelowLayer { layer_path }),
			ImaginateGenerate { layer_path } => responses.add(PortfolioMessage::SubmitGraphRender { document_id, layer_path }),
			ImaginateRandom {
				layer_path,
				imaginate_node,
				then_generate,
			} => {
				// Generate a random seed. We only want values between -2^53 and 2^53, because integer values
				// outside of this range can get rounded in f64
				let random_bits = generate_uuid();
				let random_value = ((random_bits >> 11) as f64).copysign(f64::from_bits(random_bits & (1 << 63)));
				// Set a random seed input
				responses.add(NodeGraphMessage::SetInputValue {
					node_id: *imaginate_node.last().unwrap(),
					// Needs to match the index of the seed parameter in `pub const IMAGINATE_NODE: DocumentNodeBlueprint` in `document_node_type.rs`
					input_index: 3,
					value: graph_craft::document::value::TaggedValue::F64(random_value),
				});

				// Generate the image
				if then_generate {
					responses.add(DocumentMessage::ImaginateGenerate { layer_path });
				}
			}
			InputFrameRasterizeRegionBelowLayer { layer_path } => responses.add(PortfolioMessage::SubmitGraphRender { document_id, layer_path }),
			LayerChanged { affected_layer_path } => {
				if let Ok(layer_entry) = self.layer_panel_entry(affected_layer_path.clone(), &render_data) {
					responses.add(FrontendMessage::UpdateDocumentLayerDetails { data: layer_entry });
				}
				responses.add(PropertiesPanelMessage::CheckSelectedWasUpdated { path: affected_layer_path });
				self.update_layer_tree_options_bar_widgets(responses, &render_data);
			}
			MoveSelectedLayersTo { parent, insert_index } => {
				let selected_layers = self.metadata().selected_layers().collect::<Vec<_>>();

				// Disallow trying to insert into self
				if selected_layers.iter().any(|&layer| parent.ancestors(self.metadata()).any(|ancestor| ancestor == layer)) {
					return;
				}

				let insert_index = self.update_insert_index(&selected_layers, parent, insert_index).unwrap();

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
				let sign = if opposite_corner { -1. } else { 1. };

				for path in self.selected_layers().map(|path| path.to_vec()) {
					// Nudge translation
					let transform = if !ipp.keyboard.key(resize) {
						Some(DAffine2::from_translation((delta_x, delta_y).into()))
					}
					// Nudge resize
					else {
						self.document_legacy
							.viewport_bounding_box(&path, &render_data)
							.ok()
							.flatten()
							.map(|[existing_top_left, existing_bottom_right]| {
								let width = existing_bottom_right.x - existing_top_left.x;
								let height = existing_bottom_right.y - existing_top_left.y;

								let new_width = (width + delta_x * sign).max(1.);
								let new_height = (height + delta_y * sign).max(1.);

								let offset = DAffine2::from_translation(if opposite_corner { -existing_bottom_right } else { -existing_top_left });
								let scale = DAffine2::from_scale((new_width / width, new_height / height).into());

								offset.inverse() * scale * offset
							})
					};

					if let Some(transform) = transform {
						let transform_in = TransformIn::Local;
						responses.add(GraphOperationMessage::TransformChange {
							layer: path,
							transform,
							transform_in,
							skip_rerender: false,
						});
					}
				}
				responses.add(BroadcastEvent::DocumentIsDirty);
			}
			PasteImage { image, mouse } => {
				// All the image's pixels have been converted to 0..=1, linear, and premultiplied by `Color::from_rgba8_srgb`

				let image_size = DVec2::new(image.width as f64, image.height as f64);

				// Align the layer with the mouse or center of viewport
				let viewport_location = mouse.map_or(ipp.viewport_bounds.center(), |pos| pos.into());
				let center_in_viewport = DAffine2::from_translation(viewport_location - ipp.viewport_bounds.top_left);
				let center_in_viewport_layerspace = center_in_viewport;

				// Scale the image to fit into a 512x512 box
				let image_size = image_size / DVec2::splat((image_size.max_element() / 512.).max(1.));

				// Make layer the size of the image
				let fit_image_size = DAffine2::from_scale_angle_translation(image_size, 0., image_size / -2.);

				let transform = center_in_viewport_layerspace * fit_image_size;

				responses.add(DocumentMessage::StartTransaction);

				let image_frame = ImageFrame { image, transform: DAffine2::IDENTITY };

				use crate::messages::tool::common_functionality::graph_modification_utils;
				let layer = graph_modification_utils::new_image_layer(image_frame, generate_uuid(), self.new_layer_parent(), responses);

				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });

				responses.add(GraphOperationMessage::TransformSet {
					layer: layer.to_path(),
					transform,
					transform_in: TransformIn::Local,
					skip_rerender: false,
				});

				// Force chosen tool to be Select Tool after importing image.
				responses.add(ToolMessage::ActivateTool { tool_type: ToolType::Select });
			}
			Redo => {
				responses.add(SelectToolMessage::Abort);
				responses.add(DocumentHistoryForward);
				responses.add(BroadcastEvent::DocumentIsDirty);
				responses.add(RenderDocument);
				responses.add(FolderChanged { affected_folder_path: vec![] });
			}
			RenameDocument { new_name } => {
				self.name = new_name;
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				responses.add(NodeGraphMessage::UpdateNewNodeGraph);
			}
			RenameLayer { layer_path, new_name } => responses.add(DocumentOperation::RenameLayer { layer_path, new_name }),
			RenderDocument => {
				responses.add(FrontendMessage::UpdateDocumentArtwork {
					svg: self.document_legacy.render_root(&render_data),
				});
			}
			RenderRulers => {
				let document_transform_scale = self.navigation_handler.snapped_scale();

				let ruler_origin = self.metadata().document_to_viewport.transform_point2(DVec2::ZERO);
				let log = document_transform_scale.log2();
				let ruler_interval = if log < 0. { 100. * 2_f64.powf(-log.ceil()) } else { 100. / 2_f64.powf(log.ceil()) };
				let ruler_spacing = ruler_interval * document_transform_scale;

				responses.add(FrontendMessage::UpdateDocumentRulers {
					origin: ruler_origin.into(),
					spacing: ruler_spacing,
					interval: ruler_interval,
				});
			}
			RenderScrollbars => {
				let document_transform_scale = self.navigation_handler.snapped_scale();

				let scale = 0.5 + ASYMPTOTIC_EFFECT + document_transform_scale * SCALE_EFFECT;

				let viewport_size = ipp.viewport_bounds.size();
				let viewport_mid = ipp.viewport_bounds.center();
				let [bounds1, bounds2] = self.metadata().document_bounds().unwrap_or([viewport_mid; 2]);
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
			RollbackTransaction => {
				self.rollback(responses).unwrap_or_else(|e| warn!("{e}"));
				responses.extend([RenderDocument.into(), DocumentStructureChanged.into()]);
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
				let all = self.all_layers().map(|path| path.to_vec()).collect();
				responses.add_front(SetSelectedLayers { replacement_selected_layers: all });
			}
			SelectedLayersLower => {
				responses.add_front(DocumentMessage::SelectedLayersReorder { relative_index_offset: -1 });
			}
			SelectedLayersLowerToBack => {
				responses.add_front(DocumentMessage::SelectedLayersReorder { relative_index_offset: isize::MIN });
			}
			SelectedLayersRaise => {
				responses.add_front(DocumentMessage::SelectedLayersReorder { relative_index_offset: 1 });
			}
			SelectedLayersRaiseToFront => {
				responses.add_front(DocumentMessage::SelectedLayersReorder { relative_index_offset: isize::MAX });
			}
			SelectedLayersReorder { relative_index_offset } => {
				self.selected_layers_reorder(relative_index_offset, responses);
			}
			SelectLayer { layer_path, ctrl, shift } => {
				let clicked_node = *layer_path.last().expect("Cannot select root");
				let layer = LayerNodeIdentifier::new(clicked_node, self.network());

				let mut nodes = vec![];

				// If we have shift pressed and a layer already selected then fill the range
				if let Some(last_selected) = self.layer_range_selection_reference.filter(|_| shift) {
					nodes.push(last_selected.to_node());
					nodes.push(clicked_node);

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
						if self.metadata().selected_layers_contains(layer) {
							responses.add_front(NodeGraphMessage::SelectedNodesRemove { nodes: vec![clicked_node] });
						} else {
							responses.add_front(NodeGraphMessage::SelectedNodesAdd { nodes: vec![clicked_node] });
						}
						responses.add(BroadcastEvent::SelectionChanged);
					} else {
						nodes.push(clicked_node);
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
				for path in self.selected_layers() {
					responses.add(DocumentOperation::SetLayerBlendMode { path: path.to_vec(), blend_mode });
				}
			}
			SetImageBlobUrl {
				layer_path,
				blob_url,
				resolution,
				document_id,
			} => {
				let Ok(layer) = self.document_legacy.layer(&layer_path) else {
					warn!("Setting blob URL for invalid layer");
					return;
				};

				// Revoke the old blob URL
				match &layer.data {
					LayerDataType::Layer(layer) => {
						if let Some(url) = layer.as_blob_url() {
							responses.add(FrontendMessage::TriggerRevokeBlobUrl { url: url.clone() });
						}
					}
					other => {
						warn!("Setting blob URL for invalid layer type, which must be a `Layer` layer type. Found: `{other:?}`");
						return;
					}
				}

				responses.add(PortfolioMessage::DocumentPassMessage {
					document_id,
					message: DocumentOperation::SetLayerBlobUrl { layer_path, blob_url, resolution }.into(),
				});
			}
			SetLayerExpansion { layer_path, set_expanded } => {
				self.layer_metadata_mut(&layer_path).expanded = set_expanded;
				responses.add(DocumentStructureChanged);
				responses.add(LayerChanged { affected_layer_path: layer_path })
			}
			SetLayerName { layer_path, name } => {
				if let Some(layer) = self.layer_panel_entry_from_path(&layer_path, &render_data) {
					// Only save the history state if the name actually changed to something different
					if layer.name != name {
						self.backup(responses);
						responses.add(DocumentOperation::SetLayerName { path: layer_path, name });
					}
				}
			}
			SetOpacityForSelectedLayers { opacity } => {
				self.backup(responses);
				let opacity = opacity.clamp(0., 1.);

				for path in self.selected_layers().map(|path| path.to_vec()) {
					responses.add(DocumentOperation::SetLayerOpacity { path, opacity });
				}
			}
			SetOverlaysVisibility { visible } => {
				self.overlays_visible = visible;
				responses.add(BroadcastEvent::ToolAbort);
				responses.add(OverlaysMessage::ClearAllOverlays);
				responses.add(OverlaysMessage::Rerender);
			}
			SetRangeSelectionLayer { new_layer } => {
				self.layer_range_selection_reference = new_layer;
			}
			SetSelectedLayers { replacement_selected_layers } => {
				let selected = self.layer_metadata.iter_mut().filter(|(_, layer_metadata)| layer_metadata.selected);
				selected.for_each(|(path, layer_metadata)| {
					layer_metadata.selected = false;
					responses.add(LayerChanged { affected_layer_path: path.clone() })
				});

				let additional_layers = replacement_selected_layers;
				responses.add_front(AddSelectedLayers { additional_layers });
			}
			SetSnapping {
				snapping_enabled,
				bounding_box_snapping,
				node_snapping,
			} => {
				if let Some(state) = snapping_enabled {
					self.snapping_state.snapping_enabled = state
				};
				if let Some(state) = bounding_box_snapping {
					self.snapping_state.bounding_box_snapping = state
				}
				if let Some(state) = node_snapping {
					self.snapping_state.node_snapping = state
				};
			}
			SetViewMode { view_mode } => {
				self.view_mode = view_mode;
				responses.add_front(DocumentMessage::DirtyRenderDocument);
			}
			StartTransaction => self.backup(responses),
			ToggleLayerExpansion { layer_path } => {
				self.layer_metadata_mut(&layer_path).expanded ^= true;
				responses.add(DocumentStructureChanged);
				responses.add(LayerChanged { affected_layer_path: layer_path })
			}
			ToggleLayerVisibility { layer_path } => {
				if let Ok(layer) = self.document_legacy.layer(&layer_path) {
					let visible = layer.visible;
					responses.add(DocumentOperation::SetLayerVisibility { path: layer_path, visible: !visible });
					responses.add(BroadcastEvent::DocumentIsDirty);
				}
			}
			Undo => {
				self.undo_in_progress = true;
				responses.add(BroadcastEvent::ToolAbort);
				responses.add(DocumentHistoryBackward);
				responses.add(BroadcastEvent::DocumentIsDirty);
				responses.add(RenderDocument);
				responses.add(FolderChanged { affected_folder_path: vec![] });
				responses.add(UndoFinished);
			}
			UndoFinished => self.undo_in_progress = false,
			UngroupSelectedLayers => {
				responses.add(DocumentMessage::StartTransaction);

				let folder_paths = self.metadata().folders_sorted_by_most_nested(self.metadata().selected_layers());

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
				self.document_legacy.metadata.document_to_viewport = transform;
				responses.add(DocumentMessage::RenderRulers);
				responses.add(DocumentMessage::RenderScrollbars);
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			UpdateLayerMetadata { layer_path, layer_metadata } => {
				self.layer_metadata.insert(layer_path, layer_metadata);
			}
			ZoomCanvasTo100Percent => {
				responses.add_front(NavigationMessage::SetCanvasZoom { zoom_factor: 1. });
			}
			ZoomCanvasTo200Percent => {
				responses.add_front(NavigationMessage::SetCanvasZoom { zoom_factor: 2. });
			}
			ZoomCanvasToFitAll => {
				if let Some(bounds) = self.metadata().document_bounds() {
					responses.add(NavigationMessage::FitViewportToBounds {
						bounds,
						padding_scale_factor: Some(VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR),
						prevent_zoom_past_100: true,
					})
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		unimplemented!("Must use `actions_with_graph_open` instead (unless we change every implementation of the MessageHandler trait).")
	}
}

impl DocumentMessageHandler {
	pub fn actions_with_graph_open(&self, graph_open: bool) -> ActionList {
		let mut common = actions!(DocumentMessageDiscriminant;
			Undo,
			Redo,
			SelectAllLayers,
			DeselectAllLayers,
			RenderDocument,
			ExportDocument,
			SaveDocument,
			SetSnapping,
			DebugPrintDocument,
			ZoomCanvasToFitAll,
			ZoomCanvasTo100Percent,
			ZoomCanvasTo200Percent,
			CreateEmptyFolder,
		);

		if self.metadata().selected_layers().next().is_some() {
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
		common.extend(self.node_graph_handler.actions_with_node_graph_open(graph_open));
		common
	}
}

impl DocumentMessageHandler {
	pub fn network(&self) -> &NodeNetwork {
		&self.document_legacy.document_network
	}
	pub fn metadata(&self) -> &document_legacy::document_metadata::DocumentMetadata {
		&self.document_legacy.metadata
	}

	/// Remove the artwork and artboard pan/tilt/zoom to render it without the user's viewport navigation, and save it to be restored at the end
	pub(crate) fn remove_document_transform(&mut self) -> DAffine2 {
		let old_artwork_transform = self.metadata().document_to_viewport;
		self.document_legacy.metadata.document_to_viewport = DAffine2::IDENTITY;
		DocumentLegacy::mark_children_as_dirty(&mut self.document_legacy.root);

		old_artwork_transform
	}

	/// Transform the artwork and artboard back to their original scales
	pub(crate) fn restore_document_transform(&mut self, old_artwork_transform: DAffine2) {
		self.document_legacy.metadata.document_to_viewport = old_artwork_transform;
		DocumentLegacy::mark_children_as_dirty(&mut self.document_legacy.root);
	}

	pub fn render_document(&mut self, size: DVec2, transform: DAffine2, transparent_background: bool, persistent_data: &PersistentData, render_mode: DocumentRenderMode) -> String {
		// Render the document SVG code

		let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::Normal, None);

		let (artwork, outside) = match render_mode {
			DocumentRenderMode::Root => (self.document_legacy.render_root(&render_data), None),
			DocumentRenderMode::OnlyBelowLayerInFolder(below_layer_path) => (self.document_legacy.render_layers_below(below_layer_path, &render_data).unwrap(), None),
			DocumentRenderMode::LayerCutout(layer_path, background) => (self.document_legacy.render_layer(layer_path, &render_data).unwrap(), Some(background)),
		};
		let canvas_background_color = outside.map_or_else(|| "222222".to_string(), |col| col.rgba_hex());
		let canvas_background = match transparent_background {
			false => format!(r##"<rect x="0" y="0" width="100%" height="100%" fill="#{canvas_background_color}" />"##),
			true => "".into(),
		};
		let matrix = transform
			.to_cols_array()
			.iter()
			.enumerate()
			.fold(String::new(), |acc, (i, entry)| acc + &(entry.to_string() + if i == 5 { "" } else { "," }));
		let svg = format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" preserveAspectRatio="none" viewBox="0 0 1 1" width="{}" height="{}">{}{canvas_background}<g transform="matrix({matrix})">{artwork}</g></svg>"#,
			size.x, size.y, "\n",
		);

		svg
	}

	pub fn serialize_document(&self) -> String {
		let val = serde_json::to_string(self);
		// We fully expect the serialization to succeed
		val.unwrap()
	}

	pub fn deserialize_document(serialized_content: &str) -> Result<Self, DocumentError> {
		let deserialized_result: Result<Self, DocumentError> = serde_json::from_str(serialized_content).map_err(|e| DocumentError::InvalidFile(e.to_string()));
		match deserialized_result {
			Ok(document) => {
				if document.version == GRAPHITE_DOCUMENT_VERSION {
					Ok(document)
				} else {
					Err(DocumentError::InvalidFile("Graphite document version mismatch".to_string()))
				}
			}
			Err(e) => Err(e),
		}
	}

	pub fn with_name(name: String, ipp: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> Self {
		let mut document = Self { name, ..Self::default() };
		let transform = document.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.size() / 2.);
		document.document_legacy.metadata.document_to_viewport = transform;
		responses.add(DocumentMessage::UpdateDocumentTransform { transform });

		document
	}

	pub fn with_name_and_content(name: String, serialized_content: String) -> Result<Self, EditorError> {
		match Self::deserialize_document(&serialized_content) {
			Ok(mut document) => {
				document.name = name;
				Ok(document)
			}
			Err(DocumentError::InvalidFile(msg)) => Err(EditorError::DocumentDeserialization(msg)),
			_ => Err(EditorError::Document(String::from("Failed to open file"))),
		}
	}

	pub fn is_unmodified_default(&self) -> bool {
		self.serialize_root().len() == Self::default().serialize_root().len()
			&& self.document_undo_history.is_empty()
			&& self.document_redo_history.is_empty()
			&& self.name.starts_with(DEFAULT_DOCUMENT_NAME)
	}

	fn select_layer(&mut self, path: &[LayerId], render_data: &RenderData) -> Option<Message> {
		println!("Select_layer fail: {:?}", self.all_layers_sorted());

		if let Some(layer) = self.layer_metadata.get_mut(path) {
			let render_data = RenderData::new(render_data.font_cache, self.view_mode, None);

			layer.selected = true;
			let data = self.layer_panel_entry(path.to_vec(), &render_data).ok()?;
			(!path.is_empty()).then(|| FrontendMessage::UpdateDocumentLayerDetails { data }.into())
		} else {
			warn!("Tried to select non existing layer {path:?}");
			None
		}
	}

	pub fn selected_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.layer_metadata.iter().filter_map(|(path, data)| data.selected.then_some(path.as_slice()))
	}

	pub fn selected_layers_with_type(&self, discriminant: LayerDataTypeDiscriminant) -> impl Iterator<Item = &[LayerId]> {
		self.selected_layers().filter(move |path| {
			self.document_legacy
				.layer(path)
				.map(|layer| LayerDataTypeDiscriminant::from(&layer.data) == discriminant)
				.unwrap_or(false)
		})
	}

	pub fn non_selected_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.layer_metadata.iter().filter_map(|(path, data)| (!data.selected).then_some(path.as_slice()))
	}

	pub fn selected_layers_without_children(&self) -> Vec<&[LayerId]> {
		let unique_layers = DocumentLegacy::shallowest_unique_layers(self.selected_layers());

		// We need to maintain layer ordering
		self.sort_layers(unique_layers.iter().copied())
	}

	pub fn selected_layers_contains(&self, path: &[LayerId]) -> bool {
		self.layer_metadata.get(path).map(|layer| layer.selected).unwrap_or(false)
	}

	pub fn selected_visible_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.selected_layers().filter(|path| match self.document_legacy.layer(path) {
			Ok(layer) => layer.visible,
			Err(_) => false,
		})
	}

	pub fn visible_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.all_layers().filter(|path| match self.document_legacy.layer(path) {
			Ok(layer) => layer.visible,
			Err(_) => false,
		})
	}

	/// Returns the bounding boxes for all visible layers, optionally excluding any paths.
	pub fn bounding_boxes<'a>(&'a self, ignore_document: Option<&'a Vec<Vec<LayerId>>>, _ignore_artboard: Option<LayerId>, render_data: &'a RenderData) -> impl Iterator<Item = [DVec2; 2]> + 'a {
		self.visible_layers()
			.filter(move |path| ignore_document.map_or(true, |ignore_document| !ignore_document.iter().any(|ig| ig.as_slice() == *path)))
			.filter_map(|path| self.document_legacy.viewport_bounding_box(path, render_data).ok()?)
	}

	fn serialize_structure(&self, folder: LayerNodeIdentifier, structure: &mut Vec<u64>, data: &mut Vec<LayerId>, path: &mut Vec<LayerId>) {
		let mut space = 0;
		for layer_node in folder.children(self.metadata()) {
			data.push(layer_node.to_node());
			info!("Pushed child");
			space += 1;
			if layer_node.has_children(self.metadata()) {
				path.push(layer_node.to_node());

				// TODO: Skip if folder is not expanded.
				structure.push(space);
				self.serialize_structure(layer_node, structure, data, path);
				space = 0;

				path.pop();
			}
		}
		structure.push(space | 1 << 63);
	}

	/// Serializes the layer structure into a condensed 1D structure.
	///
	/// # Format
	/// It is a string of numbers broken into three sections:
	///
	/// | Data                                                                                                                           | Description                                  | Length           |
	/// |--------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------|------------------|
	/// | `4,` `2, 1, -2, -0,` `16533113728871998040,3427872634365736244,18115028555707261608,15878401910454357952,449479075714955186`   | Encoded example data                         |                  |
	/// | `L` = `4` = `structure.len()`                                                                                                  | `L`, the length of the **Structure** section | First value      |
	/// | **Structure** section = `2, 1, -2, -0`                                                                                         | The **Structure** section                    | Next `L` values  |
	/// | **Data** section = `16533113728871998040, 3427872634365736244, 18115028555707261608, 15878401910454357952, 449479075714955186` | The **Data** section (layer IDs)             | Remaining values |
	///
	/// The data section lists the layer IDs for all folders/layers in the tree as read from top to bottom.
	/// The structure section lists signed numbers. The sign indicates a folder indentation change (`+` is down a level, `-` is up a level).
	/// The numbers in the structure block encode the indentation. For example:
	/// - `2` means read two element from the data section, then place a `[`.
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
	pub fn serialize_root(&self) -> Vec<u64> {
		let (mut structure, mut data) = (vec![0], Vec::new());
		self.serialize_structure(self.metadata().root(), &mut structure, &mut data, &mut vec![]);
		structure[0] = structure.len() as u64 - 1;
		structure.extend(data);

		structure
	}

	/// Returns an unsorted list of all layer paths including folders at all levels, except the document's top-level root folder itself
	pub fn all_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.layer_metadata.keys().filter_map(|path| (!path.is_empty()).then_some(path.as_slice()))
	}

	/// Returns the paths to all layers in order
	fn sort_layers<'a>(&self, paths: impl Iterator<Item = &'a [LayerId]>) -> Vec<&'a [LayerId]> {
		// Compute the indices for each layer to be able to sort them
		let mut layers_with_indices: Vec<(&[LayerId], Vec<usize>)> = paths
			// 'path.len() > 0' filters out root layer since it has no indices
			.filter(|path| !path.is_empty())
			.filter_map(|path| {
				// TODO: `indices_for_path` can return an error. We currently skip these layers and log a warning. Once this problem is solved this code can be simplified.
				match self.document_legacy.indices_for_path(path) {
					Err(err) => {
						warn!("layers_sorted: Could not get indices for the layer {path:?}: {err:?}");
						None
					}
					Ok(indices) => Some((path, indices)),
				}
			})
			.collect();

		layers_with_indices.sort_by_key(|(_, indices)| indices.clone());
		layers_with_indices.into_iter().map(|(path, _)| path).collect()
	}

	/// Returns the paths to all layers in order
	pub fn all_layers_sorted(&self) -> Vec<&[LayerId]> {
		self.sort_layers(self.all_layers())
	}

	/// Returns the paths to all selected layers in order
	pub fn selected_layers_sorted(&self) -> Vec<&[LayerId]> {
		self.sort_layers(self.selected_layers())
	}

	/// Returns the paths to all non_selected layers in order
	#[allow(dead_code)] // used for test cases
	pub fn non_selected_layers_sorted(&self) -> Vec<&[LayerId]> {
		self.sort_layers(self.non_selected_layers())
	}

	pub fn layer_metadata(&self, path: &[LayerId]) -> &LayerMetadata {
		self.layer_metadata.get(path).unwrap_or_else(|| panic!("Editor's layer metadata for {path:?} does not exist"))
	}

	pub fn layer_metadata_mut(&mut self, path: &[LayerId]) -> &mut LayerMetadata {
		Self::layer_metadata_mut_no_borrow_self(&mut self.layer_metadata, path)
	}

	pub fn layer_metadata_mut_no_borrow_self<'a>(layer_metadata: &'a mut HashMap<Vec<LayerId>, LayerMetadata>, path: &[LayerId]) -> &'a mut LayerMetadata {
		layer_metadata
			.get_mut(path)
			.unwrap_or_else(|| panic!("Layer data cannot be found because the path {path:?} does not exist"))
	}

	/// Places a document into the history system
	fn backup_with_document(&mut self, document: DocumentLegacy, layer_metadata: HashMap<Vec<LayerId>, LayerMetadata>, responses: &mut VecDeque<Message>) {
		self.document_redo_history.clear();
		self.document_undo_history.push_back(DocumentSave { document, layer_metadata });
		if self.document_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			self.document_undo_history.pop_front();
		}

		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
	}

	/// Copies the entire document into the history system
	pub fn backup(&mut self, responses: &mut VecDeque<Message>) {
		self.backup_with_document(self.document_legacy.clone(), self.layer_metadata.clone(), responses);
	}

	/// Push a message backing up the document in its current state
	pub fn backup_nonmut(&self, responses: &mut VecDeque<Message>) {
		responses.add(DocumentMessage::BackupDocument {
			document: self.document_legacy.clone(),
			layer_metadata: self.layer_metadata.clone(),
		});
	}

	pub fn rollback(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		self.backup(responses);
		self.undo(responses)
		// TODO: Consider if we should check if the document is saved
	}

	/// Replace the document with a new document save, returning the document save.
	pub fn replace_document(&mut self, DocumentSave { document, layer_metadata }: DocumentSave) -> DocumentSave {
		// Keeping the root is required if the bounds of the viewport have changed during the operation
		let old_root = self.metadata().document_to_viewport;
		let document = std::mem::replace(&mut self.document_legacy, document);
		self.document_legacy.metadata.document_to_viewport = old_root;
		self.document_legacy.root.cache_dirty = true;

		let layer_metadata = std::mem::replace(&mut self.layer_metadata, layer_metadata);

		DocumentSave { document, layer_metadata }
	}

	pub fn undo(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);

		let selected_paths: Vec<Vec<LayerId>> = self.selected_layers().map(|path| path.to_vec()).collect();

		match self.document_undo_history.pop_back() {
			Some(DocumentSave { document, layer_metadata }) => {
				// Update the currently displayed layer on the Properties panel if the selection changes after an undo action
				// Also appropriately update the Properties panel if an undo action results in a layer being deleted
				let prev_selected_paths: Vec<Vec<LayerId>> = layer_metadata.iter().filter_map(|(layer_id, metadata)| metadata.selected.then_some(layer_id.clone())).collect();

				if prev_selected_paths != selected_paths {
					responses.add(BroadcastEvent::SelectionChanged);
				}

				let document_save = self.replace_document(DocumentSave { document, layer_metadata });

				self.document_redo_history.push_back(document_save);
				if self.document_redo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
					self.document_redo_history.pop_front();
				}

				for layer in self.layer_metadata.keys() {
					responses.add(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() })
				}

				responses.add(NodeGraphMessage::SendGraph { should_rerender: true });

				Ok(())
			}
			None => Err(EditorError::NoTransactionInProgress),
		}
	}

	pub fn redo(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);

		let selected_paths: Vec<Vec<LayerId>> = self.selected_layers().map(|path| path.to_vec()).collect();

		match self.document_redo_history.pop_back() {
			Some(DocumentSave { document, layer_metadata }) => {
				// Update currently displayed layer on property panel if selection changes after redo action
				// Also appropriately update property panel if redo action results in a layer being added
				let next_selected_paths: Vec<Vec<LayerId>> = layer_metadata.iter().filter_map(|(layer_id, metadata)| metadata.selected.then_some(layer_id.clone())).collect();

				if next_selected_paths != selected_paths {
					responses.add(BroadcastEvent::SelectionChanged);
				}

				let document_save = self.replace_document(DocumentSave { document, layer_metadata });
				self.document_undo_history.push_back(document_save);
				if self.document_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
					self.document_undo_history.pop_front();
				}

				for layer in self.layer_metadata.keys() {
					responses.add(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() })
				}

				responses.add(NodeGraphMessage::SendGraph { should_rerender: true });

				Ok(())
			}
			None => Err(EditorError::NoTransactionInProgress),
		}
	}

	pub fn current_identifier(&self) -> u64 {
		// We can use the last state of the document to serve as the identifier to compare against
		// This is useful since when the document is empty the identifier will be 0
		self.document_undo_history
			.iter()
			.last()
			.map(|DocumentSave { document, .. }| document.current_state_identifier())
			.unwrap_or(0)
	}

	pub fn is_auto_saved(&self) -> bool {
		self.current_identifier() == self.auto_saved_document_identifier
	}

	pub fn is_saved(&self) -> bool {
		self.current_identifier() == self.saved_document_identifier
	}

	pub fn set_auto_save_state(&mut self, is_saved: bool) {
		if is_saved {
			self.auto_saved_document_identifier = self.current_identifier();
		} else {
			self.auto_saved_document_identifier = generate_uuid();
		}
	}

	pub fn set_save_state(&mut self, is_saved: bool) {
		if is_saved {
			self.saved_document_identifier = self.current_identifier();
		} else {
			self.saved_document_identifier = generate_uuid();
		}
	}

	// TODO: This should probably take a slice not a vec, also why does this even exist when `layer_panel_entry_from_path` also exists?
	pub fn layer_panel_entry(&mut self, path: Vec<LayerId>, render_data: &RenderData) -> Result<LayerPanelEntry, EditorError> {
		let data: LayerMetadata = *self
			.layer_metadata
			.get_mut(&path)
			.ok_or_else(|| EditorError::Document(format!("Could not get layer metadata for {path:?}")))?;
		let layer = self.document_legacy.layer(&path)?;
		let entry = LayerPanelEntry::new(&data, self.document_legacy.multiply_transforms(&path)?, layer, path, render_data);
		Ok(entry)
	}

	/// Returns a list of `LayerPanelEntry`s intended for display purposes. These don't contain
	/// any actual data, but rather attributes such as visibility and names of the layers.
	pub fn layer_panel(&mut self, path: &[LayerId], render_data: &RenderData) -> Result<Vec<LayerPanelEntry>, EditorError> {
		let folder = self.document_legacy.folder(path)?;
		let paths: Vec<Vec<LayerId>> = folder.layer_ids.iter().map(|id| [path, &[*id]].concat()).collect();
		let entries = paths.iter().rev().filter_map(|path| self.layer_panel_entry_from_path(path, render_data)).collect();
		Ok(entries)
	}

	pub fn layer_panel_entry_from_path(&self, path: &[LayerId], render_data: &RenderData) -> Option<LayerPanelEntry> {
		let layer_metadata = self.layer_metadata(path);
		let transform = self.document_legacy.generate_transform_across_scope(path, Some(self.metadata().document_to_viewport.inverse())).ok()?;
		let layer = self.document_legacy.layer(path).ok()?;

		Some(LayerPanelEntry::new(layer_metadata, transform, layer, path.to_vec(), render_data))
	}

	/// When working with an insert index, deleting the layers may cause the insert index to point to a different location (if the layer being deleted was located before the insert index).
	///
	/// This function updates the insert index so that it points to the same place after the specified `layers` are deleted.
	fn update_insert_index(&self, layers: &[LayerNodeIdentifier], parent: LayerNodeIdentifier, insert_index: isize) -> Result<isize, DocumentError> {
		let layer_ids_above = parent.children(self.metadata()).take(if insert_index < 0 { usize::MAX } else { insert_index as usize });
		let new_insert_index = layer_ids_above.filter(|layer_id| !layers.contains(layer_id)).count() as isize;

		Ok(new_insert_index)
	}

	/// Calculates the bounding box of all layers in the document
	pub fn all_layer_bounds(&self, render_data: &RenderData) -> Option<[DVec2; 2]> {
		self.document_legacy.viewport_bounding_box(&[], render_data).ok().flatten()
	}

	/// Calculate the path that new layers should be inserted to.
	/// Depends on the selected layers as well as their types (Folder/Non-Folder)
	pub fn get_path_for_new_layer(&self) -> Vec<u64> {
		// If the selected layers don't actually exist, a new uuid for the
		// root folder will be returned
		let mut path = self.document_legacy.shallowest_common_folder(self.selected_layers()).map_or(vec![], |v| v.to_vec());
		path.push(generate_uuid());
		path
	}

	pub fn new_layer_parent(&self) -> LayerNodeIdentifier {
		let new_parent = self
			.metadata()
			.deepest_common_ancestor(self.metadata().selected_layers())
			.unwrap_or_else(|| self.metadata().active_artboard());
		println!("New parent {new_parent:?} layers{:?}", self.metadata().selected_layers().collect::<Vec<_>>());
		new_parent
	}

	/// Loads layer resources such as creating the blob URLs for the images and loading all of the fonts in the document
	pub fn load_layer_resources(&self, responses: &mut VecDeque<Message>) {
		let mut fonts = HashSet::new();
		for (_node_id, node) in self.document_legacy.document_network.recursive_nodes() {
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
		let snapping_state = self.snapping_state.clone();
		let mut widgets = vec![
			OptionalInput::new(snapping_state.snapping_enabled, "Snapping")
				.tooltip("Snapping")
				.on_update(move |optional_input: &OptionalInput| {
					let snapping_enabled = optional_input.checked;
					DocumentMessage::SetSnapping {
						snapping_enabled: Some(snapping_enabled),
						bounding_box_snapping: Some(snapping_state.bounding_box_snapping),
						node_snapping: Some(snapping_state.node_snapping),
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
										node_snapping: None,
									}
									.into()
								})
								.widget_holder(),
							Separator::new(SeparatorType::Unrelated).widget_holder(),
							TextLabel::new(SnappingOptions::BoundingBoxes.to_string()).table_align(false).min_width(60).widget_holder(),
							Separator::new(SeparatorType::Related).widget_holder(),
						],
					},
					LayoutGroup::Row {
						widgets: vec![
							CheckboxInput::new(self.snapping_state.node_snapping)
								.tooltip(SnappingOptions::Points.to_string())
								.on_update(|input: &CheckboxInput| {
									DocumentMessage::SetSnapping {
										snapping_enabled: None,
										bounding_box_snapping: None,
										node_snapping: Some(input.checked),
									}
									.into()
								})
								.widget_holder(),
							Separator::new(SeparatorType::Unrelated).widget_holder(),
							TextLabel::new(SnappingOptions::Points.to_string()).table_align(false).min_width(60).widget_holder(),
						],
					},
				])
				.widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			OptionalInput::new(true, "Grid")
				.tooltip("Grid")
				.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(318) }.into())
				.widget_holder(),
			PopoverButton::new("Grid", "Coming soon").widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			OptionalInput::new(self.overlays_visible, "Overlays")
				.tooltip("Overlays")
				.on_update(|optional_input: &OptionalInput| DocumentMessage::SetOverlaysVisibility { visible: optional_input.checked }.into())
				.widget_holder(),
			PopoverButton::new("Overlays", "Coming soon").widget_holder(),
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
			Separator::new(SeparatorType::Section).widget_holder(),
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
				.tooltip("Zoom to 100%")
				.tooltip_shortcut(action_keys!(DocumentMessageDiscriminant::ZoomCanvasTo100Percent))
				.on_update(|_| NavigationMessage::SetCanvasZoom { zoom_factor: 1. }.into())
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(self.navigation_handler.snapped_scale() * 100.))
				.unit("%")
				.min(0.000001)
				.max(1000000.)
				.mode_increment()
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
		let rotation_value = self.navigation_handler.snapped_angle() / (std::f64::consts::PI / 180.);
		if rotation_value.abs() > 0.00001 {
			widgets.extend([
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(rotation_value))
					.unit("°")
					.step(15.)
					.on_update(|number_input: &NumberInput| {
						NavigationMessage::SetCanvasRotation {
							angle_radians: number_input.value.unwrap() * (std::f64::consts::PI / 180.),
						}
						.into()
					})
					.widget_holder(),
			]);
		}
		widgets.extend([
			Separator::new(SeparatorType::Related).widget_holder(),
			PopoverButton::new(
				"Canvas Navigation",
				"Interactive options in this popover\nmenu are coming soon.\n\nZoom:\n• Shift + Middle Click Drag\n• Ctrl + Scroll Wheel Roll\nRotate:\n• Alt + Left Click Drag",
			)
			.widget_holder(),
		]);
		let document_bar_layout = WidgetLayout::new(vec![LayoutGroup::Row { widgets }]);

		let document_mode_layout = WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![
				DropdownInput::new(
					vec![vec![
						DropdownEntryData::new(DocumentMode::DesignMode.to_string()).icon(DocumentMode::DesignMode.icon_name()),
						DropdownEntryData::new(DocumentMode::SelectMode.to_string())
							.icon(DocumentMode::SelectMode.icon_name())
							.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(330) }.into()),
						DropdownEntryData::new(DocumentMode::GuideMode.to_string())
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
			layout: Layout::WidgetLayout(document_bar_layout),
			layout_target: LayoutTarget::DocumentBar,
		});

		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(document_mode_layout),
			layout_target: LayoutTarget::DocumentMode,
		});
	}

	pub fn update_layer_tree_options_bar_widgets(&self, responses: &mut VecDeque<Message>, render_data: &RenderData) {
		let mut opacity = None;
		let mut opacity_is_mixed = false;

		let mut blend_mode = None;
		let mut blend_mode_is_mixed = false;

		self.layer_metadata
			.keys()
			.filter_map(|path| self.layer_panel_entry_from_path(path, render_data))
			.filter(|layer_panel_entry| layer_panel_entry.layer_metadata.selected)
			.flat_map(|layer_panel_entry| self.document_legacy.layer(layer_panel_entry.path.as_slice()))
			.for_each(|layer| {
				match opacity {
					None => opacity = Some(layer.opacity),
					Some(opacity) => {
						if (opacity - layer.opacity).abs() > (1. / 1_000_000.) {
							opacity_is_mixed = true;
						}
					}
				}

				match blend_mode {
					None => blend_mode = Some(layer.blend_mode),
					Some(blend_mode) => {
						if blend_mode != layer.blend_mode {
							blend_mode_is_mixed = true;
						}
					}
				}
			});

		if opacity_is_mixed {
			opacity = None;
		}
		if blend_mode_is_mixed {
			blend_mode = None;
		}

		let blend_mode_menu_entries = BlendMode::list_modes_in_groups()
			.iter()
			.map(|modes| {
				modes
					.iter()
					.map(|mode| {
						DropdownEntryData::new(mode.to_string())
							.value(mode.to_string())
							.on_update(|_| DocumentMessage::SetBlendModeForSelectedLayers { blend_mode: *mode }.into())
					})
					.collect()
			})
			.collect();

		let layer_tree_options = WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![
				DropdownInput::new(blend_mode_menu_entries)
					.selected_index(blend_mode.map(|blend_mode| blend_mode as u32))
					.disabled(blend_mode.is_none() && !blend_mode_is_mixed)
					.draw_icon(false)
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(opacity.map(|opacity| opacity * 100.))
					.label("Opacity")
					.unit("%")
					.display_decimal_places(2)
					.disabled(opacity.is_none() && !opacity_is_mixed)
					.min(0.)
					.max(100.)
					.range_min(Some(0.))
					.range_max(Some(100.))
					.mode(NumberInputMode::Range)
					.on_update(|number_input: &NumberInput| {
						if let Some(value) = number_input.value {
							DocumentMessage::SetOpacityForSelectedLayers { opacity: value / 100. }.into()
						} else {
							Message::NoOp
						}
					})
					.widget_holder(),
				Separator::new(SeparatorType::Section).widget_holder(),
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
			layout: Layout::WidgetLayout(layer_tree_options),
			layout_target: LayoutTarget::LayerTreeOptions,
		});
	}

	pub fn selected_layers_reorder(&mut self, relative_index_offset: isize, responses: &mut VecDeque<Message>) {
		self.backup(responses);

		let mut selected_layers = self.metadata().selected_layers();

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
}
