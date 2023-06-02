use super::utility_types::error::EditorError;
use super::utility_types::misc::DocumentRenderMode;
use crate::application::generate_uuid;
use crate::consts::{ASYMPTOTIC_EFFECT, DEFAULT_DOCUMENT_NAME, FILE_SAVE_SUFFIX, GRAPHITE_DOCUMENT_VERSION, SCALE_EFFECT, SCROLLBAR_SPACING, VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR};
use crate::messages::frontend::utility_types::ExportBounds;
use crate::messages::frontend::utility_types::FileType;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::button_widgets::{IconButton, PopoverButton};
use crate::messages::layout::utility_types::widgets::input_widgets::{
	DropdownEntryData, DropdownInput, NumberInput, NumberInputIncrementBehavior, NumberInputMode, OptionalInput, RadioEntryData, RadioInput,
};
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType};
use crate::messages::portfolio::document::properties_panel::utility_types::PropertiesPanelMessageHandlerData;
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::portfolio::document::utility_types::layer_panel::{LayerMetadata, LayerPanelEntry, RawBuffer};
use crate::messages::portfolio::document::utility_types::misc::DocumentMode;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, DocumentSave, FlipAxis};
use crate::messages::portfolio::document::utility_types::vectorize_layer_metadata;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::ToolType;
use crate::node_graph_executor::NodeGraphExecutor;

use document_legacy::document::Document as DocumentLegacy;
use document_legacy::layers::blend_mode::BlendMode;
use document_legacy::layers::folder_layer::FolderLayer;
use document_legacy::layers::layer_info::{LayerDataType, LayerDataTypeDiscriminant};
use document_legacy::layers::layer_layer::CachedOutputData;
use document_legacy::layers::style::{RenderData, ViewMode};
use document_legacy::{DocumentError, DocumentResponse, LayerId, Operation as DocumentOperation};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput, NodeNetwork};
use graphene_core::raster::ImageFrame;
use graphene_core::text::Font;

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
	pub snapping_enabled: bool,
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
	layer_range_selection_reference: Vec<LayerId>,

	navigation_handler: NavigationMessageHandler,
	#[serde(skip)]
	overlays_message_handler: OverlaysMessageHandler,
	pub artboard_message_handler: ArtboardMessageHandler,
	properties_panel_message_handler: PropertiesPanelMessageHandler,
	#[serde(skip)]
	node_graph_handler: NodeGraphMessageHandler,
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			document_legacy: DocumentLegacy::default(),
			saved_document_identifier: 0,
			auto_saved_document_identifier: 0,
			name: String::from("Untitled Document"),
			version: GRAPHITE_DOCUMENT_VERSION.to_string(),

			document_mode: DocumentMode::DesignMode,
			view_mode: ViewMode::default(),
			snapping_enabled: true,
			overlays_visible: true,

			document_undo_history: VecDeque::new(),
			document_redo_history: VecDeque::new(),
			undo_in_progress: false,

			layer_metadata: vec![(vec![], LayerMetadata::new(true))].into_iter().collect(),
			layer_range_selection_reference: Vec::new(),

			navigation_handler: NavigationMessageHandler::default(),
			overlays_message_handler: OverlaysMessageHandler::default(),
			artboard_message_handler: ArtboardMessageHandler::default(),
			properties_panel_message_handler: PropertiesPanelMessageHandler::default(),
			node_graph_handler: Default::default(),
		}
	}
}

impl MessageHandler<DocumentMessage, (u64, &InputPreprocessorMessageHandler, &PersistentData, &PreferencesMessageHandler, &mut NodeGraphExecutor)> for DocumentMessageHandler {
	#[remain::check]
	fn process_message(
		&mut self,
		message: DocumentMessage,
		responses: &mut VecDeque<Message>,
		(document_id, ipp, persistent_data, preferences, executor): (u64, &InputPreprocessorMessageHandler, &PersistentData, &PreferencesMessageHandler, &mut NodeGraphExecutor),
	) {
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
								DocumentResponse::AddSelectedLayer { additional_layers } => responses.add(AddSelectedLayers {
									additional_layers: additional_layers.clone(),
								}),
								DocumentResponse::DeletedLayer { path } => {
									self.layer_metadata.remove(path);
								}
								DocumentResponse::LayerChanged { path } => responses.add(LayerChanged { affected_layer_path: path.clone() }),
								DocumentResponse::MoveSelectedLayersTo {
									folder_path,
									insert_index,
									reverse_index,
								} => responses.add(MoveSelectedLayersTo {
									folder_path: folder_path.clone(),
									insert_index: *insert_index,
									reverse_index: *reverse_index,
								}),
								DocumentResponse::CreatedLayer { path, is_selected } => {
									if self.layer_metadata.contains_key(path) {
										warn!("CreatedLayer overrides existing layer metadata.");
									}
									self.layer_metadata.insert(path.clone(), LayerMetadata::new(false));

									responses.add(LayerChanged { affected_layer_path: path.clone() });
									self.layer_range_selection_reference = path.clone();

									if *is_selected {
										responses.add(AddSelectedLayers {
											additional_layers: vec![path.clone()],
										});
									}
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
					Err(e) => error!("DocumentError: {:?}", e),
					Ok(_) => (),
				}
			}
			#[remain::unsorted]
			Artboard(message) => {
				self.artboard_message_handler.process_message(message, responses, persistent_data);
			}
			#[remain::unsorted]
			Navigation(message) => {
				self.navigation_handler
					.process_message(message, responses, (&self.document_legacy, ipp, self.selected_visible_layers_bounding_box(&render_data)));
			}
			#[remain::unsorted]
			Overlays(message) => {
				self.overlays_message_handler.process_message(message, responses, (self.overlays_visible, persistent_data, ipp));
			}
			#[remain::unsorted]
			PropertiesPanel(message) => {
				let properties_panel_message_handler_data = PropertiesPanelMessageHandlerData {
					artwork_document: &self.document_legacy,
					artboard_document: &self.artboard_message_handler.artboards_document,
					selected_layers: &mut self.layer_metadata.iter().filter_map(|(path, data)| data.selected.then_some(path.as_slice())),
					node_graph_message_handler: &self.node_graph_handler,
					executor,
				};
				self.properties_panel_message_handler
					.process_message(message, responses, (persistent_data, properties_panel_message_handler_data));
			}
			#[remain::unsorted]
			NodeGraph(message) => {
				self.node_graph_handler.process_message(message, responses, (&mut self.document_legacy, executor, document_id));
			}
			#[remain::unsorted]
			GraphOperation(message) => GraphOperationMessageHandler.process_message(message, responses, (&mut self.document_legacy, &mut self.node_graph_handler)),

			// Messages
			AbortTransaction => {
				if !self.undo_in_progress {
					self.undo(responses).unwrap_or_else(|e| warn!("{}", e));
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
				let (paths, boxes): (Vec<_>, Vec<_>) = self
					.selected_layers()
					.filter_map(|path| self.document_legacy.viewport_bounding_box(path, &render_data).ok()?.map(|b| (path, b)))
					.unzip();

				let axis = match axis {
					AlignAxis::X => DVec2::X,
					AlignAxis::Y => DVec2::Y,
				};
				let lerp = |bbox: &[DVec2; 2]| bbox[0].lerp(bbox[1], 0.5);
				if let Some(combined_box) = self.document_legacy.combined_viewport_bounding_box(self.selected_layers(), &render_data) {
					let aggregated = match aggregate {
						AlignAggregate::Min => combined_box[0],
						AlignAggregate::Max => combined_box[1],
						AlignAggregate::Center => lerp(&combined_box),
						AlignAggregate::Average => boxes.iter().map(|b| lerp(b)).reduce(|a, b| a + b).map(|b| b / boxes.len() as f64).unwrap(),
					};
					for (path, bbox) in paths.into_iter().zip(boxes) {
						let center = match aggregate {
							AlignAggregate::Min => bbox[0],
							AlignAggregate::Max => bbox[1],
							_ => lerp(&bbox),
						};
						let translation = (aggregated - center) * axis;
						responses.add(GraphOperationMessage::TransformChange {
							layer: path.to_vec(),
							transform: DAffine2::from_translation(translation),
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						});
					}
					responses.add(BroadcastEvent::DocumentIsDirty);
				}
			}
			BackupDocument { document, artboard, layer_metadata } => self.backup_with_document(document, *artboard, layer_metadata, responses),
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
			CreateEmptyFolder { mut container_path } => {
				let id = generate_uuid();
				container_path.push(id);
				responses.add(DocumentMessage::DeselectAllLayers);
				responses.add(DocumentOperation::CreateFolder {
					path: container_path.clone(),
					insert_index: -1,
				});
				responses.add(DocumentMessage::SetLayerExpansion {
					layer_path: container_path,
					set_expanded: true,
				});
			}
			DebugPrintDocument => {
				info!("{:#?}\n{:#?}", self.document_legacy, self.layer_metadata);
			}
			DeleteLayer { layer_path } => {
				responses.add_front(DocumentOperation::DeleteLayer { path: layer_path.clone() });
				responses.add_front(BroadcastEvent::ToolAbort);
				responses.add(PropertiesPanelMessage::CheckSelectedWasDeleted { path: layer_path });
			}
			DeleteSelectedLayers => {
				self.backup(responses);

				for path in self.selected_layers_without_children() {
					responses.add_front(DocumentMessage::DeleteLayer { layer_path: path.to_vec() });
				}

				responses.add_front(BroadcastEvent::SelectionChanged);
				responses.add(BroadcastEvent::DocumentIsDirty);
			}
			DeselectAllLayers => {
				responses.add_front(SetSelectedLayers { replacement_selected_layers: vec![] });
				self.layer_range_selection_reference.clear();
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
			DocumentHistoryBackward => self.undo(responses).unwrap_or_else(|e| warn!("{}", e)),
			DocumentHistoryForward => self.redo(responses).unwrap_or_else(|e| warn!("{}", e)),
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
				self.layer_range_selection_reference.clear();
				for path in self.selected_layers_sorted() {
					responses.add(DocumentOperation::DuplicateLayer { path: path.to_vec() });
				}
			}
			ExportDocument {
				file_name,
				file_type,
				scale_factor,
				bounds,
			} => {
				let old_transforms = self.remove_document_transform();

				// Calculate the bounding box of the region to be exported
				let bounds = match bounds {
					ExportBounds::AllArtwork => self.all_layer_bounds(&render_data),
					ExportBounds::Selection => self.selected_visible_layers_bounding_box(&render_data),
					ExportBounds::Artboard(id) => self.artboard_message_handler.artboards_document.layer(&[id]).ok().and_then(|layer| layer.aabb(&render_data)),
				}
				.unwrap_or_default();
				let size = bounds[1] - bounds[0];
				let transform = (DAffine2::from_translation(bounds[0]) * DAffine2::from_scale(size)).inverse();

				let document = self.render_document(size, transform, persistent_data, DocumentRenderMode::Root);

				self.restore_document_transform(old_transforms);

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
				if let Some([min, max]) = self.document_legacy.combined_viewport_bounding_box(self.selected_layers(), &render_data) {
					let center = (max + min) / 2.;
					let bbox_trans = DAffine2::from_translation(-center);
					for path in self.selected_layers() {
						responses.add(GraphOperationMessage::TransformChange {
							layer: path.to_vec(),
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
				let mut new_folder_path = self.document_legacy.shallowest_common_folder(self.selected_layers()).unwrap_or(&[]).to_vec();

				// Required for grouping parent folders with their own children
				if !new_folder_path.is_empty() && self.selected_layers_contains(&new_folder_path) {
					new_folder_path.remove(new_folder_path.len() - 1);
				}

				new_folder_path.push(generate_uuid());

				responses.add(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
				responses.add(DocumentMessage::DeleteSelectedLayers);
				responses.add(DocumentOperation::CreateFolder {
					path: new_folder_path.clone(),
					insert_index: -1,
				});
				responses.add(DocumentMessage::ToggleLayerExpansion { layer_path: new_folder_path.clone() });
				responses.add(PortfolioMessage::PasteIntoFolder {
					clipboard: Clipboard::Internal,
					folder_path: new_folder_path.clone(),
					insert_index: -1,
				});
				responses.add(DocumentMessage::SetSelectedLayers {
					replacement_selected_layers: vec![new_folder_path],
				});
			}
			ImaginateClear {
				layer_path,
				node_id,
				cached_index: input_index,
			} => {
				let value = graph_craft::document::value::TaggedValue::RcImage(None);
				responses.add(NodeGraphMessage::SetInputValue { node_id, input_index, value });
				responses.add(InputFrameRasterizeRegionBelowLayer { layer_path });
			}
			ImaginateGenerate { layer_path, imaginate_node } => {
				if let Some(message) = self.rasterize_region_below_layer(document_id, layer_path, preferences, persistent_data, Some(imaginate_node)) {
					responses.add(message);
				}
			}
			ImaginateRandom {
				layer_path,
				imaginate_node,
				then_generate,
			} => {
				// Set a random seed input
				responses.add(NodeGraphMessage::SetInputValue {
					node_id: *imaginate_node.last().unwrap(),
					// Needs to match the index of the seed parameter in `pub const IMAGINATE_NODE: DocumentNodeType` in `document_node_type.rs`
					input_index: 1,
					value: graph_craft::document::value::TaggedValue::F64((generate_uuid() >> 1) as f64),
				});

				// Generate the image
				if then_generate {
					responses.add(DocumentMessage::ImaginateGenerate { layer_path, imaginate_node });
				}
			}
			ImaginateTerminate { layer_path, node_path } => {
				responses.add(FrontendMessage::TriggerImaginateTerminate {
					document_id,
					layer_path,
					node_path,
					hostname: preferences.imaginate_server_hostname.clone(),
				});
			}
			InputFrameRasterizeRegionBelowLayer { layer_path } => {
				if layer_path.is_empty() {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				} else if let Some(message) = self.rasterize_region_below_layer(document_id, layer_path, preferences, persistent_data, None) {
					responses.add(message);
				}
			}
			LayerChanged { affected_layer_path } => {
				if let Ok(layer_entry) = self.layer_panel_entry(affected_layer_path.clone(), &render_data) {
					responses.add(FrontendMessage::UpdateDocumentLayerDetails { data: layer_entry });
				}
				responses.add(PropertiesPanelMessage::CheckSelectedWasUpdated { path: affected_layer_path });
				self.update_layer_tree_options_bar_widgets(responses, &render_data);
			}
			MoveSelectedLayersTo {
				folder_path,
				insert_index,
				reverse_index,
			} => {
				let selected_layers = self.selected_layers().collect::<Vec<_>>();

				// Prevent trying to insert into self
				if selected_layers.iter().any(|layer| folder_path.starts_with(layer)) {
					return;
				}

				let insert_index = self.update_insert_index(&selected_layers, &folder_path, insert_index, reverse_index).unwrap();

				responses.add(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
				responses.add(DocumentMessage::DeleteSelectedLayers);
				responses.add(PortfolioMessage::PasteIntoFolder {
					clipboard: Clipboard::Internal,
					folder_path,
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
				let image_size = DVec2::new(image.width as f64, image.height as f64);

				let Some(image_node_type) = crate::messages::portfolio::document::node_graph::resolve_document_node_type("Image") else {
					warn!("Image node should be in registry");
					return;
				};
				let Some(transform_node_type) = crate::messages::portfolio::document::node_graph::resolve_document_node_type("Transform") else {
					warn!("Transform node should be in registry");
					return;
				};
				let Some(downres_node_type) = crate::messages::portfolio::document::node_graph::resolve_document_node_type("Downres") else {
					warn!("Downres node should be in registry");
					return;
				};

				let path = vec![generate_uuid()];
				let mut network = NodeNetwork::default();

				// Transform of parent folder
				let to_parent_folder = self.document_legacy.generate_transform_across_scope(&path[..path.len() - 1], None).unwrap_or_default();

				// Align the layer with the mouse or center of viewport
				let viewport_location = mouse.map_or(ipp.viewport_bounds.center(), |pos| pos.into());
				let center_in_viewport = DAffine2::from_translation(viewport_location - ipp.viewport_bounds.top_left);
				let center_in_viewport_layerspace = to_parent_folder.inverse() * center_in_viewport;

				// Scale the image to fit into a 512x512 box
				let image_size = image_size / DVec2::splat((image_size.max_element() / 512.).max(1.));

				// Make layer the size of the image
				let fit_image_size = DAffine2::from_scale_angle_translation(image_size, 0., image_size / -2.);

				let transform = center_in_viewport_layerspace * fit_image_size;

				responses.add(DocumentMessage::StartTransaction);

				network.push_node(
					image_node_type.to_document_node(
						[graph_craft::document::NodeInput::value(
							graph_craft::document::value::TaggedValue::ImageFrame(ImageFrame { image, transform: DAffine2::IDENTITY }),
							false,
						)],
						graph_craft::document::DocumentNodeMetadata::position((8, 4)),
					),
					false,
				);
				network.push_node(transform_node_type.to_document_node_default_inputs([], Default::default()), true);
				network.push_node(downres_node_type.to_document_node_default_inputs([], Default::default()), true);
				network.push_output_node();

				responses.add(DocumentOperation::AddFrame {
					path: path.clone(),
					insert_index: -1,
					transform: DAffine2::ZERO.to_cols_array(),
					network,
				});
				responses.add(DocumentMessage::SetSelectedLayers {
					replacement_selected_layers: vec![path.clone()],
				});

				responses.add(GraphOperationMessage::TransformSet {
					layer: path.clone(),
					transform,
					transform_in: TransformIn::Local,
					skip_rerender: false,
				});

				responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path: path });

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
			RenameLayer { layer_path, new_name } => responses.add(DocumentOperation::RenameLayer { layer_path, new_name }),
			RenderDocument => {
				responses.add(FrontendMessage::UpdateDocumentArtwork {
					svg: self.document_legacy.render_root(&render_data),
				});
				responses.add(ArtboardMessage::RenderArtboards);

				let document_transform_scale = self.navigation_handler.snapped_scale();
				let scale = 0.5 + ASYMPTOTIC_EFFECT + document_transform_scale * SCALE_EFFECT;
				let viewport_size = ipp.viewport_bounds.size();
				let viewport_mid = ipp.viewport_bounds.center();
				let [bounds1, bounds2] = self.document_bounds(&render_data).unwrap_or([viewport_mid; 2]);
				let bounds1 = bounds1.min(viewport_mid) - viewport_size * scale;
				let bounds2 = bounds2.max(viewport_mid) + viewport_size * scale;
				let bounds_length = (bounds2 - bounds1) * (1. + SCROLLBAR_SPACING);
				let scrollbar_position = DVec2::splat(0.5) - (bounds1.lerp(bounds2, 0.5) - viewport_mid) / (bounds_length - viewport_size);
				let scrollbar_multiplier = bounds_length - viewport_size;
				let scrollbar_size = viewport_size / bounds_length;

				let log = document_transform_scale.log2();
				let ruler_interval = if log < 0. { 100. * 2_f64.powf(-log.ceil()) } else { 100. / 2_f64.powf(log.ceil()) };
				let ruler_spacing = ruler_interval * document_transform_scale;

				let ruler_origin = self.document_legacy.root.transform.transform_point2(DVec2::ZERO);

				responses.add(FrontendMessage::UpdateDocumentScrollbars {
					position: scrollbar_position.into(),
					size: scrollbar_size.into(),
					multiplier: scrollbar_multiplier.into(),
				});

				responses.add(FrontendMessage::UpdateDocumentRulers {
					origin: ruler_origin.into(),
					spacing: ruler_spacing,
					interval: ruler_interval,
				});
			}
			RollbackTransaction => {
				self.rollback(responses).unwrap_or_else(|e| warn!("{}", e));
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
				let mut paths = vec![];
				let last_selection_exists = !self.layer_range_selection_reference.is_empty();

				// If we have shift pressed and a layer already selected then fill the range
				if shift && last_selection_exists {
					// Fill the selection range
					self.layer_metadata
						.iter()
						.filter(|(target, _)| self.document_legacy.layer_is_between(target, &layer_path, &self.layer_range_selection_reference))
						.for_each(|(layer_path, _)| {
							paths.push(layer_path.clone());
						});
				} else {
					if ctrl {
						// Toggle selection when holding ctrl
						let layer = self.layer_metadata_mut(&layer_path);
						layer.selected = !layer.selected;
						responses.add(LayerChanged {
							affected_layer_path: layer_path.clone(),
						});
						responses.add(BroadcastEvent::SelectionChanged);
					} else {
						paths.push(layer_path.clone());
					}

					// Set our last selection reference
					self.layer_range_selection_reference = layer_path;
				}

				// Don't create messages for empty operations
				if !paths.is_empty() {
					// Add or set our selected layers
					if ctrl {
						responses.add_front(AddSelectedLayers { additional_layers: paths });
					} else {
						responses.add_front(SetSelectedLayers { replacement_selected_layers: paths });
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
						warn!("Setting blob URL for invalid layer type, which must be a `Layer` layer type. Found: `{:?}`", other);
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
			SetSelectedLayers { replacement_selected_layers } => {
				let selected = self.layer_metadata.iter_mut().filter(|(_, layer_metadata)| layer_metadata.selected);
				selected.for_each(|(path, layer_metadata)| {
					layer_metadata.selected = false;
					responses.add(LayerChanged { affected_layer_path: path.clone() })
				});

				let additional_layers = replacement_selected_layers;
				responses.add_front(AddSelectedLayers { additional_layers });
			}
			SetSnapping { snap } => {
				self.snapping_enabled = snap;
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
			UngroupLayers { folder_path } => {
				// Select all the children of the folder
				let select = self.document_legacy.folder_children_paths(&folder_path);

				let message_buffer: [Message; 4] = [
					// Select them
					DocumentMessage::SetSelectedLayers { replacement_selected_layers: select }.into(),
					// Copy them
					PortfolioMessage::Copy { clipboard: Clipboard::Internal }.into(),
					// Paste them into the folder above
					PortfolioMessage::PasteIntoFolder {
						clipboard: Clipboard::Internal,
						folder_path: folder_path[..folder_path.len() - 1].to_vec(),
						insert_index: -1,
					}
					.into(),
					// Delete the parent folder
					DocumentMessage::DeleteLayer { layer_path: folder_path }.into(),
				];

				// Push these messages in reverse due to push_front
				for message in message_buffer.into_iter().rev() {
					responses.add_front(message);
				}
			}
			UngroupSelectedLayers => {
				responses.add(DocumentMessage::StartTransaction);
				let folder_paths = self.document_legacy.sorted_folders_by_depth(self.selected_layers());
				for folder_path in folder_paths {
					responses.add(DocumentMessage::UngroupLayers { folder_path: folder_path.to_vec() });
				}
				responses.add(DocumentMessage::CommitTransaction);
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
				if let Some(bounds) = self.document_bounds(&render_data) {
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

		if self.layer_metadata.values().any(|data| data.selected) {
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
		common.extend(self.node_graph_handler.actions());
		common
	}
}

impl DocumentMessageHandler {
	pub fn rasterize_region_below_layer(
		&mut self,
		document_id: u64,
		layer_path: Vec<LayerId>,
		_preferences: &PreferencesMessageHandler,
		persistent_data: &PersistentData,
		imaginate_node_path: Option<Vec<NodeId>>,
	) -> Option<Message> {
		// Prepare the node graph input image

		let Some(node_network) = self.document_legacy.layer(&layer_path).ok().and_then(|layer| layer.as_layer_network().ok()) else {
			return None;
		};

		// Check if we use the "Input Frame" node.
		// TODO: Remove once rasterization is moved into a node.
		let input_frame_node_id = node_network.nodes.iter().find(|(_, node)| node.name == "Input Frame").map(|(&id, _)| id);
		let input_frame_connected_to_graph_output = input_frame_node_id.map_or(false, |target_node_id| node_network.connected_to_output(target_node_id, imaginate_node_path.is_none()));

		// If the Input Frame node is connected upstream, rasterize the artwork below this layer by calling into JS
		let response = if input_frame_connected_to_graph_output {
			let old_transforms = self.remove_document_transform();

			// Calculate the size of the region to be exported and generate an SVG of the artwork below this layer within that region
			let transform = self.document_legacy.multiply_transforms(&layer_path).unwrap();
			let size = DVec2::new(transform.transform_vector2(DVec2::new(1., 0.)).length(), transform.transform_vector2(DVec2::new(0., 1.)).length());
			// TODO: Fix this hack
			// This is a hack to prevent the compiler from optimizing out the size calculation which likely is due
			// to undefined behavior. THIS IS NOT A FIX.
			log::trace!("size: {:?}", size);
			let svg = self.render_document(size, transform.inverse(), persistent_data, DocumentRenderMode::OnlyBelowLayerInFolder(&layer_path));

			self.restore_document_transform(old_transforms);

			// Once JS asynchronously rasterizes the SVG, it will call the `PortfolioMessage::RenderGraphUsingRasterizedRegionBelowLayer` message with the rasterized image data
			FrontendMessage::TriggerRasterizeRegionBelowLayer {
				document_id,
				layer_path,
				svg,
				size,
				imaginate_node_path,
			}
			.into()
		}
		// Skip taking a round trip through JS since there's nothing to rasterize, and instead directly call the message which would otherwise be called asynchronously from JS
		else {
			PortfolioMessage::RenderGraphUsingRasterizedRegionBelowLayer {
				document_id,
				layer_path,
				input_image_data: vec![],
				size: (0, 0),
				imaginate_node_path,
			}
			.into()
		};
		Some(response)
	}

	/// Remove the artwork and artboard pan/tilt/zoom to render it without the user's viewport navigation, and save it to be restored at the end
	pub(crate) fn remove_document_transform(&mut self) -> [DAffine2; 2] {
		let old_artwork_transform = self.document_legacy.root.transform;
		self.document_legacy.root.transform = DAffine2::IDENTITY;
		DocumentLegacy::mark_children_as_dirty(&mut self.document_legacy.root);

		let old_artboard_transform = self.artboard_message_handler.artboards_document.root.transform;
		self.artboard_message_handler.artboards_document.root.transform = DAffine2::IDENTITY;
		DocumentLegacy::mark_children_as_dirty(&mut self.artboard_message_handler.artboards_document.root);

		[old_artwork_transform, old_artboard_transform]
	}

	/// Transform the artwork and artboard back to their original scales
	pub(crate) fn restore_document_transform(&mut self, [old_artwork_transform, old_artboard_transform]: [DAffine2; 2]) {
		self.document_legacy.root.transform = old_artwork_transform;
		DocumentLegacy::mark_children_as_dirty(&mut self.document_legacy.root);

		self.artboard_message_handler.artboards_document.root.transform = old_artboard_transform;
		DocumentLegacy::mark_children_as_dirty(&mut self.artboard_message_handler.artboards_document.root);
	}

	pub fn render_document(&mut self, size: DVec2, transform: DAffine2, persistent_data: &PersistentData, render_mode: DocumentRenderMode) -> String {
		// Render the document SVG code

		let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::Normal, None);

		let (artwork, outside) = match render_mode {
			DocumentRenderMode::Root => (self.document_legacy.render_root(&render_data), None),
			DocumentRenderMode::OnlyBelowLayerInFolder(below_layer_path) => (self.document_legacy.render_layers_below(below_layer_path, &render_data).unwrap(), None),
			DocumentRenderMode::LayerCutout(layer_path, background) => (self.document_legacy.render_layer(layer_path, &render_data).unwrap(), Some(background)),
		};
		let artboards = self.artboard_message_handler.artboards_document.render_root(&render_data);
		let outside_artboards_color = outside.map_or_else(
			|| if self.artboard_message_handler.artboard_ids.is_empty() { "ffffff" } else { "222222" }.to_string(),
			|col| col.rgba_hex(),
		);
		let outside_artboards = format!(r##"<rect x="0" y="0" width="100%" height="100%" fill="#{}" />"##, outside_artboards_color);
		let matrix = transform
			.to_cols_array()
			.iter()
			.enumerate()
			.fold(String::new(), |acc, (i, entry)| acc + &(entry.to_string() + if i == 5 { "" } else { "," }));
		let svg = format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" preserveAspectRatio="none" viewBox="0 0 1 1" width="{}" height="{}">{}{}<g transform="matrix({})">{}{}</g></svg>"#,
			size.x, size.y, "\n", outside_artboards, matrix, artboards, artwork
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

	pub fn with_name(name: String, ipp: &InputPreprocessorMessageHandler) -> Self {
		let mut document = Self { name, ..Self::default() };
		let starting_root_transform = document.navigation_handler.calculate_offset_transform(ipp.viewport_bounds.size() / 2.);
		document.document_legacy.root.transform = starting_root_transform;
		document.artboard_message_handler.artboards_document.root.transform = starting_root_transform;
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
			warn!("Tried to select non existing layer {:?}", path);
			None
		}
	}

	pub fn selected_visible_layers_bounding_box(&self, render_data: &RenderData) -> Option<[DVec2; 2]> {
		let paths = self.selected_visible_layers();
		self.document_legacy.combined_viewport_bounding_box(paths, render_data)
	}

	pub fn artboard_bounding_box_and_transform(&self, path: &[LayerId], render_data: &RenderData) -> Option<([DVec2; 2], DAffine2)> {
		self.artboard_message_handler.artboards_document.bounding_box_and_transform(path, render_data).unwrap_or(None)
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

	/// Returns the bounding boxes for all visible layers and artboards, optionally excluding any paths.
	pub fn bounding_boxes<'a>(&'a self, ignore_document: Option<&'a Vec<Vec<LayerId>>>, ignore_artboard: Option<LayerId>, render_data: &'a RenderData) -> impl Iterator<Item = [DVec2; 2]> + 'a {
		self.visible_layers()
			.filter(move |path| ignore_document.map_or(true, |ignore_document| !ignore_document.iter().any(|ig| ig.as_slice() == *path)))
			.filter_map(|path| self.document_legacy.viewport_bounding_box(path, render_data).ok()?)
			.chain(
				self.artboard_message_handler
					.artboard_ids
					.iter()
					.filter(move |&&id| Some(id) != ignore_artboard)
					.filter_map(|&path| self.artboard_message_handler.artboards_document.viewport_bounding_box(&[path], render_data).ok()?),
			)
	}

	fn serialize_structure(&self, folder: &FolderLayer, structure: &mut Vec<u64>, data: &mut Vec<LayerId>, path: &mut Vec<LayerId>) {
		let mut space = 0;
		for (id, layer) in folder.layer_ids.iter().zip(folder.layers()).rev() {
			data.push(*id);
			space += 1;
			if let LayerDataType::Folder(ref folder) = layer.data {
				path.push(*id);
				if self.layer_metadata(path).expanded {
					structure.push(space);
					self.serialize_structure(folder, structure, data, path);
					space = 0;
				}
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
		self.serialize_structure(self.document_legacy.root.as_folder().unwrap(), &mut structure, &mut data, &mut vec![]);
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
			.filter_map(|path| (!path.is_empty()).then_some(path))
			.filter_map(|path| {
				// TODO: `indices_for_path` can return an error. We currently skip these layers and log a warning. Once this problem is solved this code can be simplified.
				match self.document_legacy.indices_for_path(path) {
					Err(err) => {
						warn!("layers_sorted: Could not get indices for the layer {:?}: {:?}", path, err);
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
		self.layer_metadata.get(path).unwrap_or_else(|| panic!("Editor's layer metadata for {:?} does not exist", path))
	}

	pub fn layer_metadata_mut(&mut self, path: &[LayerId]) -> &mut LayerMetadata {
		Self::layer_metadata_mut_no_borrow_self(&mut self.layer_metadata, path)
	}

	pub fn layer_metadata_mut_no_borrow_self<'a>(layer_metadata: &'a mut HashMap<Vec<LayerId>, LayerMetadata>, path: &[LayerId]) -> &'a mut LayerMetadata {
		layer_metadata
			.get_mut(path)
			.unwrap_or_else(|| panic!("Layer data cannot be found because the path {:?} does not exist", path))
	}

	/// Places a document into the history system
	fn backup_with_document(&mut self, document: DocumentLegacy, artboard: ArtboardMessageHandler, layer_metadata: HashMap<Vec<LayerId>, LayerMetadata>, responses: &mut VecDeque<Message>) {
		self.document_redo_history.clear();
		self.document_undo_history.push_back(DocumentSave { document, artboard, layer_metadata });
		if self.document_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			self.document_undo_history.pop_front();
		}

		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
	}

	/// Copies the entire document into the history system
	pub fn backup(&mut self, responses: &mut VecDeque<Message>) {
		self.backup_with_document(self.document_legacy.clone(), self.artboard_message_handler.clone(), self.layer_metadata.clone(), responses);
	}

	/// Push a message backing up the document in its current state
	pub fn backup_nonmut(&self, responses: &mut VecDeque<Message>) {
		responses.add(DocumentMessage::BackupDocument {
			document: self.document_legacy.clone(),
			artboard: Box::new(self.artboard_message_handler.clone()),
			layer_metadata: self.layer_metadata.clone(),
		});
	}

	pub fn rollback(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		self.backup(responses);
		self.undo(responses)
		// TODO: Consider if we should check if the document is saved
	}

	/// Replace the document with a new document save, returning the document save.
	pub fn replace_document(&mut self, DocumentSave { document, artboard, layer_metadata }: DocumentSave) -> DocumentSave {
		// Keeping the root is required if the bounds of the viewport have changed during the operation
		let old_root = self.document_legacy.root.transform;
		let old_artboard_root = self.artboard_message_handler.artboards_document.root.transform;
		let document = std::mem::replace(&mut self.document_legacy, document);
		let artboard = std::mem::replace(&mut self.artboard_message_handler, artboard);
		self.document_legacy.root.transform = old_root;
		self.artboard_message_handler.artboards_document.root.transform = old_artboard_root;
		self.document_legacy.root.cache_dirty = true;
		self.artboard_message_handler.artboards_document.root.cache_dirty = true;

		let layer_metadata = std::mem::replace(&mut self.layer_metadata, layer_metadata);

		DocumentSave { document, artboard, layer_metadata }
	}

	pub fn undo(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);

		let selected_paths: Vec<Vec<LayerId>> = self.selected_layers().map(|path| path.to_vec()).collect();

		match self.document_undo_history.pop_back() {
			Some(DocumentSave { document, artboard, layer_metadata }) => {
				// Update the currently displayed layer on the Properties panel if the selection changes after an undo action
				// Also appropriately update the Properties panel if an undo action results in a layer being deleted
				let prev_selected_paths: Vec<Vec<LayerId>> = layer_metadata.iter().filter_map(|(layer_id, metadata)| metadata.selected.then_some(layer_id.clone())).collect();

				if prev_selected_paths != selected_paths {
					responses.add(BroadcastEvent::SelectionChanged);
				}

				let document_save = self.replace_document(DocumentSave { document, artboard, layer_metadata });

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
			Some(DocumentSave { document, artboard, layer_metadata }) => {
				// Update currently displayed layer on property panel if selection changes after redo action
				// Also appropriately update property panel if redo action results in a layer being added
				let next_selected_paths: Vec<Vec<LayerId>> = layer_metadata.iter().filter_map(|(layer_id, metadata)| metadata.selected.then_some(layer_id.clone())).collect();

				if next_selected_paths != selected_paths {
					responses.add(BroadcastEvent::SelectionChanged);
				}

				let document_save = self.replace_document(DocumentSave { document, artboard, layer_metadata });
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
			.ok_or_else(|| EditorError::Document(format!("Could not get layer metadata for {:?}", path)))?;
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
		let transform = self.document_legacy.generate_transform_across_scope(path, Some(self.document_legacy.root.transform.inverse())).ok()?;
		let layer = self.document_legacy.layer(path).ok()?;

		Some(LayerPanelEntry::new(layer_metadata, transform, layer, path.to_vec(), render_data))
	}

	/// When working with an insert index, deleting the layers may cause the insert index to point to a different location (if the layer being deleted was located before the insert index).
	///
	/// This function updates the insert index so that it points to the same place after the specified `layers` are deleted.
	fn update_insert_index(&self, layers: &[&[LayerId]], path: &[LayerId], insert_index: isize, reverse_index: bool) -> Result<isize, DocumentError> {
		let folder = self.document_legacy.folder(path)?;
		let insert_index = if reverse_index { folder.layer_ids.len() as isize - insert_index } else { insert_index };
		let layer_ids_above = if insert_index < 0 { &folder.layer_ids } else { &folder.layer_ids[..(insert_index as usize)] };

		Ok(insert_index - layer_ids_above.iter().filter(|layer_id| layers.iter().any(|x| *x == [path, &[**layer_id]].concat())).count() as isize)
	}

	/// Calculates the bounding box of all layers in the document
	pub fn all_layer_bounds(&self, render_data: &RenderData) -> Option<[DVec2; 2]> {
		self.document_legacy.viewport_bounding_box(&[], render_data).ok().flatten()
	}

	/// Calculates the document bounds used for scrolling and centring (the layer bounds or the artboard (if applicable))
	pub fn document_bounds(&self, render_data: &RenderData) -> Option<[DVec2; 2]> {
		if self.artboard_message_handler.is_infinite_canvas() {
			self.all_layer_bounds(render_data)
		} else {
			self.artboard_message_handler.artboards_document.viewport_bounding_box(&[], render_data).ok().flatten()
		}
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

	/// Loads layer resources such as creating the blob URLs for the images and loading all of the fonts in the document
	pub fn load_layer_resources(&self, responses: &mut VecDeque<Message>, root: &LayerDataType, mut path: Vec<LayerId>, _document_id: u64) {
		fn walk_layers(data: &LayerDataType, path: &mut Vec<LayerId>, responses: &mut VecDeque<Message>, fonts: &mut HashSet<Font>) {
			match data {
				LayerDataType::Folder(folder) => {
					for (id, layer) in folder.layer_ids.iter().zip(folder.layers().iter()) {
						path.push(*id);
						walk_layers(&layer.data, path, responses, fonts);
						path.pop();
					}
				}
				LayerDataType::Layer(layer) => {
					if layer.cached_output_data == CachedOutputData::None {
						responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path: path.clone() });
					}
					for node in layer.network.nodes.values() {
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
				}
				_ => {}
			}
		}

		let mut fonts = HashSet::new();
		walk_layers(root, &mut path, responses, &mut fonts);
		for font in fonts {
			responses.add_front(FrontendMessage::TriggerFontLoad { font, is_default: false });
		}
	}

	pub fn update_document_widgets(&self, responses: &mut VecDeque<Message>) {
		let mut widgets = vec![
			WidgetHolder::new(Widget::OptionalInput(OptionalInput {
				checked: self.snapping_enabled,
				icon: "Snapping".into(),
				tooltip: "Snapping".into(),
				on_update: WidgetCallback::new(|optional_input: &OptionalInput| DocumentMessage::SetSnapping { snap: optional_input.checked }.into()),
				..Default::default()
			})),
			WidgetHolder::new(Widget::PopoverButton(PopoverButton {
				header: "Snapping".into(),
				text: "Coming soon".into(),
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::OptionalInput(OptionalInput {
				checked: true,
				icon: "Grid".into(),
				tooltip: "Grid".into(),
				on_update: WidgetCallback::new(|_| DialogMessage::RequestComingSoonDialog { issue: Some(318) }.into()),
				..Default::default()
			})),
			WidgetHolder::new(Widget::PopoverButton(PopoverButton {
				header: "Grid".into(),
				text: "Coming soon".into(),
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::OptionalInput(OptionalInput {
				checked: self.overlays_visible,
				icon: "Overlays".into(),
				tooltip: "Overlays".into(),
				on_update: WidgetCallback::new(|optional_input: &OptionalInput| DocumentMessage::SetOverlaysVisibility { visible: optional_input.checked }.into()),
				..Default::default()
			})),
			WidgetHolder::new(Widget::PopoverButton(PopoverButton {
				header: "Overlays".into(),
				text: "Coming soon".into(),
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::RadioInput(RadioInput {
				selected_index: match self.view_mode {
					ViewMode::Normal => 0,
					_ => 1,
				},
				entries: vec![
					RadioEntryData {
						value: "normal".into(),
						icon: "ViewModeNormal".into(),
						tooltip: "View Mode: Normal".into(),
						on_update: WidgetCallback::new(|_| DocumentMessage::SetViewMode { view_mode: ViewMode::Normal }.into()),
						..RadioEntryData::default()
					},
					RadioEntryData {
						value: "outline".into(),
						icon: "ViewModeOutline".into(),
						tooltip: "View Mode: Outline".into(),
						on_update: WidgetCallback::new(|_| DocumentMessage::SetViewMode { view_mode: ViewMode::Outline }.into()),
						..RadioEntryData::default()
					},
					RadioEntryData {
						value: "pixels".into(),
						icon: "ViewModePixels".into(),
						tooltip: "View Mode: Pixels".into(),
						on_update: WidgetCallback::new(|_| DialogMessage::RequestComingSoonDialog { issue: Some(320) }.into()),
						..RadioEntryData::default()
					},
				],
				..Default::default()
			})),
			WidgetHolder::new(Widget::PopoverButton(PopoverButton {
				header: "View Mode".into(),
				text: "Coming soon".into(),
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Section,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::IconButton(IconButton {
				size: 24,
				icon: "ZoomIn".into(),
				tooltip: "Zoom In".into(),
				tooltip_shortcut: action_keys!(NavigationMessageDiscriminant::IncreaseCanvasZoom),
				on_update: WidgetCallback::new(|_| NavigationMessage::IncreaseCanvasZoom { center_on_mouse: false }.into()),
				..IconButton::default()
			})),
			WidgetHolder::new(Widget::IconButton(IconButton {
				size: 24,
				icon: "ZoomOut".into(),
				tooltip: "Zoom Out".into(),
				tooltip_shortcut: action_keys!(NavigationMessageDiscriminant::DecreaseCanvasZoom),
				on_update: WidgetCallback::new(|_| NavigationMessage::DecreaseCanvasZoom { center_on_mouse: false }.into()),
				..IconButton::default()
			})),
			WidgetHolder::new(Widget::IconButton(IconButton {
				size: 24,
				icon: "ZoomReset".into(),
				tooltip: "Zoom to 100%".into(),
				tooltip_shortcut: action_keys!(DocumentMessageDiscriminant::ZoomCanvasTo100Percent),
				on_update: WidgetCallback::new(|_| NavigationMessage::SetCanvasZoom { zoom_factor: 1. }.into()),
				..IconButton::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: "%".into(),
				value: Some(self.navigation_handler.snapped_scale() * 100.),
				min: Some(0.000001),
				max: Some(1000000.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| {
					NavigationMessage::SetCanvasZoom {
						zoom_factor: number_input.value.unwrap() / 100.,
					}
					.into()
				}),
				increment_behavior: NumberInputIncrementBehavior::Callback,
				increment_callback_decrease: WidgetCallback::new(|_| NavigationMessage::DecreaseCanvasZoom { center_on_mouse: false }.into()),
				increment_callback_increase: WidgetCallback::new(|_| NavigationMessage::IncreaseCanvasZoom { center_on_mouse: false }.into()),
				..NumberInput::default()
			})),
		];
		let rotation_value = self.navigation_handler.snapped_angle() / (std::f64::consts::PI / 180.);
		if rotation_value.abs() > 0.00001 {
			widgets.extend([
				WidgetHolder::new(Widget::Separator(Separator {
					separator_type: SeparatorType::Related,
					direction: SeparatorDirection::Horizontal,
				})),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					unit: "°".into(),
					value: Some(rotation_value),
					step: 15.,
					on_update: WidgetCallback::new(|number_input: &NumberInput| {
						NavigationMessage::SetCanvasRotation {
							angle_radians: number_input.value.unwrap() * (std::f64::consts::PI / 180.),
						}
						.into()
					}),
					..NumberInput::default()
				})),
			]);
		}
		widgets.extend([
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::PopoverButton(PopoverButton {
				header: "Canvas Navigation".into(),
				text: "Interactive options in this popover menu are coming soon.\nZoom with Shift + MMB Drag or Ctrl + Scroll Wheel Roll.\nRotate with Ctrl + MMB Drag.".into(),
				..Default::default()
			})),
		]);
		let document_bar_layout = WidgetLayout::new(vec![LayoutGroup::Row { widgets }]);

		let document_mode_layout = WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![
				WidgetHolder::new(Widget::DropdownInput(DropdownInput {
					entries: vec![vec![
						DropdownEntryData {
							label: DocumentMode::DesignMode.to_string(),
							icon: DocumentMode::DesignMode.icon_name(),
							..DropdownEntryData::default()
						},
						DropdownEntryData {
							label: DocumentMode::SelectMode.to_string(),
							icon: DocumentMode::SelectMode.icon_name(),
							on_update: WidgetCallback::new(|_| DialogMessage::RequestComingSoonDialog { issue: Some(330) }.into()),
							..DropdownEntryData::default()
						},
						DropdownEntryData {
							label: DocumentMode::GuideMode.to_string(),
							icon: DocumentMode::GuideMode.icon_name(),
							on_update: WidgetCallback::new(|_| DialogMessage::RequestComingSoonDialog { issue: Some(331) }.into()),
							..DropdownEntryData::default()
						},
					]],
					selected_index: Some(self.document_mode as u32),
					draw_icon: true,
					interactive: false, // TODO: set to true when dialogs are not spawned
					..Default::default()
				})),
				WidgetHolder::new(Widget::Separator(Separator {
					separator_type: SeparatorType::Section,
					direction: SeparatorDirection::Horizontal,
				})),
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
					.map(|mode| DropdownEntryData {
						label: mode.to_string(),
						value: mode.to_string(),
						on_update: WidgetCallback::new(|_| DocumentMessage::SetBlendModeForSelectedLayers { blend_mode: *mode }.into()),
						..Default::default()
					})
					.collect()
			})
			.collect();

		let layer_tree_options = WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![
				WidgetHolder::new(Widget::DropdownInput(DropdownInput {
					entries: blend_mode_menu_entries,
					selected_index: blend_mode.map(|blend_mode| blend_mode as u32),
					disabled: blend_mode.is_none() && !blend_mode_is_mixed,
					draw_icon: false,
					..Default::default()
				})),
				WidgetHolder::new(Widget::Separator(Separator {
					separator_type: SeparatorType::Related,
					direction: SeparatorDirection::Horizontal,
				})),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					label: "Opacity".into(),
					unit: "%".into(),
					display_decimal_places: 2,
					disabled: opacity.is_none() && !opacity_is_mixed,
					value: opacity.map(|opacity| opacity * 100.),
					min: Some(0.),
					max: Some(100.),
					range_min: Some(0.),
					range_max: Some(100.),
					mode: NumberInputMode::Range,
					on_update: WidgetCallback::new(|number_input: &NumberInput| {
						if let Some(value) = number_input.value {
							DocumentMessage::SetOpacityForSelectedLayers { opacity: value / 100. }.into()
						} else {
							Message::NoOp
						}
					}),
					..NumberInput::default()
				})),
				WidgetHolder::new(Widget::Separator(Separator {
					separator_type: SeparatorType::Section,
					direction: SeparatorDirection::Horizontal,
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "Folder".into(),
					tooltip: "New Folder".into(),
					tooltip_shortcut: action_keys!(DocumentMessageDiscriminant::CreateEmptyFolder),
					size: 24,
					on_update: WidgetCallback::new(|_| DocumentMessage::CreateEmptyFolder { container_path: vec![] }.into()),
					..Default::default()
				})),
				WidgetHolder::new(Widget::IconButton(IconButton {
					icon: "Trash".into(),
					tooltip: "Delete Selected".into(),
					tooltip_shortcut: action_keys!(DocumentMessageDiscriminant::DeleteSelectedLayers),
					size: 24,
					on_update: WidgetCallback::new(|_| DocumentMessage::DeleteSelectedLayers.into()),
					..Default::default()
				})),
			],
		}]);

		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(layer_tree_options),
			layout_target: LayoutTarget::LayerTreeOptions,
		});
	}

	pub fn selected_layers_reorder(&mut self, relative_index_offset: isize, responses: &mut VecDeque<Message>) {
		self.backup(responses);

		let all_layer_paths = self.all_layers_sorted();
		let selected_layers = self.selected_layers_sorted();

		let first_or_last_selected_layer = match relative_index_offset.signum() {
			-1 => selected_layers.first(),
			1 => selected_layers.last(),
			_ => panic!("selected_layers_reorder() must be given a non-zero value"),
		};

		if let Some(pivot_layer) = first_or_last_selected_layer {
			let sibling_layer_paths: Vec<_> = all_layer_paths
				.iter()
				.filter(|layer| {
					// Check if this is a sibling of the pivot layer
					// TODO: Break this out into a reusable function `fn are_layers_siblings(layer_a, layer_b) -> bool`
					let containing_folder_path = &pivot_layer[0..pivot_layer.len() - 1];
					layer.starts_with(containing_folder_path) && pivot_layer.len() == layer.len()
				})
				.collect();

			// TODO: Break this out into a reusable function: `fn layer_index_in_containing_folder(layer_path) -> usize`
			let pivot_index_among_siblings = sibling_layer_paths.iter().position(|path| *path == pivot_layer);

			if let Some(pivot_index) = pivot_index_among_siblings {
				let max = sibling_layer_paths.len() as i64 - 1;
				let insert_index = (pivot_index as i64 + relative_index_offset as i64).clamp(0, max) as usize;

				let existing_layer_to_insert_beside = sibling_layer_paths.get(insert_index);

				// TODO: Break this block out into a call to a message called `MoveSelectedLayersNextToLayer { neighbor_path, above_or_below }`
				if let Some(neighbor_path) = existing_layer_to_insert_beside {
					let (neighbor_id, folder_path) = neighbor_path.split_last().expect("Can't move the root folder");

					if let Some(folder) = self.document_legacy.layer(folder_path).ok().and_then(|layer| layer.as_folder().ok()) {
						let neighbor_layer_index = folder.layer_ids.iter().position(|id| id == neighbor_id).unwrap() as isize;

						// If moving down, insert below this layer. If moving up, insert above this layer.
						let insert_index = if relative_index_offset < 0 { neighbor_layer_index } else { neighbor_layer_index + 1 };

						responses.add(DocumentMessage::MoveSelectedLayersTo {
							folder_path: folder_path.to_vec(),
							insert_index,
							reverse_index: false,
						});
					}
				}
			}
		}
	}
}
