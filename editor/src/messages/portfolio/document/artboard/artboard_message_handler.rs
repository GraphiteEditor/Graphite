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
			DispatchOperation(operation) => match self.artboards_document.handle_operation(*operation) {
				Ok(Some(document_responses)) => {
					for response in document_responses {
						match &response {
							DocumentResponse::LayerChanged { path } => responses.add(PropertiesPanelMessage::CheckSelectedWasUpdated { path: path.clone() }),
							DocumentResponse::DeletedLayer { path } => responses.add(PropertiesPanelMessage::CheckSelectedWasDeleted { path: path.clone() }),
							DocumentResponse::DocumentChanged => responses.add(ArtboardMessage::RenderArtboards),
							_ => {}
						};
						responses.add(BroadcastEvent::DocumentIsDirty);
					}
				}
				Ok(None) => {}
				Err(e) => error!("Artboard Error: {:?}", e),
			},

			// Messages
			AddArtboard { id, position, size } => {
				let artboard_id = id.unwrap_or_else(generate_uuid);
				self.artboard_ids.push(artboard_id);

				responses.add(ArtboardMessage::DispatchOperation(
					DocumentOperation::AddRect {
						path: vec![artboard_id],
						insert_index: -1,
						transform: DAffine2::from_scale_angle_translation(size.into(), 0., position.into()).to_cols_array(),
						style: style::PathStyle::new(None, Fill::solid(Color::WHITE)),
					}
					.into(),
				));

				responses.add(DocumentMessage::RenderDocument);
			}
			ClearArtboards => {
				// TODO: Make this remove the artboard layers from the graph (and cleanly reconnect the artwork)
				responses.add(DialogMessage::RequestComingSoonDialog { issue: None });
				// for &artboard in self.artboard_ids.iter() {
				// 	responses.add_front(ArtboardMessage::DeleteArtboard { artboard });
				// }
			}
			DeleteArtboard { artboard } => {
				self.artboard_ids.retain(|&id| id != artboard);

				responses.add(ArtboardMessage::DispatchOperation(Box::new(DocumentOperation::DeleteLayer { path: vec![artboard] })));

				responses.add(DocumentMessage::RenderDocument);
			}
			RenderArtboards => {
				// Render an infinite canvas if there are no artboards
				if self.artboard_ids.is_empty() {
					responses.add(FrontendMessage::UpdateDocumentArtboards {
						svg: r##"<rect width="100%" height="100%" fill="#ffffff" />"##.to_string(),
					})
				} else {
					let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::Normal, None);
					responses.add(FrontendMessage::UpdateDocumentArtboards {
						svg: self.artboards_document.render_root(&render_data),
					});
				}
			}
			ResizeArtboard { artboard, position, mut size } => {
				if size.0.abs() == 0. {
					size.0 = size.0.signum();
				}
				if size.1.abs() == 0. {
					size.1 = size.1.signum();
				}

				responses.add(ArtboardMessage::DispatchOperation(Box::new(DocumentOperation::SetLayerTransform {
					path: vec![artboard],
					transform: DAffine2::from_scale_angle_translation(size.into(), 0., position.into()).to_cols_array(),
				})));

				responses.add(DocumentMessage::RenderDocument);
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
