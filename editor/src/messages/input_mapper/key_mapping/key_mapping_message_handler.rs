use crate::messages::input_mapper::input_mapper_message_handler::InputMapperMessageData;
use crate::messages::input_mapper::utility_types::input_keyboard::KeysGroup;
use crate::messages::prelude::*;

pub struct KeyMappingMessageData<'a> {
	pub input: &'a InputPreprocessorMessageHandler,
	pub actions: ActionList,
}

#[derive(Debug, Default)]
pub struct KeyMappingMessageHandler {
	mapping_handler: InputMapperMessageHandler,
}

impl MessageHandler<KeyMappingMessage, KeyMappingMessageData<'_>> for KeyMappingMessageHandler {
	fn process_message(&mut self, message: KeyMappingMessage, responses: &mut VecDeque<Message>, data: KeyMappingMessageData) {
		let KeyMappingMessageData { input, actions } = data;

		match message {
			KeyMappingMessage::Lookup(input_message) => self.mapping_handler.process_message(input_message, responses, InputMapperMessageData { input, actions }),
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
