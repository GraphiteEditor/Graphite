use document_core::{DocumentResponse, LayerId, Operation as DocumentOperation};
use proc_macros::MessageImpl;

use super::{AsMessage, Message, MessageDiscriminant, MessageHandler};
use crate::{document::Document, events::ToolResponse, SvgDocument};

#[derive(MessageImpl, PartialEq, Clone)]
#[message(Message, Message, Document)]
pub enum DocumentMessage {
	Operation(DocumentOperation),
	SelectLayer(Vec<LayerId>),
	DeleteLayer(Vec<LayerId>),
	AddFolder(Vec<LayerId>),
	RenameLayer(Vec<LayerId>, String),
	ToggleLayerVisibility(Vec<LayerId>),
	ToggleLayerExpansion(Vec<LayerId>),
	SelectDocument(usize),
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
			DeleteLayer(path) => responses.push(DocumentOperation::DeleteLayer { path: path.clone() }.into()),
			AddFolder(path) => responses.push(DocumentOperation::AddFolder { path: path.clone() }.into()),
			SelectDocument(id) => self.active_document = id,
			Undo => {
				// this is a temporary fix and will be addressed by #123
				if let Some(id) = self.active_document().document.root.list_layers().last() {
					responses.push(DocumentOperation::DeleteLayer { path: vec![*id] }.into())
				}
			}
			_ => (),
		}

		/*
		let mut document_responses = self.dispatch_operations(doc, operations.drain(..));
		let canvas_dirty = self.filter_document_responses(&mut document_responses);
		responses.extend(document_responses.drain(..).map(Into::into));
		if canvas_dirty {
			responses.push(ToolResponse::UpdateCanvas { document: doc.render_root() }.into())
		}

		consumed*/
	}
	actions_fn!(DocumentMessageDiscriminant::Undo, DocumentMessageDiscriminant::Redo, DocumentMessageDiscriminant::Save);
}
