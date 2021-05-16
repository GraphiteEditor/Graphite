pub mod dispatcher;
pub mod document_action_handler;
pub mod events;
pub mod frontend;
pub mod global_action_handler;
pub mod input_manager;
pub mod message;
pub mod tool_action_handler;
pub use dispatcher::*;
pub use events::{DocumentResponse, Event, Key, Response, ToolResponse};
pub use message::{AsMessage, Message, MessageDiscriminant};
pub use proc_macros::MessageImpl;

pub use self::input_manager::InputPreprocessor;
use self::{global_action_handler::GlobalActionHandler, input_manager::InputMapper};

pub use global_action_handler::GlobalAction;

pub type Callback = Box<dyn Fn(Response)>;

pub type ActionList<'a> = &'a [&'static [MessageDiscriminant]];

// TODO: Add Send + Sync requirement
// Use something like rw locks for synchronization
pub trait MessageHandlerData {}

pub trait MessageHandler<A: AsMessage, T: MessageHandlerData> {
	/// Return true if the Action is consumed.
	fn process_action(&mut self, action: A, data: T, responses: &mut Vec<GlobalAction>);
	fn actions(&self) -> ActionList;
}
