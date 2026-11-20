use crate::application::Environment;
use crate::messages::input_mapper::input_mapper_message_handler::InputMapperMessageContext;
use crate::messages::input_mapper::key_mapping::MappingVariant;
use crate::messages::input_mapper::utility_types::keyboard::KeysGroup;
use crate::messages::input_mapper::utility_types::misc::Mapping;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct KeyMappingMessageContext<'a> {
	pub environment: Environment,
	pub input: &'a InputPreprocessorMessageHandler,
	pub actions: ActionList,
}

#[derive(Debug, ExtractField)]
pub struct KeyMappingMessageHandler {
	mapping_handler: InputMapperMessageHandler,
}

#[message_handler_data]
impl MessageHandler<KeyMappingMessage, KeyMappingMessageContext<'_>> for KeyMappingMessageHandler {
	fn process_message(&mut self, message: KeyMappingMessage, responses: &mut VecDeque<Message>, context: KeyMappingMessageContext) {
		let KeyMappingMessageContext { environment, input, actions } = context;

		match message {
			// Sub-messages
			KeyMappingMessage::Lookup(input_message) => self.mapping_handler.process_message(input_message, responses, InputMapperMessageContext { input, actions }),

			// Messages
			KeyMappingMessage::ModifyMapping { mapping } => self.mapping_handler.set_mapping(Mapping::new(mapping, environment)),
		}
	}
	advertise_actions!();
}

impl KeyMappingMessageHandler {
	pub fn new(environment: Environment) -> Self {
		Self {
			mapping_handler: InputMapperMessageHandler::new(Mapping::new(MappingVariant::Default, environment)),
		}
	}

	pub fn action_input_mapping(&self, action_to_find: &MessageDiscriminant, environment: Environment) -> Option<KeysGroup> {
		self.mapping_handler.action_input_mapping(action_to_find, environment)
	}
}
