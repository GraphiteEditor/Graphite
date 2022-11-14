use super::utility_types::PersistentData;
use crate::application::generate_uuid;
use crate::consts::{DEFAULT_DOCUMENT_NAME, GRAPHITE_DOCUMENT_VERSION};
use crate::messages::dialog::simple_dialogs;
use crate::messages::frontend::utility_types::{FrontendDocumentDetails, FrontendImageData};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::portfolio::document::utility_types::clipboards::{Clipboard, CopyBufferEntry, INTERNAL_CLIPBOARD_COUNT};
use crate::messages::portfolio::utility_types::ImaginateServerStatus;
use crate::messages::prelude::*;

use graphene::layers::layer_info::{LayerDataType, LayerDataTypeDiscriminant};
use graphene::layers::text_layer::Font;
use graphene::{LayerId, Operation as DocumentOperation};

#[derive(Debug, Clone, Default)]
pub struct PortfolioMessageHandler {
	menu_bar_message_handler: MenuBarMessageHandler,
	documents: HashMap<u64, DocumentMessageHandler>,
	document_ids: Vec<u64>,
	active_document_id: Option<u64>,
	copy_buffer: [Vec<CopyBufferEntry>; INTERNAL_CLIPBOARD_COUNT as usize],
	pub persistent_data: PersistentData,
}

impl MessageHandler<PortfolioMessage, (&InputPreprocessorMessageHandler, &PreferencesMessageHandler)> for PortfolioMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PortfolioMessage, (ipp, preferences): (&InputPreprocessorMessageHandler, &PreferencesMessageHandler), responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			PortfolioMessage::MenuBar(message) => self.menu_bar_message_handler.process_message(message, (), responses),
			#[remain::unsorted]
			PortfolioMessage::Document(message) => {
				if let Some(document_id) = self.active_document_id {
					if let Some(document) = self.documents.get_mut(&document_id) {
						document.process_message(message, (document_id, ipp, &self.persistent_data, preferences), responses)
					}
				}
			}

			// Messages
			#[remain::unsorted]
			PortfolioMessage::DocumentPassMessage { document_id, message } => {
				if let Some(document) = self.documents.get_mut(&document_id) {
					document.process_message(message, (document_id, ipp, &self.persistent_data, preferences), responses)
				}
			}
			PortfolioMessage::AutoSaveActiveDocument => {
				if let Some(document_id) = self.active_document_id {
					if let Some(document) = self.active_document_mut() {
						document.set_auto_save_state(true);
					}
					responses.push_back(PortfolioMessage::AutoSaveDocument { document_id }.into());
				}
			}
			PortfolioMessage::AutoSaveDocument { document_id } => {
				let document = self.documents.get(&document_id).unwrap();
				responses.push_back(
					FrontendMessage::TriggerIndexedDbWriteDocument {
						document: document.serialize_document(),
						details: FrontendDocumentDetails {
							is_auto_saved: document.is_auto_saved(),
							is_saved: document.is_saved(),
							id: document_id,
							name: document.name.clone(),
						},
						version: GRAPHITE_DOCUMENT_VERSION.to_string(),
					}
					.into(),
				)
			}
			PortfolioMessage::CloseActiveDocumentWithConfirmation => {
				if let Some(document_id) = self.active_document_id {
					responses.push_back(PortfolioMessage::CloseDocumentWithConfirmation { document_id }.into());
				}
			}
			PortfolioMessage::CloseAllDocuments => {
				if self.active_document_id.is_some() {
					responses.push_back(PropertiesPanelMessage::Deactivate.into());
					responses.push_back(BroadcastEvent::ToolAbort.into());
					responses.push_back(ToolMessage::DeactivateTools.into());
				}

				for document_id in &self.document_ids {
					responses.push_back(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id: *document_id }.into());
				}

				responses.push_back(PortfolioMessage::DestroyAllDocuments.into());
				responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
			}
			PortfolioMessage::CloseDocument { document_id } => {
				let document_index = self.document_index(document_id);
				self.documents.remove(&document_id);
				self.document_ids.remove(document_index);

				if self.document_ids.is_empty() {
					self.active_document_id = None;
				} else if self.active_document_id.is_some() {
					let document_id = if document_index == self.document_ids.len() {
						// If we closed the last document take the one previous (same as last)
						*self.document_ids.last().unwrap()
					} else {
						// Move to the next tab
						self.document_ids[document_index]
					};
					responses.push_back(PortfolioMessage::SelectDocument { document_id }.into());
				}

				// Send the new list of document tab names
				responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
				responses.push_back(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id }.into());
				responses.push_back(DocumentMessage::RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				if let Some(document) = self.active_document() {
					for layer in document.layer_metadata.keys() {
						responses.push_back(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() }.into());
					}
				}
			}
			PortfolioMessage::CloseDocumentWithConfirmation { document_id } => {
				let target_document = self.documents.get(&document_id).unwrap();
				if target_document.is_saved() {
					responses.push_back(BroadcastEvent::ToolAbort.into());
					responses.push_back(PortfolioMessage::CloseDocument { document_id }.into());
				} else {
					let dialog = simple_dialogs::CloseDocumentDialog {
						document_name: target_document.name.clone(),
						document_id,
					};
					dialog.register_properties(responses, LayoutTarget::DialogDetails);
					responses.push_back(FrontendMessage::DisplayDialog { icon: "File".to_string() }.into());

					// Select the document being closed
					responses.push_back(PortfolioMessage::SelectDocument { document_id }.into());
				}
			}
			PortfolioMessage::Copy { clipboard } => {
				// We can't use `self.active_document()` because it counts as an immutable borrow of the entirety of `self`
				if let Some(active_document) = self.active_document_id.and_then(|id| self.documents.get(&id)) {
					let copy_val = |buffer: &mut Vec<CopyBufferEntry>| {
						for layer_path in active_document.selected_layers_without_children() {
							match (active_document.graphene_document.layer(layer_path).map(|t| t.clone()), *active_document.layer_metadata(layer_path)) {
								(Ok(layer), layer_metadata) => {
									buffer.push(CopyBufferEntry { layer, layer_metadata });
								}
								(Err(e), _) => warn!("Could not access selected layer {:?}: {:?}", layer_path, e),
							}
						}
					};

					if clipboard == Clipboard::Device {
						let mut buffer = Vec::new();
						copy_val(&mut buffer);
						let mut copy_text = String::from("graphite/layer: ");
						copy_text += &serde_json::to_string(&buffer).expect("Could not serialize paste");

						responses.push_back(FrontendMessage::TriggerTextCopy { copy_text }.into());
					} else {
						let copy_buffer = &mut self.copy_buffer;
						copy_buffer[clipboard as usize].clear();
						copy_val(&mut copy_buffer[clipboard as usize]);
					}
				}
			}
			PortfolioMessage::Cut { clipboard } => {
				responses.push_back(PortfolioMessage::Copy { clipboard }.into());
				responses.push_back(DocumentMessage::DeleteSelectedLayers.into());
			}
			PortfolioMessage::DestroyAllDocuments => {
				// Empty the list of internal document data
				self.documents.clear();
				self.document_ids.clear();
				self.active_document_id = None;
			}
			PortfolioMessage::FontLoaded {
				font_family,
				font_style,
				preview_url,
				data,
				is_default,
			} => {
				self.persistent_data.font_cache.insert(Font::new(font_family, font_style), preview_url, data, is_default);

				if let Some(document) = self.active_document_mut() {
					document.graphene_document.mark_all_layers_of_type_as_dirty(LayerDataTypeDiscriminant::Text);
					responses.push_back(DocumentMessage::RenderDocument.into());
					responses.push_back(BroadcastEvent::DocumentIsDirty.into());
				}
			}
			PortfolioMessage::ImaginateCheckServerStatus => {
				self.persistent_data.imaginate_server_status = ImaginateServerStatus::Checking;
				responses.push_back(
					FrontendMessage::TriggerImaginateCheckServerStatus {
						hostname: preferences.imaginate_server_hostname.clone(),
					}
					.into(),
				);
				responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
			}
			PortfolioMessage::ImaginateSetBlobUrl {
				document_id,
				layer_path,
				blob_url,
				resolution,
			} => {
				if let Some(document) = self.documents.get_mut(&document_id) {
					if let Ok(layer) = document.graphene_document.layer(&layer_path) {
						let previous_blob_url = &layer.as_imaginate().unwrap().blob_url;

						if let Some(url) = previous_blob_url {
							responses.push_back(FrontendMessage::TriggerRevokeBlobUrl { url: url.clone() }.into());
						}

						let message = DocumentOperation::SetLayerBlobUrl { layer_path, blob_url, resolution }.into();
						responses.push_back(PortfolioMessage::DocumentPassMessage { document_id, message }.into());
					}
				}
			}
			PortfolioMessage::ImaginateSetGeneratingStatus { document_id, path, percent, status } => {
				let message = DocumentOperation::ImaginateSetGeneratingStatus { path, percent, status }.into();
				responses.push_back(PortfolioMessage::DocumentPassMessage { document_id, message }.into());
			}
			PortfolioMessage::ImaginateSetImageData { document_id, layer_path, image_data } => {
				let message = DocumentOperation::ImaginateSetImageData { layer_path, image_data }.into();
				responses.push_back(PortfolioMessage::DocumentPassMessage { document_id, message }.into());
			}
			PortfolioMessage::ImaginateSetServerStatus { status } => {
				self.persistent_data.imaginate_server_status = status;
				responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
			}
			PortfolioMessage::Import => {
				// This portfolio message wraps the frontend message so it can be listed as an action, which isn't possible for frontend messages
				if self.active_document().is_some() {
					responses.push_back(FrontendMessage::TriggerImport.into());
				}
			}
			PortfolioMessage::LoadDocumentResources { document_id } => {
				if let Some(document) = self.document_mut(document_id) {
					document.load_layer_resources(responses, &document.graphene_document.root.data, Vec::new(), document_id);
				}
			}
			PortfolioMessage::LoadFont { font, is_default } => {
				if !self.persistent_data.font_cache.loaded_font(&font) {
					responses.push_front(FrontendMessage::TriggerFontLoad { font, is_default }.into());
				}
			}
			PortfolioMessage::NewDocumentWithName { name } => {
				let new_document = DocumentMessageHandler::with_name(name, ipp);
				let document_id = generate_uuid();
				if self.active_document().is_some() {
					responses.push_back(BroadcastEvent::ToolAbort.into());
					responses.push_back(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() }.into());
				}

				self.load_document(new_document, document_id, responses);
			}
			PortfolioMessage::NextDocument => {
				if let Some(active_document_id) = self.active_document_id {
					let current_index = self.document_index(active_document_id);
					let next_index = (current_index + 1) % self.document_ids.len();
					let next_id = self.document_ids[next_index];

					responses.push_back(PortfolioMessage::SelectDocument { document_id: next_id }.into());
				}
			}
			PortfolioMessage::OpenDocument => {
				// This portfolio message wraps the frontend message so it can be listed as an action, which isn't possible for frontend messages
				responses.push_back(FrontendMessage::TriggerOpenDocument.into());
			}
			PortfolioMessage::OpenDocumentFile {
				document_name,
				document_serialized_content,
			} => {
				responses.push_back(
					PortfolioMessage::OpenDocumentFileWithId {
						document_id: generate_uuid(),
						document_name,
						document_is_auto_saved: false,
						document_is_saved: true,
						document_serialized_content,
					}
					.into(),
				);
			}
			PortfolioMessage::OpenDocumentFileWithId {
				document_id,
				document_name,
				document_is_auto_saved,
				document_is_saved,
				document_serialized_content,
			} => {
				let document = DocumentMessageHandler::with_name_and_content(document_name, document_serialized_content);
				match document {
					Ok(mut document) => {
						document.set_auto_save_state(document_is_auto_saved);
						document.set_save_state(document_is_saved);
						self.load_document(document, document_id, responses);
					}
					Err(e) => responses.push_back(
						DialogMessage::DisplayDialogError {
							title: "Failed to open document".to_string(),
							description: e.to_string(),
						}
						.into(),
					),
				}
			}
			// TODO: Paste message is unused, delete it?
			PortfolioMessage::Paste { clipboard } => {
				let shallowest_common_folder = self.active_document().map(|document| {
					document
						.graphene_document
						.shallowest_common_folder(document.selected_layers())
						.expect("While pasting, the selected layers did not exist while attempting to find the appropriate folder path for insertion")
				});

				if let Some(folder) = shallowest_common_folder {
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(
						PortfolioMessage::PasteIntoFolder {
							clipboard,
							folder_path: folder.to_vec(),
							insert_index: -1,
						}
						.into(),
					);
					responses.push_back(DocumentMessage::CommitTransaction.into());
				}
			}
			PortfolioMessage::PasteIntoFolder {
				clipboard,
				folder_path: path,
				insert_index,
			} => {
				let paste = |entry: &CopyBufferEntry, responses: &mut VecDeque<_>| {
					if let Some(document) = self.active_document() {
						trace!("Pasting into folder {:?} as index: {}", &path, insert_index);
						let destination_path = [path.to_vec(), vec![generate_uuid()]].concat();

						responses.push_front(
							DocumentMessage::UpdateLayerMetadata {
								layer_path: destination_path.clone(),
								layer_metadata: entry.layer_metadata,
							}
							.into(),
						);
						document.load_layer_resources(responses, &entry.layer.data, destination_path.clone(), self.active_document_id.unwrap());
						responses.push_front(
							DocumentOperation::InsertLayer {
								layer: Box::new(entry.layer.clone()),
								destination_path,
								insert_index,
							}
							.into(),
						);
					}
				};

				if insert_index == -1 {
					for entry in self.copy_buffer[clipboard as usize].iter().rev() {
						paste(entry, responses)
					}
				} else {
					for entry in self.copy_buffer[clipboard as usize].iter() {
						paste(entry, responses)
					}
				}
			}
			PortfolioMessage::PasteSerializedData { data } => {
				if let Some(document) = self.active_document() {
					if let Ok(data) = serde_json::from_str::<Vec<CopyBufferEntry>>(&data) {
						let shallowest_common_folder = document
							.graphene_document
							.shallowest_common_folder(document.selected_layers())
							.expect("While pasting from serialized, the selected layers did not exist while attempting to find the appropriate folder path for insertion");
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(DocumentMessage::StartTransaction.into());

						for entry in data.iter().rev() {
							let destination_path = [shallowest_common_folder.to_vec(), vec![generate_uuid()]].concat();

							responses.push_front(
								DocumentMessage::UpdateLayerMetadata {
									layer_path: destination_path.clone(),
									layer_metadata: entry.layer_metadata,
								}
								.into(),
							);
							document.load_layer_resources(responses, &entry.layer.data, destination_path.clone(), self.active_document_id.unwrap());
							responses.push_front(
								DocumentOperation::InsertLayer {
									layer: Box::new(entry.layer.clone()),
									destination_path,
									insert_index: -1,
								}
								.into(),
							);
						}

						responses.push_back(DocumentMessage::CommitTransaction.into());
					}
				}
			}
			PortfolioMessage::PrevDocument => {
				if let Some(active_document_id) = self.active_document_id {
					let len = self.document_ids.len();
					let current_index = self.document_index(active_document_id);
					let prev_index = (current_index + len - 1) % len;
					let prev_id = self.document_ids[prev_index];
					responses.push_back(PortfolioMessage::SelectDocument { document_id: prev_id }.into());
				}
			}
			PortfolioMessage::ProcessNodeGraphFrame {
				document_id,
				layer_path,
				image_data,
				size,
			} => {
				fn read_image(document: Option<&DocumentMessageHandler>, layer_path: &[LayerId], image_data: Vec<u8>, (width, height): (u32, u32)) -> Result<Vec<u8>, String> {
					use graphene_std::raster::Image;
					use image::{ImageBuffer, Rgba};
					use std::io::Cursor;

					let data = image_data.chunks_exact(4).map(|v| graphene_core::raster::color::Color::from_rgba8(v[0], v[1], v[2], v[3])).collect();
					let image = graphene_std::raster::Image { width, height, data };

					let document = document.ok_or_else(|| "Invalid document".to_string())?;
					let layer = document.graphene_document.layer(layer_path).map_err(|e| format!("No layer: {e:?}"))?;
					let node_graph_frame = match &layer.data {
						LayerDataType::NodeGraphFrame(frame) => Ok(frame),
						_ => Err("Invalid layer type".to_string()),
					}?;

					// Execute the node graph

					let mut network = node_graph_frame.network.clone();
					info!("Executing network {network:#?}");

					let stack = borrow_stack::FixedSizeStack::new(256);
					for node_id in node_graph_frame.network.nodes.keys() {
						network.flatten(*node_id);
					}

					let mut proto_network = network.into_proto_network();
					proto_network.reorder_ids();

					info!("proto_network with reordered ids: {proto_network:#?}");

					assert_ne!(proto_network.nodes.len(), 0, "No protonodes exist?");
					for (_id, node) in proto_network.nodes {
						info!("Inserting proto node {:?}", node);
						graph_craft::node_registry::push_node(node, &stack);
					}

					use borrow_stack::BorrowStack;
					use dyn_any::IntoDynAny;
					use graphene_core::Node;

					let result = unsafe { stack.get().last().unwrap().eval(image.into_dyn()) };
					let result = *dyn_any::downcast::<Image>(result).unwrap();

					let mut bytes: Vec<u8> = Vec::new();
					let [result_width, result_height] = [result.width, result.height];
					let size_estimate = (result_width * result_height * 4) as usize;

					let mut result_bytes = Vec::with_capacity(size_estimate);
					result_bytes.extend(result.data.into_iter().flat_map(|colour| colour.to_rgba8()));
					let output: ImageBuffer<Rgba<u8>, _> = image::ImageBuffer::from_raw(result_width, result_height, result_bytes).ok_or_else(|| "Invalid image size".to_string())?;
					output.write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::Bmp).map_err(|e| e.to_string())?;

					Ok(bytes)
				}

				match read_image(self.documents.get(&document_id), &layer_path, image_data, size) {
					Ok(image_data) => {
						responses.push_back(
							DocumentOperation::SetNodeGraphFrameImageData {
								layer_path: layer_path.clone(),
								image_data: image_data.clone(),
							}
							.into(),
						);
						let mime = "image/bmp".to_string();
						let image_data = std::rc::Rc::new(image_data);
						responses.push_back(
							FrontendMessage::UpdateImageData {
								document_id,
								image_data: vec![FrontendImageData { path: layer_path, image_data, mime }],
							}
							.into(),
						);
					}
					Err(description) => responses.push_back(
						DialogMessage::DisplayDialogError {
							title: "Failed to update node graph".to_string(),
							description,
						}
						.into(),
					),
				}
			}
			PortfolioMessage::SelectDocument { document_id } => {
				if let Some(document) = self.active_document() {
					if !document.is_auto_saved() {
						responses.push_back(
							PortfolioMessage::AutoSaveDocument {
								// Safe to unwrap since we know that there is an active document
								document_id: self.active_document_id.unwrap(),
							}
							.into(),
						);
					}
				}

				if self.active_document().is_some() {
					responses.push_back(BroadcastEvent::ToolAbort.into());
				}

				// TODO: Remove this message in favor of having tools have specific data per document instance
				responses.push_back(PortfolioMessage::SetActiveDocument { document_id }.into());
				responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
				responses.push_back(FrontendMessage::UpdateActiveDocument { document_id }.into());
				responses.push_back(DocumentMessage::RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				for layer in self.documents.get(&document_id).unwrap().layer_metadata.keys() {
					responses.push_back(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() }.into());
				}
				responses.push_back(BroadcastEvent::SelectionChanged.into());
				responses.push_back(BroadcastEvent::DocumentIsDirty.into());
				responses.push_back(PortfolioMessage::UpdateDocumentWidgets.into());
				responses.push_back(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() }.into());
			}
			PortfolioMessage::SetActiveDocument { document_id } => self.active_document_id = Some(document_id),
			PortfolioMessage::SetImageBlobUrl {
				document_id,
				layer_path,
				blob_url,
				resolution,
			} => {
				let message = DocumentMessage::SetImageBlobUrl {
					layer_path,
					blob_url,
					resolution,
					document_id,
				};
				responses.push_back(PortfolioMessage::DocumentPassMessage { document_id, message }.into());
			}
			PortfolioMessage::UpdateDocumentWidgets => {
				if let Some(document) = self.active_document() {
					document.update_document_widgets(responses);
				}
			}
			PortfolioMessage::UpdateOpenDocumentsList => {
				// Send the list of document tab names
				let open_documents = self
					.document_ids
					.iter()
					.filter_map(|id| {
						self.documents.get(id).map(|document| FrontendDocumentDetails {
							is_auto_saved: document.is_auto_saved(),
							is_saved: document.is_saved(),
							id: *id,
							name: document.name.clone(),
						})
					})
					.collect::<Vec<_>>();
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(PortfolioMessageDiscriminant;
			CloseActiveDocumentWithConfirmation,
			CloseAllDocuments,
			Import,
			NextDocument,
			OpenDocument,
			Paste,
			PasteIntoFolder,
			PrevDocument,
		);

		if let Some(document) = self.active_document() {
			if document.layer_metadata.values().any(|data| data.selected) {
				let select = actions!(PortfolioMessageDiscriminant;
					Copy,
					Cut,
				);
				common.extend(select);
			}
			common.extend(document.actions());
		}

		common
	}
}

impl PortfolioMessageHandler {
	pub fn document(&self, document_id: u64) -> Option<&DocumentMessageHandler> {
		self.documents.get(&document_id)
	}

	pub fn document_mut(&mut self, document_id: u64) -> Option<&mut DocumentMessageHandler> {
		self.documents.get_mut(&document_id)
	}

	pub fn active_document(&self) -> Option<&DocumentMessageHandler> {
		self.active_document_id.and_then(|id| self.documents.get(&id))
	}

	pub fn active_document_mut(&mut self) -> Option<&mut DocumentMessageHandler> {
		self.active_document_id.and_then(|id| self.documents.get_mut(&id))
	}

	pub fn active_document_id(&self) -> Option<u64> {
		self.active_document_id
	}

	pub fn generate_new_document_name(&self) -> String {
		let mut doc_title_numbers = self
			.ordered_document_iterator()
			.filter_map(|doc| {
				doc.name
					.rsplit_once(DEFAULT_DOCUMENT_NAME)
					.map(|(prefix, number)| (prefix.is_empty()).then(|| number.trim().parse::<isize>().ok()).flatten().unwrap_or(1))
			})
			.collect::<Vec<isize>>();

		doc_title_numbers.sort_unstable();
		doc_title_numbers.iter_mut().enumerate().for_each(|(i, number)| *number = *number - i as isize - 2);
		// Uses binary search to find the index of the element where number is bigger than i
		let new_doc_title_num = doc_title_numbers.binary_search(&0).map_or_else(|e| e, |v| v) + 1;

		match new_doc_title_num {
			1 => DEFAULT_DOCUMENT_NAME.to_string(),
			_ => format!("{} {}", DEFAULT_DOCUMENT_NAME, new_doc_title_num),
		}
	}

	// TODO Fix how this doesn't preserve tab order upon loading new document from *File > Load*
	fn load_document(&mut self, new_document: DocumentMessageHandler, document_id: u64, responses: &mut VecDeque<Message>) {
		self.document_ids.push(document_id);

		responses.extend(
			new_document
				.layer_metadata
				.keys()
				.filter_map(|path| new_document.layer_panel_entry_from_path(path, &self.persistent_data.font_cache))
				.map(|entry| FrontendMessage::UpdateDocumentLayerDetails { data: entry }.into())
				.collect::<Vec<_>>(),
		);
		new_document.update_layer_tree_options_bar_widgets(responses, &self.persistent_data.font_cache);

		self.documents.insert(document_id, new_document);

		if self.active_document().is_some() {
			responses.push_back(PropertiesPanelMessage::Deactivate.into());
			responses.push_back(BroadcastEvent::ToolAbort.into());
			responses.push_back(ToolMessage::DeactivateTools.into());
		}

		responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
		responses.push_back(PortfolioMessage::SelectDocument { document_id }.into());
		responses.push_back(PortfolioMessage::LoadDocumentResources { document_id }.into());
		responses.push_back(PortfolioMessage::UpdateDocumentWidgets.into());
		responses.push_back(ToolMessage::InitTools.into());
		responses.push_back(PropertiesPanelMessage::Init.into());
		responses.push_back(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() }.into());
		responses.push_back(DocumentMessage::DocumentStructureChanged.into());
	}

	/// Returns an iterator over the open documents in order.
	pub fn ordered_document_iterator(&self) -> impl Iterator<Item = &DocumentMessageHandler> {
		self.document_ids.iter().map(|id| self.documents.get(id).expect("document id was not found in the document hashmap"))
	}

	fn document_index(&self, document_id: u64) -> usize {
		self.document_ids.iter().position(|id| id == &document_id).expect("Active document is missing from document ids")
	}
}
