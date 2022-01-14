pub use crate::communication::dispatcher::*;
pub use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;

use std::collections::VecDeque;

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
	fn process_action(&mut self, action: A, data: T, responses: &mut VecDeque<Message>);

	fn actions(&self) -> ActionList;
}
