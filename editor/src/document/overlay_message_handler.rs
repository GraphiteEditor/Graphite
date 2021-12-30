pub use crate::document::layer_panel::*;
use crate::document::{DocumentMessage, LayerData};
use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use graphene::document::Document;
use graphene::Operation as DocumentOperation;

use graphene::document::Document as GrapheneDocument;
use graphene::layers::style::ViewMode;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

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
	overlay_path_mapping: HashMap<Vec<LayerId>, Vec<LayerId>>,
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

		// Render overlays
		responses.push_back(
			FrontendMessage::UpdateOverlays {
				svg: self.overlays_graphene_document.render_root(ViewMode::Normal),
			}
			.into(),
		);
	}

	fn actions(&self) -> ActionList {
		actions!(OverlayMessageDiscriminant;
			ClearAllOverlays
		)
	}
}
