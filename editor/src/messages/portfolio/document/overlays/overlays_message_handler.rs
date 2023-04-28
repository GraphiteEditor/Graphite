use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

use document_legacy::document::Document as DocumentLegacy;
use document_legacy::layers::style::{RenderData, ViewMode};

#[derive(Debug, Clone, Default)]
pub struct OverlaysMessageHandler {
	pub overlays_document: DocumentLegacy,
}

impl MessageHandler<OverlaysMessage, (bool, &PersistentData, &InputPreprocessorMessageHandler)> for OverlaysMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: OverlaysMessage, responses: &mut VecDeque<Message>, (overlays_visible, persistent_data, ipp): (bool, &PersistentData, &InputPreprocessorMessageHandler)) {
		use OverlaysMessage::*;

		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			DispatchOperation(operation) => {
				let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::Normal, Some(ipp.document_bounds()));

				match self.overlays_document.handle_operation(*operation, &render_data) {
					Ok(_) => responses.add(OverlaysMessage::Rerender),
					Err(e) => error!("OverlaysError: {:?}", e),
				}
			}

			// Messages
			ClearAllOverlays => {
				self.overlays_document = DocumentLegacy::default();
			}
			Rerender =>
			// Render overlays
			{
				responses.add(FrontendMessage::UpdateDocumentOverlays {
					svg: if overlays_visible {
						let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::Normal, Some(ipp.document_bounds()));
						self.overlays_document.render_root(&render_data)
					} else {
						String::from("")
					},
				})
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(OverlaysMessageDiscriminant;
			ClearAllOverlays,
		)
	}
}
