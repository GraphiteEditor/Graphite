use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use graphene::layers::Layer;
use graphene::{LayerId, Operation as DocumentOperation};
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

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
	NewDocument,
	OpenDocument,
	OpenDocumentFile(String, String),
	GetOpenDocumentsList,
	NextDocument,
	PrevDocument,
}

#[derive(Debug, Clone)]
pub struct DocumentsMessageHandler {
	documents: Vec<DocumentMessageHandler>,
	document_ids: Vec<usize>,
	free_ids: Vec<usize>,
	active_document_id: usize,
	copy_buffer: Vec<Layer>,
}

impl DocumentsMessageHandler {
	pub fn active_document(&self) -> &DocumentMessageHandler {
		let index = self.map_index_to_id(self.active_document_id);
		&self.documents[index]
	}

	pub fn active_document_mut(&mut self) -> &mut DocumentMessageHandler {
		let index = self.map_index_to_id(self.active_document_id);
		&mut self.documents[index]
	}

	fn map_index_to_id(&self, id: usize) -> usize {
		for i in 0..self.document_ids.len() {
			if self.document_ids[i] == id {
				return i;
			}
		}
		0
	}

	fn get_free_id(&mut self) -> usize {
		if self.free_ids.len() > 0 {
			// Treat the vector like a queue
			let id = self.free_ids[0];
			self.free_ids.remove(0);
			return id;
		}
		self.document_ids.len()
	}

	fn generate_new_document_name(&self) -> String {
		let mut doc_title_numbers = self
			.documents
			.iter()
			.filter_map(|d| {
				d.name
					.rsplit_once(DEFAULT_DOCUMENT_NAME)
					.map(|(prefix, number)| (prefix.is_empty()).then(|| number.trim().parse::<isize>().ok()).flatten().unwrap_or(1))
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
		self.active_document_id = self.get_free_id();
		self.document_ids.push(self.active_document_id);
		self.documents.push(new_document);

		// Send the new list of document tab names
		let open_documents = self.documents.iter().map(|doc| doc.name.clone()).collect();
		responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());
		
		let index = self.map_id_to_index(self.active_document_id);
		responses.push_back(DocumentsMessage::SelectDocument(index).into());
		responses.push_back(DocumentMessage::RenderDocument.into());
		responses.push_back(DocumentMessage::DocumentStructureChanged.into());
		for layer in self.active_document().layer_data.keys() {
			responses.push_back(DocumentMessage::LayerChanged(layer.clone()).into());
		}
	}
}

impl Default for DocumentsMessageHandler {
	fn default() -> Self {
		Self {
			documents: vec![DocumentMessageHandler::default()],
			document_ids: vec![0],
			free_ids: vec![],
			active_document_id: 0,
			copy_buffer: vec![],
		}
	}
}

impl MessageHandler<DocumentsMessage, &InputPreprocessor> for DocumentsMessageHandler {
	fn process_action(&mut self, message: DocumentsMessage, ipp: &InputPreprocessor, responses: &mut VecDeque<Message>) {
		use DocumentMessage::*;
		use DocumentsMessage::*;
		match message {
			Document(message) => self.active_document_mut().process_action(message, ipp, responses),
			SelectDocument(index) => {
				assert!(index < self.documents.len(), "Tried to select a document that was not initialized");
				let id = self.document_ids[index];
				self.active_document_id = id;
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
						document_index: self.map_index_to_id(self.active_document_id),
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

				// Recycle old ids
				for id in &self.document_ids {
					self.free_ids.push(*id);
				}
				self.document_ids.clear();

				// Create a new blank document
				responses.push_back(NewDocument.into());
			}
			CloseDocument(index) => {
				assert!(index < self.documents.len(), "Tried to select a document that was not initialized");
				// Remove doc from the backend store; use `id` as client tabs and backend documents will be in sync
				let id = self.document_ids[index];
				// Map the ID to an index and remove the document
				self.documents.remove(index);
				self.document_ids.remove(index);

				// Push the removed id into a free id
				self.free_ids.push(id);

				// Send the new list of document tab names
				let open_documents = self.documents.iter().map(|doc| doc.name.clone()).collect();
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());

				// Last tab was closed, so create a new blank tab
				if self.documents.is_empty() {
					self.active_document_id = self.get_free_id();
					responses.push_back(NewDocument.into());
				}
				// if the ID we're closing is the same as the active ID
				else {
					// Clamp the index between the total number of documents
					let doc_index = if index >= self.document_ids.len() { self.document_ids.len() - 1 } else { index };
					self.active_document_id = self.document_ids[doc_index];

					responses.push_back(DocumentMessage::DocumentStructureChanged.into());
					responses.push_back(
						FrontendMessage::SetActiveDocument {
							document_index: doc_index,
						}
						.into(),
					);
					responses.push_back(
						FrontendMessage::UpdateCanvas {
							document: self.active_document_mut().graphene_document.render_root(),
						}
						.into(),
					);
				}
			}
			NewDocument => {
				let name = self.generate_new_document_name();
				let new_document = DocumentMessageHandler::with_name(name);
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
			GetOpenDocumentsList => {
				// Send the list of document tab names
				let open_documents = self.documents.iter().map(|doc| doc.name.clone()).collect();
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());
			}
			NextDocument => {
				let next = (self.map_index_to_id(self.active_document_id) + 1) % self.document_ids.len();
				responses.push_back(SelectDocument(next).into());
			}
			PrevDocument => {
				let len = self.document_ids.len();
				let prev = (self.map_index_to_id(self.active_document_id) + len - 1) % len;
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
