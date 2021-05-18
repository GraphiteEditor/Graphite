use crate::{
	communication::{
		message::{AsMessage, ToDiscriminant, ToolMessage},
		MessageHandler,
	},
	tools::rectangle::RectangleMessage,
	EditorError,
};

pub use super::input_manager::InputPreprocessor;
use super::{document_action_handler::DocumentActionHandler, input_manager::InputMapper, tool_action_handler::ToolActionHandler};

use super::global_action_handler::GlobalActionHandler;
use super::FrontendMessage;
use super::Message;
use std::collections::VecDeque;

pub type Callback = Box<dyn Fn(FrontendMessage)>;

pub struct Dispatcher {
	callback: Callback,
	input_preprocessor: InputPreprocessor,
	input_mapper: InputMapper,
	global_event_handler: GlobalActionHandler,
	tool_action_handler: ToolActionHandler,
	document_action_handler: DocumentActionHandler,
	messages: VecDeque<Message>,
}

impl Dispatcher {
	pub fn handle_message(&mut self, message: Message) -> Result<(), EditorError> {
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
			Frontend(message) => Self::dispatch_response(message, &self.callback),
			InputPreprocessor(message) => self.input_preprocessor.process_action(message, (), &mut self.messages),
			InputMapper(message) => self.input_mapper.process_action(message, &self.input_preprocessor, &mut self.messages),
		}
		if let Some(message) = self.messages.pop_front() {
			self.handle_message(message)?;
		}
		Ok(())
	}

	pub fn dispatch_response<T: Into<FrontendMessage>>(response: T, callback: &Callback) {
		let response: FrontendMessage = response.into();
		log::trace!("Sending {} Response", response.to_discriminant().global_name());
		callback(response)
	}

	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher {
			callback,
			input_preprocessor: InputPreprocessor::default(),
			global_event_handler: GlobalActionHandler::new(),
			input_mapper: InputMapper::default(),
			document_action_handler: DocumentActionHandler::default(),
			tool_action_handler: ToolActionHandler::default(),
			messages: VecDeque::new(),
		}
	}
}
