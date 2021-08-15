pub mod dispatcher;
pub mod message;
use crate::message_prelude::*;
pub use dispatcher::*;
use rand_chacha::{
	rand_core::{RngCore, SeedableRng},
	ChaCha20Rng,
};
use spin::Mutex;

pub use crate::input::InputPreprocessor;
use std::collections::VecDeque;

pub type ActionList = Vec<Vec<MessageDiscriminant>>;

static RNG: Mutex<Option<ChaCha20Rng>> = Mutex::new(None);

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

pub fn generate_uuid() -> u64 {
	let mut lock = RNG.lock();
	if lock.is_none() {
		*lock = Some(ChaCha20Rng::seed_from_u64(0));
	}
	lock.as_mut().map(ChaCha20Rng::next_u64).unwrap()
}
