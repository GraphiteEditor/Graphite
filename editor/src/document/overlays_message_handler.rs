use super::layer_panel::LayerMetadata;
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;

use graphene::document::Document;
use graphene::document::Document as GrapheneDocument;
use graphene::layers::style::ViewMode;

#[derive(Debug, Clone, Default)]
pub struct OverlaysMessageHandler {
	pub overlays_graphene_document: GrapheneDocument,
}

impl MessageHandler<OverlaysMessage, (&mut LayerMetadata, &Document, &InputPreprocessorMessageHandler)> for OverlaysMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: OverlaysMessage, _data: (&mut LayerMetadata, &Document, &InputPreprocessorMessageHandler), responses: &mut VecDeque<Message>) {
		// let (layer_metadata, document, ipp) = data;
		use OverlaysMessage::*;
		#[remain::sorted]
		match message {
			ClearAllOverlays => todo!(),
			DispatchOperation(operation) => match self.overlays_graphene_document.handle_operation(&operation) {
				Ok(_) => (),
				Err(e) => log::error!("OverlaysError: {:?}", e),
			},
		}

		// Render overlays
		responses.push_back(
			FrontendMessage::UpdateDocumentOverlays {
				svg: self.overlays_graphene_document.render_root(ViewMode::Normal),
			}
			.into(),
		);
	}

	fn actions(&self) -> ActionList {
		actions!(OverlaysMessageDiscriminant; ClearAllOverlays)
	}
}
