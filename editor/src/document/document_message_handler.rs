use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use graphene::layers::Layer;
use graphene::{LayerId, Operation as DocumentOperation};
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use super::DocumentMessageHandler;
use crate::consts::DEFAULT_DOCUMENT_NAME;

#[impl_message(Message, Documents)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum DocumentsMessage {
	Copy,
	PasteIntoFolder {
		path: Vec<LayerId>,
		insert_index: isize,
	},
	Paste,
	SelectDocument(usize),
	CloseDocument(usize),
	#[child]
	Document(DocumentMessage),
	CloseActiveDocumentWithConfirmation,
	CloseAllDocumentsWithConfirmation,
	CloseAllDocuments,
	RequestAboutGraphiteDialog,
	NewDocument,
	OpenDocument,
	OpenDocumentFile(String, String),
	UpdateOpenDocumentsList,
	NextDocument,
	PrevDocument,
}

#[derive(Debug, Clone)]
pub struct DocumentsMessageHandler {
	documents: HashMap<u64, DocumentMessageHandler>,
	document_ids: Vec<u64>,
	document_id_counter: u64,
	active_document_index: usize,
	copy_buffer: Vec<Layer>,
}

impl DocumentsMessageHandler {
	pub fn active_document(&self) -> &DocumentMessageHandler {
		let id = self.document_ids[self.active_document_index];
		self.documents.get(&id).unwrap()
	}

	pub fn active_document_mut(&mut self) -> &mut DocumentMessageHandler {
		let id = self.document_ids[self.active_document_index];
		self.documents.get_mut(&id).unwrap()
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

	fn load_document(&mut self, new_document: DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.document_id_counter += 1;
		self.active_document_index = self.document_ids.len();
		self.document_ids.push(self.document_id_counter);
		self.documents.insert(self.document_id_counter, new_document);

		// Send the new list of document tab names
		let open_documents = self
			.document_ids
			.iter()
			.filter_map(|id| self.documents.get(&id).map(|doc| (doc.name.clone(), doc.is_saved())))
			.collect::<Vec<_>>();

		responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());

		responses.push_back(DocumentsMessage::SelectDocument(self.active_document_index).into());
		responses.push_back(DocumentMessage::RenderDocument.into());
		responses.push_back(DocumentMessage::DocumentStructureChanged.into());
		for layer in self.active_document().layer_data.keys() {
			responses.push_back(DocumentMessage::LayerChanged(layer.clone()).into());
		}
	}

	// Returns an iterator over the open documents in order
	pub fn ordered_document_iterator(&self) -> impl Iterator<Item = &DocumentMessageHandler> {
		self.document_ids.iter().map(|id| self.documents.get(id).expect("document id was not found in the document hashmap"))
	}
}

impl Default for DocumentsMessageHandler {
	fn default() -> Self {
		let mut documents_map: HashMap<u64, DocumentMessageHandler> = HashMap::with_capacity(1);
		documents_map.insert(0, DocumentMessageHandler::default());
		Self {
			documents: documents_map,
			document_ids: vec![0],
			copy_buffer: vec![],
			active_document_index: 0,
			document_id_counter: 0,
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
			SelectDocument(index) => {
				// NOTE: Potentially this will break if we ever exceed 56 bit values due to how the message parsing system works.
				assert!(index < self.documents.len(), "Tried to select a document that was not initialized");
				self.active_document_index = index;
				responses.push_back(FrontendMessage::SetActiveDocument { document_index: index }.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				for layer in self.active_document().layer_data.keys() {
					responses.push_back(DocumentMessage::LayerChanged(layer.clone()).into());
				}
			}
			CloseActiveDocumentWithConfirmation => {
				responses.push_back(
					FrontendMessage::DisplayConfirmationToCloseDocument {
						document_index: self.active_document_index,
					}
					.into(),
				);
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
			CloseDocument(index) => {
				assert!(index < self.documents.len(), "Tried to close a document that was not initialized");
				// Get the ID based on the current collection of the documents.
				let id = self.document_ids[index];
				// Map the ID to an index and remove the document
				self.documents.remove(&id);
				self.document_ids.remove(index);

				// Last tab was closed, so create a new blank tab
				if self.document_ids.is_empty() {
					self.document_id_counter += 1;
					self.document_ids.push(self.document_id_counter);
					self.documents.insert(self.document_id_counter, DocumentMessageHandler::default());
				}

				self.active_document_index = if self.active_document_index >= self.document_ids.len() {
					self.document_ids.len() - 1
				} else {
					index
				};

				// Send the new list of document tab names
				let open_documents = self.ordered_document_iterator().map(|doc| (doc.name.clone(), doc.is_saved())).collect();

				// Update the list of new documents on the front end, active tab, and ensure that document renders
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());
				responses.push_back(
					FrontendMessage::SetActiveDocument {
						document_index: self.active_document_index,
					}
					.into(),
				);
				responses.push_back(RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				for layer in self.active_document().layer_data.keys() {
					responses.push_back(DocumentMessage::LayerChanged(layer.clone()).into());
				}
			}
			NewDocument => {
				let name = self.generate_new_document_name();
				let new_document = DocumentMessageHandler::with_name_and_centered_transform(name, ipp);
				self.load_document(new_document, responses);
			}
			OpenDocument => {
				responses.push_back(FrontendMessage::OpenDocumentBrowse.into());
			}
			OpenDocumentFile(name, serialized_contents) => {
				let document = DocumentMessageHandler::with_name_and_content(name, serialized_contents);
				match document {
					Ok(document) => {
						self.load_document(document, responses);
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
				let open_documents = self.ordered_document_iterator().map(|doc| (doc.name.clone(), doc.is_saved())).collect();
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());
			}
			NextDocument => {
				let next = (self.active_document_index + 1) % self.document_ids.len();
				responses.push_back(SelectDocument(next).into());
			}
			PrevDocument => {
				let len = self.document_ids.len();
				let prev = (self.active_document_index + len - 1) % len;
				responses.push_back(SelectDocument(prev).into());
			}
			Copy => {
				let paths = self.active_document().selected_layers_sorted();
				self.copy_buffer.clear();
				for path in paths {
					match self.active_document().graphene_document.layer(&path).map(|t| t.clone()) {
						Ok(layer) => {
							self.copy_buffer.push(layer);
						}
						Err(e) => warn!("Could not access selected layer {:?}: {:?}", path, e),
					}
				}
			}
			Paste => {
				let document = self.active_document();
				let shallowest_common_folder = document
					.graphene_document
					.deepest_common_folder(document.selected_layers())
					.expect("While pasting, the selected layers did not exist while attempting to find the appropriate folder path for insertion");

				responses.push_back(
					PasteIntoFolder {
						path: shallowest_common_folder.to_vec(),
						insert_index: -1,
					}
					.into(),
				);
			}
			PasteIntoFolder { path, insert_index } => {
				let paste = |layer: &Layer, responses: &mut VecDeque<_>| {
					log::trace!("Pasting into folder {:?} as index: {}", path, insert_index);
					responses.push_back(
						DocumentOperation::PasteLayer {
							layer: layer.clone(),
							path: path.clone(),
							insert_index,
						}
						.into(),
					)
				};
				if insert_index == -1 {
					for layer in self.copy_buffer.iter() {
						paste(layer, responses)
					}
				} else {
					for layer in self.copy_buffer.iter().rev() {
						paste(layer, responses)
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
			);
			common.extend(select);
		}
		common.extend(self.active_document().actions());
		common
	}
}
