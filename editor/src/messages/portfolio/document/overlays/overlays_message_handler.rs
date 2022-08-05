use crate::messages::prelude::*;

use graphene::document::Document as GrapheneDocument;
use graphene::layers::style::{RenderData, ViewMode};
use graphene::layers::text_layer::FontCache;

#[derive(Debug, Clone, Default)]
pub struct OverlaysMessageHandler {
	pub overlays_graphene_document: GrapheneDocument,
}

impl MessageHandler<OverlaysMessage, (bool, &FontCache, &InputPreprocessorMessageHandler)> for OverlaysMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: OverlaysMessage, (overlays_visible, font_cache, ipp): (bool, &FontCache, &InputPreprocessorMessageHandler), responses: &mut VecDeque<Message>) {
		use OverlaysMessage::*;

		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			DispatchOperation(operation) => match self.overlays_graphene_document.handle_operation(*operation, font_cache) {
				Ok(_) => responses.push_back(OverlaysMessage::Rerender.into()),
				Err(e) => log::error!("OverlaysError: {:?}", e),
			},

			// Messages
			ClearAllOverlays => todo!(),
			Rerender =>
			// Render overlays
			{
				responses.push_back(
					FrontendMessage::UpdateDocumentOverlays {
						svg: if overlays_visible {
							let render_data = RenderData::new(ViewMode::Normal, font_cache, Some(ipp.document_bounds()), false);
							self.overlays_graphene_document.render_root(render_data)
						} else {
							String::from("")
						},
					}
					.into(),
				)
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(OverlaysMessageDiscriminant;
			ClearAllOverlays,
		)
	}
}
