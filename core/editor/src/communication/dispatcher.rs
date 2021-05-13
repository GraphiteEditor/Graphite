
use crate::EditorError;
use document_core::Operation;
pub use events::{DocumentResponse, Event, Key, Response, ToolResponse};

pub use self::input_manager::InputPreprocessor;
use self::{global_action_handler::GlobalActionHandler, input_manager::InputMapper};

pub use global_action_handler::GlobalAction;

pub use actions::Action;

pub type Callback = Box<dyn Fn(Response)>;

pub type ActionList<'a> = &'a [&'static [GlobalAction]];

pub trait ActionHandlerData: Clone + Sized {}

pub trait ActionHandler<A: Action, T: ActionHandlerData> {
	/// Return true if the Action is consumed.
	fn process_action(&mut self, action: A, data: T, responses: &mut Vec<GlobalAction>);
	fn actions(&self) -> ActionList;
}

pub struct Dispatcher {
	callback: Callback,
	input_preprocessor: InputPreprocessor,
	input_mapper: InputMapper,
	global_event_handler: GlobalActionHandler,
	operations: Vec<Operation>,
	responses: Vec<Response>,
}

impl Dispatcher {
	pub fn handle_event(&mut self, event: Event) -> Result<Vec<FrontendResponse> EditorError> {
		log::trace!("{:?}", event);

		self.operations.clear();
		self.responses.clear();
		let events = self.input_preprocessor.handle_user_input(event);
		for event in events {
			let actions = self.input_mapper.translate_event(event, &self.input_preprocessor, self.global_event_handler.actions());
			for action in actions {
				self.handle_action(action);
			}
		}

		Ok(())
	}

	fn handle_action(&mut self, action: GlobalAction) {
		let consumed = self
			.global_event_handler
			.process_action((), &self.input_preprocessor, &action, &mut self.responses, &mut self.operations);

		debug_assert!(self.operations.is_empty());

		self.dispatch_responses();

		if !consumed {
			log::trace!("Unhandled action {:?}", action);
		}
	}

	pub fn dispatch_responses(&mut self) {
		for response in self.responses.drain(..) {
			Self::dispatch_response(response, &self.callback);
		}
	}

	pub fn dispatch_response<T: Into<Response>>(response: T, callback: &Callback) {
		let response: Response = response.into();
		log::trace!("Sending {} Response", response);
		callback(response)
	}

	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher {
			callback,
			input_preprocessor: InputPreprocessor::default(),
			global_event_handler: GlobalActionHandler::new(),
			input_mapper: InputMapper::default(),
			operations: Vec::new(),
			responses: Vec::new(),
		}
	}
}
