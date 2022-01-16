use super::layer_panel::LayerMetadata;
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;

use graphene::document::Document;
use graphene::document::Document as GrapheneDocument;
use graphene::layers::style::ViewMode;

#[derive(Debug, Clone)]
pub struct OverlaysMessageHandler {
	pub overlays_graphene_document: GrapheneDocument,
	pub visible: bool,
}

impl Default for OverlaysMessageHandler {
	fn default() -> Self {
		Self {
			overlays_graphene_document: Default::default(),
			visible: true,
		}
	}
}

impl MessageHandler<OverlaysMessage, (&mut LayerMetadata, &Document, &InputPreprocessorMessageHandler)> for OverlaysMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: OverlaysMessage, _data: (&mut LayerMetadata, &Document, &InputPreprocessorMessageHandler), responses: &mut VecDeque<Message>) {
		use OverlaysMessage::*;

		// let (layer_metadata, document, ipp) = data;
		#[remain::sorted]
		match message {
			ClearAllOverlays => todo!(),
			DispatchOperation(operation) => match self.overlays_graphene_document.handle_operation(&operation) {
				Ok(_) => (),
				Err(e) => log::error!("OverlaysError: {:?}", e),
			},
			SetOverlaysVisible { visible } => {
				self.visible = visible;
			}
		}

		// Render overlays
		responses.push_back(
			FrontendMessage::UpdateDocumentOverlays {
				svg: if self.visible {
					self.overlays_graphene_document.render_root(ViewMode::Normal)
				} else {
					String::from("")
				},
			}
			.into(),
		);
	}

	fn actions(&self) -> ActionList {
		actions!(OverlaysMessageDiscriminant; ClearAllOverlays, SetOverlaysVisible)
	}
}
