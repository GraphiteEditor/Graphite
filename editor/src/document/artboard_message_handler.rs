use super::layer_panel::LayerMetadata;
use super::{ArtboardMessage, DocumentMessage};
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;

use graphene::color::Color;
use graphene::document::Document as GrapheneDocument;
use graphene::layers::style::{self, Fill, ViewMode};
use graphene::Operation as DocumentOperation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtboardMessageHandler {
	pub artboards_graphene_document: GrapheneDocument,
	pub artboard_ids: Vec<LayerId>,
}

impl ArtboardMessageHandler {
	pub fn is_infinite_canvas(&self) -> bool {
		self.artboard_ids.is_empty()
	}
}

impl MessageHandler<ArtboardMessage, (&mut LayerMetadata, &GrapheneDocument, &InputPreprocessorMessageHandler)> for ArtboardMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: ArtboardMessage, _data: (&mut LayerMetadata, &GrapheneDocument, &InputPreprocessorMessageHandler), responses: &mut VecDeque<Message>) {
		// let (layer_metadata, document, ipp) = data;
		use ArtboardMessage::*;
		#[remain::sorted]
		match message {
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

				responses.push_back(DocumentMessage::RenderDocument.into());
			}
			DispatchOperation(operation) => match self.artboards_graphene_document.handle_operation(&operation) {
				Ok(_) => (),
				Err(e) => log::error!("Artboard Error: {:?}", e),
			},
			RenderArtboards => {
				// Render an infinite canvas if there are no artboards
				if self.artboard_ids.is_empty() {
					responses.push_back(
						FrontendMessage::UpdateDocumentArtboards {
							svg: r##"<rect width="100%" height="100%" fill="#ffffff" />"##.to_string(),
						}
						.into(),
					)
				} else {
					responses.push_back(
						FrontendMessage::UpdateDocumentArtboards {
							svg: self.artboards_graphene_document.render_root(ViewMode::Normal),
						}
						.into(),
					);
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(ArtboardMessageDiscriminant;)
	}
}
