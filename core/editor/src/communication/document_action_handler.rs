use document_core::{DocumentResponse, LayerId};

use super::{Action, InputPreprocessor, MessageHandler, Operation, Response};
use crate::{events::ToolResponse, SvgDocument};
use crate::{
	tools::{DocumentToolData, ToolActionHandlerData},
	EditorError,
};

use strum_macros::{AsRefStr, Display, EnumDiscriminants, EnumIter, EnumString};

#[derive(Debug, Clone, Display, AsRefStr, EnumDiscriminants, EnumIter, EnumString)]
pub enum DocumentMessage {
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
impl AsMessage for DocumentMessage {
	fn as_discriminant(&self) -> MessageDiscriminant {
		match self {
			SelectLayer(_) => SelectLayer,
		}
	}
}
pub enum DocumentMessageDiscriminant {
	SelectLayer,
	DeleteLayer,
	Undo,
	Redo,
	Save,
}

#[derive(Debug, Default, Clone)]
pub struct DocumentActionHandler {}

impl MessageHandler<DocumentMessage, &mut SvgDocument> for DocumentActionHandler {
	fn process_action(&mut self, action: DocumentMessage, document: &mut SvgDocument, responses: &mut Vec<Response>) {
		use DocumentMessage::*;
		match action {
			DeleteLayer(path) => responses.push(Operation::DeleteLayer { path: path.clone() }.into()),
			AddFolder(path) => responses.push(Operation::AddFolder { path: path.clone() }.into()),
			Undo => {
				// this is a temporary fix and will be addressed by #123
				if let Some(id) = document.root.list_layers().last() {
					responses.push(Operation::DeleteLayer { path: vec![*id] }.into())
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
	actions_fn!(Action::Undo, Action::DeleteLayer(vec![]), Action::AddFolder(vec![]));
}
