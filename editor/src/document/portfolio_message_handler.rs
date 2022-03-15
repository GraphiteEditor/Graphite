use super::clipboards::{CopyBufferEntry, INTERNAL_CLIPBOARD_COUNT};
use super::DocumentMessageHandler;
use crate::consts::{DEFAULT_DOCUMENT_NAME, GRAPHITE_DOCUMENT_VERSION};
use crate::frontend::utility_types::FrontendDocumentDetails;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;

use graphene::Operation as DocumentOperation;

use log::warn;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct PortfolioMessageHandler {
	documents: HashMap<u64, DocumentMessageHandler>,
	document_ids: Vec<u64>,
	active_document_id: u64,
	copy_buffer: [Vec<CopyBufferEntry>; INTERNAL_CLIPBOARD_COUNT as usize],
}

impl PortfolioMessageHandler {
	pub fn active_document(&self) -> &DocumentMessageHandler {
		self.documents.get(&self.active_document_id).unwrap()
	}

	pub fn active_document_mut(&mut self) -> &mut DocumentMessageHandler {
		self.documents.get_mut(&self.active_document_id).unwrap()
	}

	fn generate_new_document_name(&self) -> String {
		let mut doc_title_numbers = self
			.ordered_document_iterator()
			.map(|doc| {
				doc.name
					.rsplit_once(DEFAULT_DOCUMENT_NAME)
					.map(|(prefix, number)| (prefix.is_empty()).then(|| number.trim().parse::<isize>().ok()).flatten().unwrap_or(1))
					.unwrap()
			})
			.collect::<Vec<isize>>();

		doc_title_numbers.sort_unstable();
		doc_title_numbers.iter_mut().enumerate().for_each(|(i, number)| *number = *number - i as isize - 2);
		// Uses binary search to find the index of the element where number is bigger than i
		let new_doc_title_num = doc_title_numbers.binary_search(&0).map_or_else(|e| e, |v| v) + 1;

		let name = match new_doc_title_num {
			1 => DEFAULT_DOCUMENT_NAME.to_string(),
			_ => format!("{} {}", DEFAULT_DOCUMENT_NAME, new_doc_title_num),
		};
		name
	}

	// TODO Fix how this doesn't preserve tab order upon loading new document from file>load
	fn load_document(&mut self, mut new_document: DocumentMessageHandler, document_id: u64, replace_first_empty: bool, responses: &mut VecDeque<Message>) {
		// Special case when loading a document on an empty page
		if replace_first_empty && self.active_document().is_unmodified_default() {
			responses.push_back(ToolMessage::AbortCurrentTool.into());
			responses.push_back(PortfolioMessage::CloseDocument { document_id: self.active_document_id }.into());

			let active_document_index = self
				.document_ids
				.iter()
				.position(|id| self.active_document_id == *id)
				.expect("Did not find matching active document id");
			self.document_ids.insert(active_document_index + 1, document_id);
		} else {
			self.document_ids.push(document_id);
		}

		responses.extend(
			new_document
				.layer_metadata
				.keys()
				.filter_map(|path| new_document.layer_panel_entry_from_path(path))
				.map(|entry| FrontendMessage::UpdateDocumentLayer { data: entry }.into())
				.collect::<Vec<_>>(),
		);

		new_document.load_image_data(responses);

		self.documents.insert(document_id, new_document);

		// Send the new list of document tab names
		let open_documents = self
			.document_ids
			.iter()
			.filter_map(|id| {
				self.documents.get(id).map(|document| FrontendDocumentDetails {
					is_saved: document.is_saved(),
					id: *id,
					name: document.name.clone(),
				})
			})
			.collect::<Vec<_>>();

		responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());

		responses.push_back(PortfolioMessage::SelectDocument { document_id }.into());
	}

	/// Returns an iterator over the open documents in order.
	pub fn ordered_document_iterator(&self) -> impl Iterator<Item = &DocumentMessageHandler> {
		self.document_ids.iter().map(|id| self.documents.get(id).expect("document id was not found in the document hashmap"))
	}

	fn document_index(&self, document_id: u64) -> usize {
		self.document_ids.iter().position(|id| id == &document_id).expect("Active document is missing from document ids")
	}
}

impl Default for PortfolioMessageHandler {
	fn default() -> Self {
		let mut documents_map: HashMap<u64, DocumentMessageHandler> = HashMap::with_capacity(1);
		let starting_key = generate_uuid();
		documents_map.insert(starting_key, DocumentMessageHandler::default());

		const EMPTY_VEC: Vec<CopyBufferEntry> = vec![];

		Self {
			documents: documents_map,
			document_ids: vec![starting_key],
			copy_buffer: [EMPTY_VEC; INTERNAL_CLIPBOARD_COUNT as usize],
			active_document_id: starting_key,
		}
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
			Document(message) => self.active_document_mut().process_action(message, ipp, responses),

			// Messages
			AutoSaveActiveDocument => responses.push_back(PortfolioMessage::AutoSaveDocument { document_id: self.active_document_id }.into()),
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
				responses.push_back(PortfolioMessage::CloseDocumentWithConfirmation { document_id: self.active_document_id }.into());
			}
			CloseAllDocuments => {
				// Empty the list of internal document data
				self.documents.clear();
				self.document_ids.clear();

				// Create a new blank document
				responses.push_back(NewDocument.into());
			}
			CloseAllDocumentsWithConfirmation => {
				responses.push_back(FrontendMessage::DisplayConfirmationToCloseAllDocuments.into());
			}
			CloseDocument { document_id } => {
				let document_index = self.document_index(document_id);
				self.documents.remove(&document_id);
				self.document_ids.remove(document_index);

				// Last tab was closed, so create a new blank tab
				if self.document_ids.is_empty() {
					let new_id = generate_uuid();
					self.document_ids.push(new_id);
					self.documents.insert(new_id, DocumentMessageHandler::default());
				}

				self.active_document_id = if document_id != self.active_document_id {
					// If we are not closing the active document, stay on it
					self.active_document_id
				} else if document_index >= self.document_ids.len() {
					// If we closed the last document take the one previous (same as last)
					*self.document_ids.last().unwrap()
				} else {
					// Move to the next tab
					self.document_ids[document_index]
				};

				// Send the new list of document tab names
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
				responses.push_back(FrontendMessage::UpdateActiveDocument { document_id: self.active_document_id }.into());
				responses.push_back(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id }.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				for layer in self.active_document().layer_metadata.keys() {
					responses.push_back(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() }.into());
				}
			}
			CloseDocumentWithConfirmation { document_id } => {
				let target_document = self.documents.get(&document_id).unwrap();
				if target_document.is_saved() {
					responses.push_back(ToolMessage::AbortCurrentTool.into());
					responses.push_back(PortfolioMessage::CloseDocument { document_id }.into());
				} else {
					responses.push_back(FrontendMessage::DisplayConfirmationToCloseDocument { document_id }.into());
					// Select the document being closed
					responses.push_back(PortfolioMessage::SelectDocument { document_id }.into());
				}
			}
			Copy { clipboard } => {
				// We can't use `self.active_document()` because it counts as an immutable borrow of the entirety of `self`
				let active_document = self.documents.get(&self.active_document_id).unwrap();

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
			Cut { clipboard } => {
				responses.push_back(Copy { clipboard }.into());
				responses.push_back(DeleteSelectedLayers.into());
			}
			NewDocument => {
				let name = self.generate_new_document_name();
				let new_document = DocumentMessageHandler::with_name(name, ipp);
				let document_id = generate_uuid();
				responses.push_back(ToolMessage::AbortCurrentTool.into());
				self.load_document(new_document, document_id, false, responses);
			}
			NextDocument => {
				let current_index = self.document_index(self.active_document_id);
				let next_index = (current_index + 1) % self.document_ids.len();
				let next_id = self.document_ids[next_index];

				responses.push_back(PortfolioMessage::SelectDocument { document_id: next_id }.into());
			}
			OpenDocument => {
				responses.push_back(FrontendMessage::TriggerFileUpload.into());
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
						self.load_document(document, document_id, true, responses);
					}
					Err(e) => responses.push_back(
						FrontendMessage::DisplayDialogError {
							title: "Failed to open document".to_string(),
							description: e.to_string(),
						}
						.into(),
					),
				}
			}
			Paste { clipboard } => {
				let document = self.active_document();
				let shallowest_common_folder = document
					.graphene_document
					.shallowest_common_folder(document.selected_layers())
					.expect("While pasting, the selected layers did not exist while attempting to find the appropriate folder path for insertion");
				responses.push_back(DeselectAllLayers.into());
				responses.push_back(StartTransaction.into());
				responses.push_back(
					PasteIntoFolder {
						clipboard,
						folder_path: shallowest_common_folder.to_vec(),
						insert_index: -1,
					}
					.into(),
				);
				responses.push_back(CommitTransaction.into());
			}
			PasteIntoFolder {
				clipboard,
				folder_path: path,
				insert_index,
			} => {
				let paste = |entry: &CopyBufferEntry, responses: &mut VecDeque<_>| {
					log::trace!("Pasting into folder {:?} as index: {}", &path, insert_index);

					let destination_path = [path.to_vec(), vec![generate_uuid()]].concat();

					responses.push_front(
						DocumentMessage::UpdateLayerMetadata {
							layer_path: destination_path.clone(),
							layer_metadata: entry.layer_metadata,
						}
						.into(),
					);
					responses.push_front(
						DocumentOperation::InsertLayer {
							layer: entry.layer.clone(),
							destination_path,
							insert_index,
						}
						.into(),
					);
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
				if let Ok(data) = serde_json::from_str::<Vec<CopyBufferEntry>>(&data) {
					let document = self.active_document();
					let shallowest_common_folder = document
						.graphene_document
						.shallowest_common_folder(document.selected_layers())
						.expect("While pasting from serialized, the selected layers did not exist while attempting to find the appropriate folder path for insertion");
					responses.push_back(DeselectAllLayers.into());
					responses.push_back(StartTransaction.into());

					for entry in data {
						let destination_path = [shallowest_common_folder.to_vec(), vec![generate_uuid()]].concat();

						responses.push_front(
							DocumentMessage::UpdateLayerMetadata {
								layer_path: destination_path.clone(),
								layer_metadata: entry.layer_metadata,
							}
							.into(),
						);
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
			PrevDocument => {
				let len = self.document_ids.len();
				let current_index = self.document_index(self.active_document_id);
				let prev_index = (current_index + len - 1) % len;
				let prev_id = self.document_ids[prev_index];
				responses.push_back(PortfolioMessage::SelectDocument { document_id: prev_id }.into());
			}
			RequestAboutGraphiteDialog => {
				responses.push_back(FrontendMessage::DisplayDialogAboutGraphite.into());
			}
			SelectDocument { document_id } => {
				let active_document = self.active_document();
				if !active_document.is_saved() {
					responses.push_back(PortfolioMessage::AutoSaveDocument { document_id: self.active_document_id }.into());
				}
				responses.push_back(ToolMessage::AbortCurrentTool.into());
				responses.push_back(SetActiveDcoument { document_id }.into());

				responses.push_back(FrontendMessage::UpdateActiveDocument { document_id }.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				for layer in self.documents.get(&document_id).unwrap().layer_metadata.keys() {
					responses.push_back(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() }.into());
				}
				responses.push_back(ToolMessage::DocumentIsDirty.into());
				responses.push_back(PortfolioMessage::UpdateDocumentBar.into());
			}
			SetActiveDcoument { document_id } => {
				self.active_document_id = document_id;
			}
			UpdateDocumentBar => {
				let active_document = self.active_document();
				active_document.register_properties(responses, LayoutTarget::DocumentBar)
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
			NewDocument,
			CloseActiveDocumentWithConfirmation,
			CloseAllDocumentsWithConfirmation,
			CloseAllDocuments,
			NextDocument,
			PrevDocument,
			PasteIntoFolder,
			Paste,
		);

		if self.active_document().layer_metadata.values().any(|data| data.selected) {
			let select = actions!(PortfolioMessageDiscriminant;
				Copy,
				Cut,
			);
			common.extend(select);
		}
		common.extend(self.active_document().actions());
		common
	}
}
