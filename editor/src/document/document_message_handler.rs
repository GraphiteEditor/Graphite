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
	active_document_index: usize,
	copy_buffer: Vec<Layer>,
}

impl DocumentsMessageHandler {
	pub fn active_document(&self) -> &DocumentMessageHandler {
		&self.documents[self.active_document_index]
	}
	pub fn active_document_mut(&mut self) -> &mut DocumentMessageHandler {
		&mut self.documents[self.active_document_index]
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
		self.active_document_index = self.documents.len();
		self.documents.push(new_document);

		// Send the new list of document tab names
		let open_documents = self.documents.iter().map(|doc| doc.name.clone()).collect();
		responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());

		responses.push_back(DocumentsMessage::SelectDocument(self.active_document_index).into());
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
			active_document_index: 0,
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
			SelectDocument(id) => {
				assert!(id < self.documents.len(), "Tried to select a document that was not initialized");
				self.active_document_index = id;
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

				// Create a new blank document
				responses.push_back(NewDocument.into());
			}
			CloseDocument(id) => {
				assert!(id < self.documents.len(), "Tried to select a document that was not initialized");
				// Remove doc from the backend store; use `id` as client tabs and backend documents will be in sync
				self.documents.remove(id);

				// Send the new list of document tab names
				let open_documents = self.documents.iter().map(|doc| doc.name.clone()).collect();
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());

				// Last tab was closed, so create a new blank tab
				if self.documents.is_empty() {
					self.active_document_index = 0;
					responses.push_back(NewDocument.into());
				}
				// The currently selected doc is being closed
				else if id == self.active_document_index {
					// The currently selected tab was the rightmost tab
					if id == self.documents.len() {
						self.active_document_index -= 1;
					}

					responses.push_back(DocumentMessage::DocumentStructureChanged.into());
					responses.push_back(
						FrontendMessage::SetActiveDocument {
							document_index: self.active_document_index,
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
				// Active doc will move one space to the left
				else if id < self.active_document_index {
					self.active_document_index -= 1;
					responses.push_back(
						FrontendMessage::SetActiveDocument {
							document_index: self.active_document_index,
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
				let id = (self.active_document_index + 1) % self.documents.len();
				responses.push_back(SelectDocument(id).into());
			}
			PrevDocument => {
				let id = (self.active_document_index + self.documents.len() - 1) % self.documents.len();
				responses.push_back(SelectDocument(id).into());
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
