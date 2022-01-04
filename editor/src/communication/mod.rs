pub mod dispatcher;
pub mod message;
use crate::message_prelude::*;
pub use dispatcher::*;
use rand_chacha::{
	rand_core::{RngCore, SeedableRng},
	ChaCha20Rng,
};
use spin::{Mutex, MutexGuard};

pub use crate::input::InputPreprocessor;
use std::{cell::Cell, collections::VecDeque};

pub type ActionList = Vec<Vec<MessageDiscriminant>>;

#[cfg(not(test))]
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

thread_local! {
	pub static UUID_SEED: Cell<Option<u64>> = Cell::new(None);
	#[cfg(test)]
	static LOCAL_RNG: Mutex<Option<ChaCha20Rng>> = Mutex::new(None);
}

pub fn set_uuid_seed(random_seed: u64) {
	UUID_SEED.with(|seed| seed.set(Some(random_seed)));
}

pub fn generate_uuid() -> u64 {
	let init = |mut lock: MutexGuard<Option<ChaCha20Rng>>| {
		if lock.is_none() {
			UUID_SEED.with(|seed| {
				let random_seed = seed.get().expect("random seed not set before editor was initialized");
				*lock = Some(ChaCha20Rng::seed_from_u64(random_seed));
			})
		}
		lock.as_mut().map(ChaCha20Rng::next_u64).unwrap()
	};
	(
		#[cfg(test)]
		LOCAL_RNG.with(|rng| init(rng.lock())),
		#[cfg(not(test))]
		init(RNG.lock()),
	)
		.0
}
