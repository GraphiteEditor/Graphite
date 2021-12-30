use super::{DocumentMessageHandler, LayerData};
use crate::consts::DEFAULT_DOCUMENT_NAME;
use crate::frontend::frontend_message_handler::FrontendDocumentDetails;
use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use graphene::layers::Layer;
use graphene::{LayerId, Operation as DocumentOperation};

use log::warn;
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, VecDeque};

#[repr(u8)]
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Clipboard {
	System,
	User,
	_ClipboardCount,
}

const CLIPBOARD_COUNT: u8 = Clipboard::_ClipboardCount as u8;

#[impl_message(Message, Documents)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum DocumentsMessage {
	Copy(Clipboard),
	Cut(Clipboard),
	PasteIntoFolder {
		clipboard: Clipboard,
		path: Vec<LayerId>,
		insert_index: isize,
	},
	Paste(Clipboard),
	SelectDocument(u64),
	CloseDocument(u64),
	#[child]
	Document(DocumentMessage),
	CloseActiveDocumentWithConfirmation,
	CloseDocumentWithConfirmation(u64),
	CloseAllDocumentsWithConfirmation,
	CloseAllDocuments,
	RequestAboutGraphiteDialog,
	NewDocument,
	OpenDocument,
	OpenDocumentFileWithId {
		document: String,
		document_name: String,
		document_id: u64,
		document_is_saved: bool,
	},
	OpenDocumentFile(String, String),
	UpdateOpenDocumentsList,
	AutoSaveDocument(u64),
	AutoSaveActiveDocument,
	NextDocument,
	PrevDocument,
}

#[derive(Debug, Clone)]
pub struct DocumentsMessageHandler {
	documents: HashMap<u64, DocumentMessageHandler>,
	document_ids: Vec<u64>,
	active_document_id: u64,
	copy_buffer: [Vec<CopyBufferEntry>; CLIPBOARD_COUNT as usize],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopyBufferEntry {
	layer: Layer,
	layer_data: LayerData,
}

impl DocumentsMessageHandler {
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
			responses.push_back(DocumentsMessage::CloseDocument(self.active_document_id).into());

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
				.layer_data
				.keys()
				.filter_map(|path| new_document.layer_panel_entry_from_path(path))
				.map(|entry| FrontendMessage::UpdateLayer { data: entry }.into())
				.collect::<Vec<_>>(),
		);

		self.documents.insert(document_id, new_document);

		// Send the new list of document tab names
		let open_documents = self
			.document_ids
			.iter()
			.filter_map(|id| {
				self.documents.get(&id).map(|doc| FrontendDocumentDetails {
					is_saved: doc.is_saved(),
					id: *id,
					name: doc.name.clone(),
				})
			})
			.collect::<Vec<_>>();

		responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());

		responses.push_back(DocumentsMessage::SelectDocument(document_id).into());
	}

	// Returns an iterator over the open documents in order
	pub fn ordered_document_iterator(&self) -> impl Iterator<Item = &DocumentMessageHandler> {
		self.document_ids.iter().map(|id| self.documents.get(id).expect("document id was not found in the document hashmap"))
	}

	fn document_index(&self, document_id: u64) -> usize {
		self.document_ids.iter().position(|id| id == &document_id).expect("Active document is missing from document ids")
	}
}

impl Default for DocumentsMessageHandler {
	fn default() -> Self {
		let mut documents_map: HashMap<u64, DocumentMessageHandler> = HashMap::with_capacity(1);
		let starting_key = generate_uuid();
		documents_map.insert(starting_key, DocumentMessageHandler::default());

		const EMPTY_VEC: Vec<CopyBufferEntry> = vec![];

		Self {
			documents: documents_map,
			document_ids: vec![starting_key],
			copy_buffer: [EMPTY_VEC; CLIPBOARD_COUNT as usize],
			active_document_id: starting_key,
		}
	}
}

impl MessageHandler<DocumentsMessage, &InputPreprocessor> for DocumentsMessageHandler {
	fn process_action(&mut self, message: DocumentsMessage, ipp: &InputPreprocessor, responses: &mut VecDeque<Message>) {
		use DocumentMessage::*;
		use DocumentsMessage::*;
		match message {
			RequestAboutGraphiteDialog => {
				responses.push_back(FrontendMessage::DisplayAboutGraphiteDialog.into());
			}
			Document(message) => self.active_document_mut().process_action(message, ipp, responses),
			SelectDocument(id) => {
				let active_document = self.active_document();
				if !active_document.is_saved() {
					responses.push_back(DocumentsMessage::AutoSaveDocument(self.active_document_id).into());
				}
				self.active_document_id = id;
				responses.push_back(FrontendMessage::SetActiveDocument { document_id: id }.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				for layer in self.active_document().layer_data.keys() {
					responses.push_back(DocumentMessage::LayerChanged(layer.clone()).into());
				}
			}
			CloseActiveDocumentWithConfirmation => {
				responses.push_back(DocumentsMessage::CloseDocumentWithConfirmation(self.active_document_id).into());
			}
			CloseDocumentWithConfirmation(id) => {
				let target_document = self.documents.get(&id).unwrap();
				if target_document.is_saved() {
					responses.push_back(DocumentsMessage::CloseDocument(id).into());
				} else {
					responses.push_back(FrontendMessage::DisplayConfirmationToCloseDocument { document_id: id }.into());
					// Select the document being closed
					responses.push_back(DocumentsMessage::SelectDocument(id).into());
				}
			}
			CloseAllDocumentsWithConfirmation => {
				responses.push_back(FrontendMessage::DisplayConfirmationToCloseAllDocuments.into());
			}
			CloseAllDocuments => {
				// Empty the list of internal document data
				self.documents.clear();
				self.document_ids.clear();

				// Create a new blank document
				responses.push_back(NewDocument.into());
			}
			CloseDocument(id) => {
				let document_index = self.document_index(id);
				self.documents.remove(&id);
				self.document_ids.remove(document_index);

				// Last tab was closed, so create a new blank tab
				if self.document_ids.is_empty() {
					let new_id = generate_uuid();
					self.document_ids.push(new_id);
					self.documents.insert(new_id, DocumentMessageHandler::default());
				}

				self.active_document_id = if id != self.active_document_id {
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
						self.documents.get(&id).map(|doc| FrontendDocumentDetails {
							is_saved: doc.is_saved(),
							id: *id,
							name: doc.name.clone(),
						})
					})
					.collect::<Vec<_>>();
				// Update the list of new documents on the front end, active tab, and ensure that document renders
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());
				responses.push_back(FrontendMessage::SetActiveDocument { document_id: self.active_document_id }.into());
				responses.push_back(FrontendMessage::RemoveAutoSaveDocument { document_id: id }.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				for layer in self.active_document().layer_data.keys() {
					responses.push_back(DocumentMessage::LayerChanged(layer.clone()).into());
				}
			}
			NewDocument => {
				let name = self.generate_new_document_name();
				let new_document = DocumentMessageHandler::with_name(name, ipp);
				let document_id = generate_uuid();
				self.active_document_id = document_id;
				self.load_document(new_document, document_id, false, responses);
			}
			OpenDocument => {
				responses.push_back(FrontendMessage::OpenDocumentBrowse.into());
			}
			OpenDocumentFile(document_name, document) => {
				responses.push_back(
					DocumentsMessage::OpenDocumentFileWithId {
						document,
						document_name,
						document_id: generate_uuid(),
						document_is_saved: true,
					}
					.into(),
				);
			}
			OpenDocumentFileWithId {
				document_name,
				document_id,
				document,
				document_is_saved,
			} => {
				let document = DocumentMessageHandler::with_name_and_content(document_name, document);
				match document {
					Ok(mut document) => {
						document.set_save_state(document_is_saved);
						self.load_document(document, document_id, true, responses);
					}
					Err(e) => responses.push_back(
						FrontendMessage::DisplayError {
							title: "Failed to open document".to_string(),
							description: e.to_string(),
						}
						.into(),
					),
				}
			}
			UpdateOpenDocumentsList => {
				// Send the list of document tab names
				let open_documents = self
					.document_ids
					.iter()
					.filter_map(|id| {
						self.documents.get(&id).map(|doc| FrontendDocumentDetails {
							is_saved: doc.is_saved(),
							id: *id,
							name: doc.name.clone(),
						})
					})
					.collect::<Vec<_>>();
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());
			}
			AutoSaveDocument(id) => {
				let document = self.documents.get(&id).unwrap();
				responses.push_back(
					FrontendMessage::AutoSaveDocument {
						document: document.serialize_document(),
						details: FrontendDocumentDetails {
							is_saved: document.is_saved(),
							id,
							name: document.name.clone(),
						},
					}
					.into(),
				)
			}
			AutoSaveActiveDocument => responses.push_back(DocumentsMessage::AutoSaveDocument(self.active_document_id).into()),
			NextDocument => {
				let current_index = self.document_index(self.active_document_id);
				let next_index = (current_index + 1) % self.document_ids.len();
				let next_id = self.document_ids[next_index];
				responses.push_back(DocumentsMessage::SelectDocument(next_id).into());
			}
			PrevDocument => {
				let len = self.document_ids.len();
				let current_index = self.document_index(self.active_document_id);
				let prev_index = (current_index + len - 1) % len;
				let prev_id = self.document_ids[prev_index];
				responses.push_back(DocumentsMessage::SelectDocument(prev_id).into());
			}
			Copy(clipboard) => {
				let paths = self.active_document().selected_layers_sorted();
				self.copy_buffer[clipboard as usize].clear();
				for path in paths {
					let document = self.active_document();
					match (document.graphene_document.layer(&path).map(|t| t.clone()), document.layer_data(&path).clone()) {
						(Ok(layer), layer_data) => {
							self.copy_buffer[clipboard as usize].push(CopyBufferEntry { layer, layer_data });
						}
						(Err(e), _) => warn!("Could not access selected layer {:?}: {:?}", path, e),
					}
				}
			}
			Cut(clipboard) => {
				responses.push_back(Copy(clipboard).into());
				responses.push_back(DeleteSelectedLayers.into());
			}
			Paste(clipboard) => {
				let document = self.active_document();
				let shallowest_common_folder = document
					.graphene_document
					.deepest_common_folder(document.selected_layers())
					.expect("While pasting, the selected layers did not exist while attempting to find the appropriate folder path for insertion");

				responses.push_back(
					PasteIntoFolder {
						clipboard,
						path: shallowest_common_folder.to_vec(),
						insert_index: -1,
					}
					.into(),
				);
			}
			PasteIntoFolder { clipboard, path, insert_index } => {
				let paste = |entry: &CopyBufferEntry, responses: &mut VecDeque<_>| {
					log::trace!("Pasting into folder {:?} as index: {}", &path, insert_index);

					let destination_path = [path.to_vec(), vec![generate_uuid()]].concat();

					responses.push_back(
						DocumentOperation::InsertLayer {
							layer: entry.layer.clone(),
							destination_path: destination_path.clone(),
							insert_index,
						}
						.into(),
					);
					responses.push_back(
						DocumentMessage::UpdateLayerData {
							path: destination_path,
							layer_data_entry: entry.layer_data,
						}
						.into(),
					);
				};

				if insert_index == -1 {
					for entry in self.copy_buffer[clipboard as usize].iter() {
						paste(entry, responses)
					}
				} else {
					for entry in self.copy_buffer[clipboard as usize].iter().rev() {
						paste(entry, responses)
					}
				}
			}
		}
	}
	fn actions(&self) -> ActionList {
		let mut common = actions!(DocumentsMessageDiscriminant;
			NewDocument,
			CloseActiveDocumentWithConfirmation,
			CloseAllDocumentsWithConfirmation,
			CloseAllDocuments,
			NextDocument,
			PrevDocument,
			PasteIntoFolder,
			Paste,
		);

		if self.active_document().layer_data.values().any(|data| data.selected) {
			let select = actions!(DocumentsMessageDiscriminant;
				Copy,
				Cut,
			);
			common.extend(select);
		}
		common.extend(self.active_document().actions());
		common
	}
}
