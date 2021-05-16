use crate::EditorError;
use document_core::Operation;

use super::input_manager::InputMapper;
pub use super::input_manager::InputPreprocessor;

use super::global_action_handler::GlobalActionHandler;
use super::FrontendMessage;
use super::Message;

pub type Callback = Box<dyn Fn(FrontendMessage)>;

pub struct Dispatcher {
	callback: Callback,
	input_preprocessor: InputPreprocessor,
	input_mapper: InputMapper,
	global_event_handler: GlobalActionHandler,
	messages: Vec<Message>,
}

impl Dispatcher {
	pub fn handle_message(&mut self, message: Message) -> Result<(), EditorError> {
		self.messages.clear();
		/*let events = self.input_preprocessor.handle_user_input(event);
		for event in events {
			let actions = self.input_mapper.translate_event(event, &self.input_preprocessor, self.global_event_handler.actions());
			for action in actions {
				self.handle_action(action);
			}
		}
		*/

		Ok(())
	}

	/*fn handle_action(&mut self, action: GlobalAction) {
		let consumed = self
			.global_event_handler
			.process_action((), &self.input_preprocessor, &action, &mut self.responses, &mut self.operations);

		debug_assert!(self.operations.is_empty());

		self.dispatch_responses();

		if !consumed {
			log::trace!("Unhandled action {:?}", action);
		}
	}*/

	/*pub fn dispatch_responses(&mut self) {
		for response in self.responses.drain(..) {
			Self::dispatch_response(response, &self.callback);
		}
	}*/

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
			messages: Vec::new(),
		}
	}
}
