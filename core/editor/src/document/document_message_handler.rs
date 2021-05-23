use crate::{input::{InputPreprocessor, mouse::ViewportPosition}, message_prelude::*};
use document_core::{DocumentResponse, LayerId, Operation as DocumentOperation};

use crate::document::Document;
use std::collections::VecDeque;

#[impl_message(Message, Document)]
#[derive(PartialEq, Clone, Debug)]
pub enum DocumentMessage {
	DocumentResize(ViewportPosition),
	DispatchOperation(DocumentOperation),
	SelectLayer(Vec<LayerId>),
	DeleteLayer(Vec<LayerId>),
	AddFolder(Vec<LayerId>),
	RenameLayer(Vec<LayerId>, String),
	ToggleLayerVisibility(Vec<LayerId>),
	ToggleLayerExpansion(Vec<LayerId>),
	SelectDocument(usize),
	RenderDocument,
	Undo,
}

impl From<DocumentOperation> for DocumentMessage {
	fn from(operation: DocumentOperation) -> DocumentMessage {
		Self::DispatchOperation(operation)
	}
}
impl From<DocumentOperation> for Message {
	fn from(operation: DocumentOperation) -> Message {
		DocumentMessage::DispatchOperation(operation).into()
	}
}

#[derive(Debug, Clone)]
pub struct DocumentMessageHandler {
	documents: Vec<Document>,
	active_document: usize,
}

impl DocumentMessageHandler {
	pub fn active_document(&self) -> &Document {
		&self.documents[self.active_document]
	}
	pub fn active_document_mut(&mut self) -> &mut Document {
		&mut self.documents[self.active_document]
	}
	fn filter_document_responses(&self, document_responses: &mut Vec<DocumentResponse>) -> bool {
		let len = document_responses.len();
		document_responses.retain(|response| !matches!(response, DocumentResponse::DocumentChanged));
		document_responses.len() != len
	}
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			documents: vec![Document::default()],
			active_document: 0,
		}
	}
}

impl MessageHandler<DocumentMessage, &mut InputPreprocessor> for DocumentMessageHandler {
	fn process_action(&mut self, message: DocumentMessage, data: &mut InputPreprocessor, responses: &mut VecDeque<Message>) {
		use DocumentMessage::*;
		match message {
			DocumentResize(size) => data.canvas_transform.size = size,
			DeleteLayer(path) => responses.push_back(DocumentOperation::DeleteLayer { path }.into()),
			AddFolder(path) => responses.push_back(DocumentOperation::AddFolder { path }.into()),
			SelectDocument(id) => {
				assert!(id < self.documents.len(), "Tried to select a document that was not initialized");
				self.active_document = id;
			}
			ToggleLayerVisibility(path) => {
				responses.push_back(DocumentOperation::ToggleVisibility { path }.into());
			}
			Undo => {
				// this is a temporary fix and will be addressed by #123
				if let Some(id) = self.active_document().document.root.list_layers().last() {
					responses.push_back(DocumentOperation::DeleteLayer { path: vec![*id] }.into())
				}
			}
			DispatchOperation(op) => {
				if let Ok(Some(mut document_responses)) = self.active_document_mut().document.handle_operation(op) {
					let canvas_dirty = self.filter_document_responses(&mut document_responses);
					responses.extend(document_responses.into_iter().map(Into::into));
					if canvas_dirty {
						responses.push_back(RenderDocument.into())
					}
				}
			}
			RenderDocument => responses.push_back(
				FrontendMessage::UpdateCanvas {
					document: self.active_document_mut().document.render_root(),
				}
				.into(),
			),
			message => todo!("document_action_handler does not implement: {}", message.to_discriminant().global_name()),
		}
	}
	advertise_actions!(DocumentMessageDiscriminant; Undo, RenderDocument);
}
