use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub mod dispatcher;
pub mod message;
use crate::message_prelude::*;
pub use dispatcher::*;

pub use crate::input::InputPreprocessor;
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

pub fn generate_hash<'a>(messages: impl IntoIterator<Item = &'a Message>, ipp: &InputPreprocessor, document_hash: u64) -> u64 {
	let mut s = DefaultHasher::new();
	document_hash.hash(&mut s);
	ipp.hash(&mut s);
	for message in messages {
		message.hash(&mut s);
	}
	s.finish()
}
