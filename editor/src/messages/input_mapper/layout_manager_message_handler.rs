use super::{utility_types::input_keyboard::KeysGroup, InputMapperMessageHandler};

use crate::messages::prelude::*;

#[derive(Debug, Default)]
pub struct LayoutManagerMessageHandler {
	mapping_handler: InputMapperMessageHandler,
}

impl MessageHandler<LayoutManagerMessage, (&InputPreprocessorMessageHandler, ActionList)> for LayoutManagerMessageHandler {
	fn process_message(&mut self, message: LayoutManagerMessage, responses: &mut VecDeque<Message>, data: (&InputPreprocessorMessageHandler, ActionList)) {
		match message {
			LayoutManagerMessage::Lookup(input) => self.mapping_handler.process_message(input, responses, data),
			_ => todo!(),
		}
	}
	advertise_actions!();
}

impl LayoutManagerMessageHandler {
	pub fn action_input_mapping(&self, action_to_find: &MessageDiscriminant) -> Vec<KeysGroup> {
		self.mapping_handler.action_input_mapping(action_to_find)
	}
}
