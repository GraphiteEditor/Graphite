pub mod dispatcher;
pub mod message;
pub mod message_handler;

pub use crate::communication::dispatcher::*;
pub use crate::input::InputPreprocessorMessageHandler;

use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use spin::Mutex;
use std::cell::Cell;

static RNG: Mutex<Option<ChaCha20Rng>> = Mutex::new(None);

thread_local! {
	pub static UUID_SEED: Cell<Option<u64>> = Cell::new(None);
}

pub fn set_uuid_seed(random_seed: u64) {
	UUID_SEED.with(|seed| seed.set(Some(random_seed)))
}

pub fn generate_uuid() -> u64 {
	let mut lock = RNG.lock();
	if lock.is_none() {
		UUID_SEED.with(|seed| {
			let random_seed = seed.get().expect("random seed not set before editor was initialized");
			*lock = Some(ChaCha20Rng::seed_from_u64(random_seed));
		})
	}
	lock.as_mut().map(ChaCha20Rng::next_u64).unwrap()
}
