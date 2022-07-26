use super::clipboards::{CopyBufferEntry, INTERNAL_CLIPBOARD_COUNT};
use super::utility_types::Platform;
use super::{DocumentMessageHandler, MenuBarMessageHandler};
use crate::consts::{DEFAULT_DOCUMENT_NAME, GRAPHITE_DOCUMENT_VERSION};
use crate::dialog;
use crate::frontend::utility_types::FrontendDocumentDetails;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;

use graphene::layers::layer_info::LayerDataTypeDiscriminant;
use graphene::layers::text_layer::{Font, FontCache};
use graphene::Operation as DocumentOperation;

use log::warn;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Default)]
pub struct PortfolioMessageHandler {
	menu_bar_message_handler: MenuBarMessageHandler,
	documents: HashMap<u64, DocumentMessageHandler>,
	document_ids: Vec<u64>,
	active_document_id: Option<u64>,
	copy_buffer: [Vec<CopyBufferEntry>; INTERNAL_CLIPBOARD_COUNT as usize],
	font_cache: FontCache,
	pub platform: Platform,
}

impl PortfolioMessageHandler {
	pub fn active_document(&self) -> Option<&DocumentMessageHandler> {
		self.active_document_id.and_then(|id| self.documents.get(&id))
	}

	pub fn active_document_mut(&mut self) -> Option<&mut DocumentMessageHandler> {
		self.active_document_id.and_then(|id| self.documents.get_mut(&id))
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
				.filter_map(|path| new_document.layer_panel_entry_from_path(path, &self.font_cache))
				.map(|entry| FrontendMessage::UpdateDocumentLayerDetails { data: entry }.into())
				.collect::<Vec<_>>(),
		);
		new_document.update_layer_tree_options_bar_widgets(responses, &self.font_cache);

		new_document.load_layer_resources(responses, &new_document.graphene_document.root.data, Vec::new());

		self.documents.insert(document_id, new_document);

		if self.active_document().is_some() {
			responses.push_back(PropertiesPanelMessage::Deactivate.into());
			responses.push_back(BroadcastSignal::ToolAbort.into());
			responses.push_back(ToolMessage::DeactivateTools.into());
		}

		responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
		responses.push_back(PortfolioMessage::SelectDocument { document_id }.into());
		responses.push_back(PortfolioMessage::UpdateDocumentWidgets.into());
		responses.push_back(ToolMessage::InitTools.into());
		responses.push_back(PropertiesPanelMessage::Init.into());
		responses.push_back(MovementMessage::TranslateCanvas { delta: (0., 0.).into() }.into());
		responses.push_back(DocumentMessage::DocumentStructureChanged.into())
	}

	/// Returns an iterator over the open documents in order.
	pub fn ordered_document_iterator(&self) -> impl Iterator<Item = &DocumentMessageHandler> {
		self.document_ids.iter().map(|id| self.documents.get(id).expect("document id was not found in the document hashmap"))
	}

	fn document_index(&self, document_id: u64) -> usize {
		self.document_ids.iter().position(|id| id == &document_id).expect("Active document is missing from document ids")
	}

	pub fn font_cache(&self) -> &FontCache {
		&self.font_cache
	}
}

impl MessageHandler<PortfolioMessage, &InputPreprocessorMessageHandler> for PortfolioMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: PortfolioMessage, ipp: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		use DocumentMessage::*;
		use PortfolioMessage::*;

		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			Document(message) => {
				if let Some(document) = self.active_document_id.and_then(|id| self.documents.get_mut(&id)) {
					document.process_action(message, (ipp, &self.font_cache), responses)
				}
			}
			#[remain::unsorted]
			MenuBar(message) => self.menu_bar_message_handler.process_action(message, (), responses),

			// Messages
			AutoSaveActiveDocument => {
				if let Some(document_id) = self.active_document_id {
					responses.push_back(PortfolioMessage::AutoSaveDocument { document_id }.into());
				}
			}
			AutoSaveDocument { document_id } => {
				let document = self.documents.get(&document_id).unwrap();
				responses.push_back(
					FrontendMessage::TriggerIndexedDbWriteDocument {
						document: document.serialize_document(),
						details: FrontendDocumentDetails {
							is_saved: document.is_saved(),
							id: document_id,
							name: document.name.clone(),
						},
						version: GRAPHITE_DOCUMENT_VERSION.to_string(),
					}
					.into(),
				)
			}
			CloseActiveDocumentWithConfirmation => {
				if let Some(document_id) = self.active_document_id {
					responses.push_back(PortfolioMessage::CloseDocumentWithConfirmation { document_id }.into());
				}
			}
			CloseAllDocuments => {
				if self.active_document_id.is_some() {
					responses.push_back(PropertiesPanelMessage::Deactivate.into());
					responses.push_back(BroadcastSignal::ToolAbort.into());
					responses.push_back(ToolMessage::DeactivateTools.into());
				}

				for document_id in &self.document_ids {
					responses.push_back(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id: *document_id }.into());
				}

				responses.push_back(PortfolioMessage::DestroyAllDocuments.into());
				responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
			}
			CloseDocument { document_id } => {
				let document_index = self.document_index(document_id);
				self.documents.remove(&document_id);
				self.document_ids.remove(document_index);

				if self.document_ids.is_empty() {
					self.active_document_id = None;
				} else if Some(document_id) == self.active_document_id {
					if document_index == self.document_ids.len() {
						// If we closed the last document take the one previous (same as last)
						responses.push_back(
							PortfolioMessage::SelectDocument {
								document_id: *self.document_ids.last().unwrap(),
							}
							.into(),
						);
					} else {
						// Move to the next tab
						responses.push_back(
							PortfolioMessage::SelectDocument {
								document_id: self.document_ids[document_index],
							}
							.into(),
						);
					}
				}

				// Send the new list of document tab names
				responses.push_back(UpdateOpenDocumentsList.into());
				responses.push_back(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id }.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				if let Some(document) = self.active_document() {
					for layer in document.layer_metadata.keys() {
						responses.push_back(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() }.into());
					}
				}
			}
			CloseDocumentWithConfirmation { document_id } => {
				let target_document = self.documents.get(&document_id).unwrap();
				if target_document.is_saved() {
					responses.push_back(BroadcastSignal::ToolAbort.into());
					responses.push_back(PortfolioMessage::CloseDocument { document_id }.into());
				} else {
					let dialog = dialog::CloseDocument {
						document_name: target_document.name.clone(),
						document_id,
					};
					dialog.register_properties(responses, LayoutTarget::DialogDetails);
					responses.push_back(FrontendMessage::DisplayDialog { icon: "File".to_string() }.into());

					// Select the document being closed
					responses.push_back(PortfolioMessage::SelectDocument { document_id }.into());
				}
			}
			Copy { clipboard } => {
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
			Cut { clipboard } => {
				responses.push_back(Copy { clipboard }.into());
				responses.push_back(DeleteSelectedLayers.into());
			}
			DestroyAllDocuments => {
				// Empty the list of internal document data
				self.documents.clear();
				self.document_ids.clear();
				self.active_document_id = None;
			}
			FontLoaded {
				font_family,
				font_style,
				preview_url,
				data,
				is_default,
			} => {
				self.font_cache.insert(Font::new(font_family, font_style), preview_url, data, is_default);

				if let Some(document) = self.active_document_mut() {
					document.graphene_document.mark_all_layers_of_type_as_dirty(LayerDataTypeDiscriminant::Text);
					responses.push_back(DocumentMessage::RenderDocument.into());
				}
			}
			Import => {
				// This portfolio message wraps the frontend message so it can be listed as an action, which isn't possible for frontend messages
				if self.active_document().is_some() {
					responses.push_back(FrontendMessage::TriggerImport.into());
				}
			}
			LoadFont { font, is_default } => {
				if !self.font_cache.loaded_font(&font) {
					responses.push_front(FrontendMessage::TriggerFontLoad { font, is_default }.into());
				}
			}
			NewDocumentWithName { name } => {
				let new_document = DocumentMessageHandler::with_name(name, ipp);
				let document_id = generate_uuid();
				if self.active_document().is_some() {
					responses.push_back(BroadcastSignal::ToolAbort.into());
					responses.push_back(MovementMessage::TranslateCanvas { delta: (0., 0.).into() }.into());
				}

				self.load_document(new_document, document_id, responses);
			}
			NextDocument => {
				if let Some(active_document_id) = self.active_document_id {
					let current_index = self.document_index(active_document_id);
					let next_index = (current_index + 1) % self.document_ids.len();
					let next_id = self.document_ids[next_index];

					responses.push_back(PortfolioMessage::SelectDocument { document_id: next_id }.into());
				}
			}
			OpenDocument => {
				// This portfolio message wraps the frontend message so it can be listed as an action, which isn't possible for frontend messages
				responses.push_back(FrontendMessage::TriggerOpenDocument.into());
			}
			OpenDocumentFile {
				document_name,
				document_serialized_content,
			} => {
				responses.push_back(
					PortfolioMessage::OpenDocumentFileWithId {
						document_id: generate_uuid(),
						document_name,
						document_is_saved: true,
						document_serialized_content,
					}
					.into(),
				);
			}
			OpenDocumentFileWithId {
				document_id,
				document_name,
				document_is_saved,
				document_serialized_content,
			} => {
				let document = DocumentMessageHandler::with_name_and_content(document_name, document_serialized_content);
				match document {
					Ok(mut document) => {
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
			Paste { clipboard } => {
				let shallowest_common_folder = self.active_document().map(|document| {
					document
						.graphene_document
						.shallowest_common_folder(document.selected_layers())
						.expect("While pasting, the selected layers did not exist while attempting to find the appropriate folder path for insertion")
				});

				if let Some(folder) = shallowest_common_folder {
					responses.push_back(DeselectAllLayers.into());
					responses.push_back(StartTransaction.into());
					responses.push_back(
						PasteIntoFolder {
							clipboard,
							folder_path: folder.to_vec(),
							insert_index: -1,
						}
						.into(),
					);
					responses.push_back(CommitTransaction.into());
				}
			}
			PasteIntoFolder {
				clipboard,
				folder_path: path,
				insert_index,
			} => {
				let paste = |entry: &CopyBufferEntry, responses: &mut VecDeque<_>| {
					if let Some(document) = self.active_document() {
						log::trace!("Pasting into folder {:?} as index: {}", &path, insert_index);
						let destination_path = [path.to_vec(), vec![generate_uuid()]].concat();

						responses.push_front(
							DocumentMessage::UpdateLayerMetadata {
								layer_path: destination_path.clone(),
								layer_metadata: entry.layer_metadata,
							}
							.into(),
						);
						document.load_layer_resources(responses, &entry.layer.data, destination_path.clone());
						responses.push_front(
							DocumentOperation::InsertLayer {
								layer: entry.layer.clone(),
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
			PasteSerializedData { data } => {
				if let Some(document) = self.active_document() {
					if let Ok(data) = serde_json::from_str::<Vec<CopyBufferEntry>>(&data) {
						let shallowest_common_folder = document
							.graphene_document
							.shallowest_common_folder(document.selected_layers())
							.expect("While pasting from serialized, the selected layers did not exist while attempting to find the appropriate folder path for insertion");
						responses.push_back(DeselectAllLayers.into());
						responses.push_back(StartTransaction.into());

						for entry in data.iter().rev() {
							let destination_path = [shallowest_common_folder.to_vec(), vec![generate_uuid()]].concat();

							responses.push_front(
								DocumentMessage::UpdateLayerMetadata {
									layer_path: destination_path.clone(),
									layer_metadata: entry.layer_metadata,
								}
								.into(),
							);
							document.load_layer_resources(responses, &entry.layer.data, destination_path.clone());
							responses.push_front(
								DocumentOperation::InsertLayer {
									layer: entry.layer.clone(),
									destination_path,
									insert_index: -1,
								}
								.into(),
							);
						}

						responses.push_back(CommitTransaction.into());
					}
				}
			}
			PrevDocument => {
				if let Some(active_document_id) = self.active_document_id {
					let len = self.document_ids.len();
					let current_index = self.document_index(active_document_id);
					let prev_index = (current_index + len - 1) % len;
					let prev_id = self.document_ids[prev_index];
					responses.push_back(PortfolioMessage::SelectDocument { document_id: prev_id }.into());
				}
			}
			SelectDocument { document_id } => {
				if let Some(document) = self.active_document() {
					if !document.is_saved() {
						// Safe to unwrap since we know that there is an active document
						responses.push_back(
							PortfolioMessage::AutoSaveDocument {
								document_id: self.active_document_id.unwrap(),
							}
							.into(),
						);
					}
				}

				if self.active_document().is_some() {
					responses.push_back(BroadcastSignal::ToolAbort.into());
				}

				// TODO: Remove this message in favor of having tools have specific data per document instance
				responses.push_back(SetActiveDocument { document_id }.into());
				responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
				responses.push_back(FrontendMessage::UpdateActiveDocument { document_id }.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				for layer in self.documents.get(&document_id).unwrap().layer_metadata.keys() {
					responses.push_back(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() }.into());
				}
				responses.push_back(BroadcastSignal::SelectionChanged.into());
				responses.push_back(BroadcastSignal::DocumentIsDirty.into());
				responses.push_back(PortfolioMessage::UpdateDocumentWidgets.into());
				responses.push_back(MovementMessage::TranslateCanvas { delta: (0., 0.).into() }.into());
			}
			SetActiveDocument { document_id } => self.active_document_id = Some(document_id),
			SetPlatform { platform } => self.platform = platform,
			UpdateDocumentWidgets => {
				if let Some(document) = self.active_document() {
					document.update_document_widgets(responses);
				}
			}
			UpdateOpenDocumentsList => {
				// Send the list of document tab names
				let open_documents = self
					.document_ids
					.iter()
					.filter_map(|id| {
						self.documents.get(id).map(|doc| FrontendDocumentDetails {
							is_saved: doc.is_saved(),
							id: *id,
							name: doc.name.clone(),
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
