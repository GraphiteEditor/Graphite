pub use crate::document::layer_panel::*;
use crate::document::{DocumentMessage, LayerData};
use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use graphene::document::Document;
use graphene::Operation as DocumentOperation;

use graphene::document::Document as GrapheneDocument;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[impl_message(Message, DocumentMessage, Overlay)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum OverlayMessage {
	DispatchOperation(Box<DocumentOperation>),
	ClearAllOverlays,
}

impl From<DocumentOperation> for OverlayMessage {
	fn from(operation: DocumentOperation) -> OverlayMessage {
		Self::DispatchOperation(Box::new(operation))
	}
}

#[derive(Debug, Clone, Default)]
pub struct OverlayMessageHandler {
	pub overlays_graphene_document: GrapheneDocument,
}

impl MessageHandler<OverlayMessage, (&mut LayerData, &Document, &InputPreprocessor)> for OverlayMessageHandler {
	fn process_action(&mut self, message: OverlayMessage, data: (&mut LayerData, &Document, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (layerdata, document, ipp) = data;
		use OverlayMessage::*;
		match message {
			DispatchOperation(operation) => match self.overlays_graphene_document.handle_operation(&operation) {
				Ok(_) => (),
				Err(e) => log::error!("OverlayError: {:?}", e),
			},
			ClearAllOverlays => todo!(),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(OverlayMessageDiscriminant;
			ClearAllOverlays,
		)
	}
}
