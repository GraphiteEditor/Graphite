use super::message::prelude::*;
use document_core::{DocumentResponse, LayerId, Operation as DocumentOperation};

use super::MessageHandler;
use crate::document::Document;
use graphite_proc_macros::*;

#[impl_message(Message, Document)]
#[derive(PartialEq, Clone, Debug)]
pub enum DocumentMessage {
	Operation(DocumentOperation),
	SelectLayer(Vec<LayerId>),
	DeleteLayer(Vec<LayerId>),
	AddFolder(Vec<LayerId>),
	RenameLayer(Vec<LayerId>, String),
	ToggleLayerVisibility(Vec<LayerId>),
	ToggleLayerExpansion(Vec<LayerId>),
	SelectDocument(usize),
	RenderDocument,
	Undo,
	Redo,
	Save,
}

impl From<DocumentOperation> for DocumentMessage {
	fn from(operation: DocumentOperation) -> DocumentMessage {
		Self::Operation(operation)
	}
}
impl From<DocumentOperation> for Message {
	fn from(operation: DocumentOperation) -> Message {
		DocumentMessage::Operation(operation).into()
	}
}

#[derive(Debug, Clone)]
pub struct DocumentActionHandler {
	documents: Vec<Document>,
	active_document: usize,
}

impl DocumentActionHandler {
	pub fn active_document(&self) -> &Document {
		&self.documents[self.active_document]
	}
	pub fn active_document_mut(&mut self) -> &mut Document {
		&mut self.documents[self.active_document]
	}
	fn filter_document_responses(&self, document_responses: &mut Vec<DocumentResponse>) -> bool {
		//let changes = document_responses.drain_filter(|x| x == DocumentResponse::DocumentChanged);
		let mut canvas_dirty = false;
		let mut i = 0;
		while i < document_responses.len() {
			if matches!(document_responses[i], DocumentResponse::DocumentChanged) {
				canvas_dirty = true;
				document_responses.remove(i);
			} else {
				i += 1;
			}
		}
		canvas_dirty
	}
}

impl Default for DocumentActionHandler {
	fn default() -> Self {
		Self {
			documents: vec![Document::default()],
			active_document: 0,
		}
	}
}

impl MessageHandler<DocumentMessage, ()> for DocumentActionHandler {
	fn process_action(&mut self, message: DocumentMessage, _data: (), responses: &mut Vec<Message>) {
		use DocumentMessage::*;
		match message {
			DeleteLayer(path) => responses.push(DocumentOperation::DeleteLayer { path }.into()),
			AddFolder(path) => responses.push(DocumentOperation::AddFolder { path }.into()),
			SelectDocument(id) => self.active_document = id,
			Undo => {
				// this is a temporary fix and will be addressed by #123
				if let Some(id) = self.active_document().document.root.list_layers().last() {
					responses.push(DocumentOperation::DeleteLayer { path: vec![*id] }.into())
				}
			}
			Operation(op) => {
				if let Ok(Some(mut document_responses)) = self.active_document_mut().document.handle_operation(op) {
					let canvas_dirty = self.filter_document_responses(&mut document_responses);
					responses.extend(document_responses.drain(..).map(Into::into));
					if canvas_dirty {
						responses.push(RenderDocument.into())
					}
				}
			}
			RenderDocument => responses.push(
				FrontendMessage::UpdateCanvas {
					document: self.active_document_mut().document.render_root(),
				}
				.into(),
			),
			_ => (),
		}
	}
	actions_fn!(DocumentMessageDiscriminant::Undo, DocumentMessageDiscriminant::Redo, DocumentMessageDiscriminant::Save);
}
