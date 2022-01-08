pub use crate::document::layer_panel::*;
use crate::document::{DocumentMessage, LayerMetadata};
use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use glam::{DAffine2, DVec2};
use graphene::color::Color;
use graphene::document::Document;
use graphene::layers::style;
use graphene::layers::style::Fill;
use graphene::Operation as DocumentOperation;

use graphene::document::Document as GrapheneDocument;
use graphene::layers::style::ViewMode;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[impl_message(Message, DocumentMessage, Artboard)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ArtboardMessage {
	DispatchOperation(Box<DocumentOperation>),
	AddArtboard { top: f64, left: f64, height: f64, width: f64 },
	RenderArtboards,
}

impl From<DocumentOperation> for ArtboardMessage {
	fn from(operation: DocumentOperation) -> Self {
		Self::DispatchOperation(Box::new(operation))
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtboardMessageHandler {
	pub artboards_graphene_document: GrapheneDocument,
	pub artboard_ids: Vec<LayerId>,
}

impl ArtboardMessageHandler {
	pub fn has_artboards(&self) -> bool {
		!self.artboard_ids.is_empty()
	}
}

impl MessageHandler<ArtboardMessage, (&mut LayerMetadata, &Document, &InputPreprocessor)> for ArtboardMessageHandler {
	fn process_action(&mut self, message: ArtboardMessage, _data: (&mut LayerMetadata, &Document, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		// let (layer_metadata, document, ipp) = data;
		use ArtboardMessage::*;
		match message {
			DispatchOperation(operation) => match self.artboards_graphene_document.handle_operation(&operation) {
				Ok(_) => (),
				Err(e) => log::error!("Artboard Error: {:?}", e),
			},
			AddArtboard { top, left, height, width } => {
				let artboard_id = generate_uuid();
				self.artboard_ids.push(artboard_id);

				responses.push_back(
					ArtboardMessage::DispatchOperation(
						DocumentOperation::AddRect {
							path: vec![artboard_id],
							insert_index: -1,
							transform: DAffine2::from_scale_angle_translation(DVec2::new(height, width), 0., DVec2::new(top, left)).to_cols_array(),
							style: style::PathStyle::new(None, Some(Fill::new(Color::WHITE))),
						}
						.into(),
					)
					.into(),
				);
			}
			RenderArtboards => {}
		}

		// Render an infinite canvas if there is no artboards
		if self.artboard_ids.is_empty() {
			responses.push_back(
				FrontendMessage::UpdateArtboards {
					svg: "<rect width=\"100%\" height=\"100%\" style=\"fill:white\" />".to_string(),
				}
				.into(),
			)
		} else {
			responses.push_back(
				FrontendMessage::UpdateArtboards {
					svg: self.artboards_graphene_document.render_root(ViewMode::Normal),
				}
				.into(),
			);
		}
	}

	fn actions(&self) -> ActionList {
		actions!(ArtBoardMessageDiscriminant;)
	}
}
