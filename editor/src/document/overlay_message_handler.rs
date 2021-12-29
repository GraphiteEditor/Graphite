pub use crate::document::layer_panel::*;
use crate::document::{DocumentMessage, LayerData};
use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use graphene::document::Document;
use graphene::{DocumentResponse, Operation as DocumentOperation};

use graphene::document::Document as GrapheneDocument;
use graphene::layers::style::ViewMode;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[impl_message(Message, DocumentMessage, Overlay)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum OverlayMessage {
	DispatchOperation(Box<DocumentOperation>),
	AssociatedDispatchOperation(Box<DocumentOperation>, Vec<LayerId>),
	UpdateAssociatedoOverlay(Vec<LayerId>),
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
				Ok(_) => (), // log::debug!("OverlayOperation {:?}", operation),
				Err(e) => log::error!("OverlayError: {:?}", e),
			},
			AssociatedDispatchOperation(operation, graphene_document_path) => match self.overlays_graphene_document.handle_operation(&operation) {
				Ok(Some(responses)) => {
					for response in responses {
						if let DocumentResponse::CreatedLayer { path } = response {
							self.overlay_path_mapping.insert(graphene_document_path.clone(), path);
						}
					}
				}
				Err(e) => log::error!("OverlayError: {:?}", e),
				_ => {}
			},
			UpdateAssociatedoOverlay(document_path) => {
				let overlay_path = self
					.overlay_path_mapping
					.get(&document_path)
					.expect("Couldn't find associated path! Possibly using AssociatedDispatchOperation instead of DispatchOperation will resolve this.");
				let document_layer = document.layer(&document_path).expect("Couldn't find document layer");
				responses.push_back(
					DocumentOperation::SetLayerVisibility {
						path: overlay_path.clone(),
						visible: document_layer.visible,
					}
					.into(),
				);
			}
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
