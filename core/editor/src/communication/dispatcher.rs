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
	global_message_handler: GlobalMessageHandler,
	tool_message_handler: ToolMessageHandler,
	document_message_handler: DocumentMessageHandler,
	messages: VecDeque<Message>,
}

impl Dispatcher {
	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Result<(), EditorError> {
		let message = message.into();
		use Message::*;
		if !(matches!(
			message,
			Message::InputPreprocessor(_)
				| Message::InputMapper(_)
				| Message::Document(DocumentMessage::RenderDocument)
				| Message::Frontend(FrontendMessage::UpdateCanvas { .. })
				| Message::Document(DocumentMessage::DispatchOperation { .. })
		) || MessageDiscriminant::from(&message).local_name().ends_with("MouseMove"))
		{
			log::trace!("Message: {}", message.to_discriminant().local_name());
		}
		match message {
			NoOp => (),
			Document(message) => self.document_message_handler.process_action(message, &self.input_preprocessor, &mut self.messages),
			Global(message) => self.global_message_handler.process_action(message, (), &mut self.messages),
			Tool(message) => self
				.tool_message_handler
				.process_action(message, (&self.document_message_handler.active_document().document, &self.input_preprocessor), &mut self.messages),
			Frontend(message) => self.frontend_message_handler.process_action(message, (), &mut self.messages),
			InputPreprocessor(message) => self.input_preprocessor.process_action(message, (), &mut self.messages),
			InputMapper(message) => {
				let actions = self.collect_actions();
				self.input_mapper.process_action(message, (&self.input_preprocessor, actions), &mut self.messages)
			}
		}
		if let Some(message) = self.messages.pop_front() {
			self.handle_message(message)?;
		}
		Ok(())
	}

	pub fn collect_actions(&self) -> ActionList {
		//TODO: reduce the number of heap allocations
		let mut list = Vec::new();
		list.extend(self.frontend_message_handler.actions());
		list.extend(self.input_preprocessor.actions());
		list.extend(self.input_mapper.actions());
		list.extend(self.global_message_handler.actions());
		list.extend(self.tool_message_handler.actions());
		list.extend(self.document_message_handler.actions());
		list
	}

	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher {
			frontend_message_handler: FrontendMessageHandler::new(callback),
			input_preprocessor: InputPreprocessor::default(),
			global_message_handler: GlobalMessageHandler::new(),
			input_mapper: InputMapper::default(),
			document_message_handler: DocumentMessageHandler::default(),
			tool_message_handler: ToolMessageHandler::default(),
			messages: VecDeque::new(),
		}
	}
}
