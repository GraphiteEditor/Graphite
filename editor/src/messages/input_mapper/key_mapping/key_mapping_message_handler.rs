use crate::messages::input_mapper::utility_types::input_keyboard::KeysGroup;
use crate::messages::prelude::*;

#[derive(Debug, Default)]
pub struct KeyMappingMessageHandler {
	mapping_handler: InputMapperMessageHandler,
}

impl MessageHandler<KeyMappingMessage, (&InputPreprocessorMessageHandler, ActionList)> for KeyMappingMessageHandler {
	fn process_message(&mut self, message: KeyMappingMessage, responses: &mut VecDeque<Message>, data: (&InputPreprocessorMessageHandler, ActionList)) {
		match message {
			KeyMappingMessage::Lookup(input) => self.mapping_handler.process_message(input, responses, data),
			KeyMappingMessage::ModifyMapping(new_layout) => self.mapping_handler.set_mapping(new_layout.into()),
		}
	}
	advertise_actions!();
}

impl KeyMappingMessageHandler {
	pub fn action_input_mapping(&self, action_to_find: &MessageDiscriminant) -> Vec<KeysGroup> {
		self.mapping_handler.action_input_mapping(action_to_find)
	}
}
