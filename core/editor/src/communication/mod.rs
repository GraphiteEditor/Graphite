pub mod dispatcher;
pub mod document_action_handler;
pub mod events;
pub mod frontend;
pub mod global_action_handler;
pub mod input_manager;
pub mod message;
pub mod tool_action_handler;
pub use dispatcher::*;
pub use frontend::FrontendMessage;
use message::prelude::*;

pub use self::input_manager::InputPreprocessor;
use crate::communication::message::{ToDiscriminant, TransitiveChild};

pub type Callback = Box<dyn Fn(FrontendMessage)>;

pub type ActionList = Vec<Vec<MessageDiscriminant>>;

// TODO: Add Send + Sync requirement
// Use something like rw locks for synchronization
pub trait MessageHandlerData {}

pub trait MessageHandler<A: ToDiscriminant, T>
where
	A::Discriminant: AsMessage,
	<A::Discriminant as TransitiveChild>::TopParent: TransitiveChild<Parent = <A::Discriminant as TransitiveChild>::TopParent, TopParent = <A::Discriminant as TransitiveChild>::TopParent> + AsMessage,
{
	/// Return true if the Action is consumed.
	fn process_action(&mut self, action: A, data: T, responses: &mut Vec<Message>);
	fn actions(&self) -> ActionList;
}
