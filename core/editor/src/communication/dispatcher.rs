use crate::{communication::MessageHandler, EditorError};

pub use super::input_manager::InputPreprocessor;
use super::{document_action_handler::DocumentActionHandler, input_manager::InputMapper, tool_action_handler::ToolActionHandler};

use super::global_action_handler::GlobalActionHandler;
use super::FrontendMessage;
use super::Message;

pub type Callback = Box<dyn Fn(FrontendMessage)>;

pub struct Dispatcher {
	callback: Callback,
	input_preprocessor: InputPreprocessor,
	input_mapper: InputMapper,
	global_event_handler: GlobalActionHandler,
	tool_action_handler: ToolActionHandler,
	document_action_handler: DocumentActionHandler,
	messages: Vec<Message>,
}

impl Dispatcher {
	pub fn handle_message(&mut self, message: Message) -> Result<(), EditorError> {
		self.messages.clear();
		use Message::*;
		match message {
			Document(message) => self.document_action_handler.process_action(message, (), &mut self.messages),
			Global(message) => self.global_event_handler.process_action(message, (), &mut self.messages),
			Tool(message) => self
				.tool_action_handler
				.process_action(message, (&self.document_action_handler.active_document().document, &self.input_preprocessor), &mut self.messages),
			Frontend(message) => Self::dispatch_response(message, &self.callback),
			InputPreprocessor(message) => self.input_preprocessor.process_action(message, (), &mut self.messages),
			InputMapper(message) => self.input_mapper.process_action(message, (), &mut self.messages),
		}
		Ok(())
	}

	pub fn dispatch_response<T: Into<FrontendMessage>>(response: T, callback: &Callback) {
		let response: FrontendMessage = response.into();
		log::trace!("Sending {} Response", response);
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
			messages: Vec::new(),
		}
	}
}
