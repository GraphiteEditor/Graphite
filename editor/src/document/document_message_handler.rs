use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use graphene::layers::Layer;
use graphene::{LayerId, Operation as DocumentOperation};
use log::warn;

use std::collections::VecDeque;

use super::DocumentMessageHandler;

#[impl_message(Message, Documents)]
#[derive(PartialEq, Clone, Debug)]
pub enum DocumentsMessage {
	CopySelectedLayers,
	PasteLayers {
		path: Vec<LayerId>,
		insert_index: isize,
	},
	SelectDocument(usize),
	CloseDocument(usize),
	#[child]
	Document(DocumentMessage),
	CloseActiveDocumentWithConfirmation,
	CloseAllDocumentsWithConfirmation,
	CloseAllDocuments,
	NewDocument,
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

					let lp = self.active_document_mut().layer_panel(&[]).expect("Could not get panel for active doc");
					responses.push_back(FrontendMessage::ExpandFolder { path: Vec::new(), children: lp }.into());
					responses.push_back(
						FrontendMessage::SetActiveDocument {
							document_index: self.active_document_index,
						}
						.into(),
					);
					responses.extend([
						FrontendMessage::UpdateCanvas {
							document: self.active_document_mut().document.render_root(),
						}
						.into(),
						FrontendMessage::UpdateScrollbars {
							bounds: {
								let bounds = self.active_document_mut().document.visible_layers_bounding_box();
								let bounds = bounds.unwrap_or([glam::DVec2::ZERO, glam::DVec2::ZERO]);
								[bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y]
							},
							position: self.active_document_mut().document.root.transform.translation.into(),
							viewport_size: ipp.viewport_size.as_f64().into(),
						}
						.into(),
					]);
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
				let digits = ('0'..='9').collect::<Vec<char>>();
				let mut doc_title_numbers = self
					.documents
					.iter()
					.map(|d| {
						if d.name.ends_with(digits.as_slice()) {
							let (_, number) = d.name.split_at(17);
							number.trim().parse::<usize>().unwrap()
						} else {
							1
						}
					})
					.collect::<Vec<usize>>();
				doc_title_numbers.sort_unstable();
				let mut new_doc_title_num = 1;
				while new_doc_title_num <= self.documents.len() {
					if new_doc_title_num != doc_title_numbers[new_doc_title_num - 1] {
						break;
					}
					new_doc_title_num += 1;
				}
				let name = match new_doc_title_num {
					1 => "Untitled Document".to_string(),
					_ => format!("Untitled Document {}", new_doc_title_num),
				};

				self.active_document_index = self.documents.len();
				let new_document = DocumentMessageHandler::with_name(name);
				self.documents.push(new_document);

				// Send the new list of document tab names
				let open_documents = self.documents.iter().map(|doc| doc.name.clone()).collect();
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());

				responses.push_back(
					FrontendMessage::ExpandFolder {
						path: Vec::new(),
						children: Vec::new(),
					}
					.into(),
				);
				responses.push_back(SelectDocument(self.active_document_index).into());
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
			CopySelectedLayers => {
				let paths = self.active_document().selected_layers_sorted();
				self.copy_buffer.clear();
				for path in paths {
					match self.active_document().document.layer(&path).map(|t| t.clone()) {
						Ok(layer) => {
							self.copy_buffer.push(layer);
						}
						Err(e) => warn!("Could not access selected layer {:?}: {:?}", path, e),
					}
				}
			}
			PasteLayers { path, insert_index } => {
				let paste = |layer: &Layer, responses: &mut VecDeque<_>| {
					log::trace!("pasting into folder {:?} as index: {}", path, insert_index);
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
			PasteLayers,
		);

		if self.active_document().layer_data.values().any(|data| data.selected) {
			let select = actions!(DocumentsMessageDiscriminant;
				CopySelectedLayers,
			);
			common.extend(select);
		}
		common.extend(self.active_document().actions());
		common
	}
}
