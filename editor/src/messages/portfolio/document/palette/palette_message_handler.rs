use crate::messages::portfolio::document::palette::PaletteMessage;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct PaletteMessageContext {
	pub palette_open: bool,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct PaletteMessageHandler {}

#[message_handler_data]
impl MessageHandler<PaletteMessage, PaletteMessageContext> for PaletteMessageHandler {
	fn process_message(&mut self, message: PaletteMessage, responses: &mut VecDeque<Message>, context: PaletteMessageContext) {
		match message {
			PaletteMessage::UploadPalette => todo!(),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PaletteMessage;)
	}
}
