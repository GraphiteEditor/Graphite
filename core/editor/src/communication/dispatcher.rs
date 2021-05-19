use crate::{frontend::FrontendMessageHandler, message_prelude::*, Callback, EditorError};

pub use crate::document::DocumentMessageHandler;
pub use crate::input::{InputMapper, InputPreprocessor};
pub use crate::tool::ToolMessageHandler;

use crate::global::GlobalMessageHandler;
use std::collections::VecDeque;

pub struct Dispatcher {
	frontend_message_handler: FrontendMessageHandler,
	input_preprocessor: InputPreprocessor,
	input_mapper: InputMapper,
	global_event_handler: GlobalMessageHandler,
	tool_action_handler: ToolMessageHandler,
	document_action_handler: DocumentMessageHandler,
	messages: VecDeque<Message>,
}

impl Dispatcher {
	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Result<(), EditorError> {
		let message = message.into();
		use Message::*;
		if !matches!(
			message,
			Message::InputPreprocessor(_) | Message::InputMapper(_) | Message::Tool(ToolMessage::Rectangle(RectangleMessage::MouseMove))
		) {
			log::trace!("Message: {}", message.to_discriminant().global_name());
		}
		match message {
			NoOp => (),
			Document(message) => self.document_action_handler.process_action(message, (), &mut self.messages),
			Global(message) => self.global_event_handler.process_action(message, (), &mut self.messages),
			Tool(message) => self
				.tool_action_handler
				.process_action(message, (&self.document_action_handler.active_document().document, &self.input_preprocessor), &mut self.messages),
			Frontend(message) => self.frontend_message_handler.process_action(message, (), &mut self.messages),
			InputPreprocessor(message) => self.input_preprocessor.process_action(message, (), &mut self.messages),
			InputMapper(message) => self.input_mapper.process_action(message, &self.input_preprocessor, &mut self.messages),
		}
		if let Some(message) = self.messages.pop_front() {
			self.handle_message(message)?;
		}
		Ok(())
	}

	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher {
			frontend_message_handler: FrontendMessageHandler::new(callback),
			input_preprocessor: InputPreprocessor::default(),
			global_event_handler: GlobalMessageHandler::new(),
			input_mapper: InputMapper::default(),
			document_action_handler: DocumentMessageHandler::default(),
			tool_action_handler: ToolMessageHandler::default(),
			messages: VecDeque::new(),
		}
	}
}
