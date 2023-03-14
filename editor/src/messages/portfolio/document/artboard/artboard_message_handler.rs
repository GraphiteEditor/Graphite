use crate::application::generate_uuid;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

use document_legacy::document::Document as DocumentLegacy;
use document_legacy::layers::style::{self, Fill, RenderData, ViewMode};
use document_legacy::DocumentResponse;
use document_legacy::LayerId;
use document_legacy::Operation as DocumentOperation;
use graphene_core::raster::color::Color;

use glam::DAffine2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ArtboardMessageHandler {
	pub artboards_document: DocumentLegacy,
	pub artboard_ids: Vec<LayerId>,
}

impl MessageHandler<ArtboardMessage, &PersistentData> for ArtboardMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: ArtboardMessage, responses: &mut VecDeque<Message>, persistent_data: &PersistentData) {
		use ArtboardMessage::*;

		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			DispatchOperation(operation) => {
				let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::Normal, None);

				match self.artboards_document.handle_operation(*operation, &render_data) {
					Ok(Some(document_responses)) => {
						for response in document_responses {
							match &response {
								DocumentResponse::LayerChanged { path } => responses.push_back(PropertiesPanelMessage::CheckSelectedWasUpdated { path: path.clone() }.into()),
								DocumentResponse::DeletedLayer { path } => responses.push_back(PropertiesPanelMessage::CheckSelectedWasDeleted { path: path.clone() }.into()),
								DocumentResponse::DocumentChanged => responses.push_back(ArtboardMessage::RenderArtboards.into()),
								_ => {}
							};
							responses.push_back(BroadcastEvent::DocumentIsDirty.into());
						}
					}
					Ok(None) => {}
					Err(e) => error!("Artboard Error: {:?}", e),
				}
			}

			// Messages
			AddArtboard { id, position, size } => {
				let artboard_id = id.unwrap_or_else(generate_uuid);
				self.artboard_ids.push(artboard_id);

				responses.push_back(
					ArtboardMessage::DispatchOperation(
						DocumentOperation::AddRect {
							path: vec![artboard_id],
							insert_index: -1,
							transform: DAffine2::from_scale_angle_translation(size.into(), 0., position.into()).to_cols_array(),
							style: style::PathStyle::new(None, Fill::solid(Color::WHITE)),
						}
						.into(),
					)
					.into(),
				);

				responses.push_back(DocumentMessage::RenderDocument.into());
			}
			ClearArtboards => {
				for &artboard in self.artboard_ids.iter() {
					responses.push_front(ArtboardMessage::DeleteArtboard { artboard }.into());
				}
			}
			DeleteArtboard { artboard } => {
				self.artboard_ids.retain(|&id| id != artboard);

				responses.push_back(ArtboardMessage::DispatchOperation(Box::new(DocumentOperation::DeleteLayer { path: vec![artboard] })).into());

				responses.push_back(DocumentMessage::RenderDocument.into());
			}
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
					let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::Normal, None);
					responses.push_back(
						FrontendMessage::UpdateDocumentArtboards {
							svg: self.artboards_document.render_root(&render_data),
						}
						.into(),
					);
				}
			}
			ResizeArtboard { artboard, position, mut size } => {
				if size.0.abs() == 0. {
					size.0 = size.0.signum();
				}
				if size.1.abs() == 0. {
					size.1 = size.1.signum();
				}

				responses.push_back(
					ArtboardMessage::DispatchOperation(Box::new(DocumentOperation::SetLayerTransform {
						path: vec![artboard],
						transform: DAffine2::from_scale_angle_translation(size.into(), 0., position.into()).to_cols_array(),
					}))
					.into(),
				);

				responses.push_back(DocumentMessage::RenderDocument.into());
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(ArtboardMessageDiscriminant;)
	}
}

impl ArtboardMessageHandler {
	pub fn is_infinite_canvas(&self) -> bool {
		self.artboard_ids.is_empty()
	}
}
