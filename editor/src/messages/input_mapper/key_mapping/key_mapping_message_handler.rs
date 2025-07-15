use crate::messages::input_mapper::input_mapper_message_handler::InputMapperMessageContext;
use crate::messages::input_mapper::utility_types::input_keyboard::KeysGroup;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct KeyMappingMessageContext<'a> {
	pub input: &'a InputPreprocessorMessageHandler,
	pub actions: ActionList,
}

#[derive(Debug, Default, ExtractField)]
pub struct KeyMappingMessageHandler {
	mapping_handler: InputMapperMessageHandler,
}

#[message_handler_data]
impl MessageHandler<KeyMappingMessage, KeyMappingMessageContext<'_>> for KeyMappingMessageHandler {
	fn process_message(&mut self, message: KeyMappingMessage, responses: &mut VecDeque<Message>, context: KeyMappingMessageContext) {
		let KeyMappingMessageContext { input, actions } = context;

		match message {
			KeyMappingMessage::Lookup(input_message) => self.mapping_handler.process_message(input_message, responses, InputMapperMessageContext { input, actions }),
			KeyMappingMessage::ModifyMapping(new_layout) => self.mapping_handler.set_mapping(new_layout.into()),
		}
	}
	advertise_actions!();
}

impl KeyMappingMessageHandler {
	pub fn action_input_mapping(&self, action_to_find: &MessageDiscriminant) -> Option<KeysGroup> {
		self.mapping_handler.action_input_mapping(action_to_find)
	}
}
