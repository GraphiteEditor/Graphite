use document_core::{DocumentResponse, LayerId, Operation as DocumentOperation};
use proc_macros::MessageImpl;

use super::{AsMessage, Message, MessageDiscriminant, MessageHandler};
use crate::{events::ToolResponse, SvgDocument};
use crate::{
	tools::{DocumentToolData, ToolActionHandlerData},
	EditorError,
};

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

#[derive(Debug, Default, Clone)]
pub struct DocumentActionHandler {}

impl MessageHandler<DocumentMessage, &mut SvgDocument> for DocumentActionHandler {
	fn process_action(&mut self, message: DocumentMessage, document: &mut SvgDocument, responses: &mut Vec<Message>) {
		use DocumentMessage::*;
		match message {
			DeleteLayer(path) => responses.push(DocumentOperation::DeleteLayer { path: path.clone() }.into()),
			AddFolder(path) => responses.push(DocumentOperation::AddFolder { path: path.clone() }.into()),
			Undo => {
				// this is a temporary fix and will be addressed by #123
				if let Some(id) = document.root.list_layers().last() {
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
