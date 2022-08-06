use crate::dispatcher::Dispatcher;
use crate::messages::prelude::*;

use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use spin::Mutex;
use std::cell::Cell;

static RNG: Mutex<Option<ChaCha20Rng>> = Mutex::new(None);
thread_local! {
	pub static UUID_SEED: Cell<Option<u64>> = Cell::new(None);
}

// TODO: serialize with serde to save the current editor state
pub struct Editor {
	pub dispatcher: Dispatcher,
}

impl Editor {
	/// Construct a new editor instance.
	/// Remember to provide a random seed with `editor::set_uuid_seed(seed)` before any editors can be used.
	pub fn new() -> Self {
		Self { dispatcher: Dispatcher::new() }
	}

	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Vec<FrontendMessage> {
		self.dispatcher.handle_message(message);

		let mut responses = Vec::new();
		std::mem::swap(&mut responses, &mut self.dispatcher.responses);

		responses
	}
}

impl Default for Editor {
	fn default() -> Self {
		Self::new()
	}
}

pub fn set_uuid_seed(random_seed: u64) {
	UUID_SEED.with(|seed| seed.set(Some(random_seed)))
}

pub fn generate_uuid() -> u64 {
	let mut lock = RNG.lock();
	if lock.is_none() {
		UUID_SEED.with(|seed| {
			let random_seed = seed.get().expect("Random seed not set before editor was initialized");
			*lock = Some(ChaCha20Rng::seed_from_u64(random_seed));
		})
	}
	lock.as_mut().map(ChaCha20Rng::next_u64).unwrap()
}

pub fn release_series() -> String {
	format!("Release Series: {}", env!("GRAPHITE_RELEASE_SERIES"))
}

pub fn commit_info() -> String {
	format!("{}\n{}\n{}", commit_timestamp(), commit_hash(), commit_branch())
}

pub fn commit_info_localized(localized_commit_date: &str) -> String {
	format!("{}\n{}\n{}", commit_timestamp_localized(localized_commit_date), commit_hash(), commit_branch())
}

pub fn commit_timestamp() -> String {
	format!("Date: {}", env!("GRAPHITE_GIT_COMMIT_DATE"))
}

pub fn commit_timestamp_localized(localized_commit_date: &str) -> String {
	format!("Date: {}", localized_commit_date)
}

pub fn commit_hash() -> String {
	format!("Hash: {}", &env!("GRAPHITE_GIT_COMMIT_HASH")[..8])
}

pub fn commit_branch() -> String {
	format!("Branch: {}", env!("GRAPHITE_GIT_COMMIT_BRANCH"))
}
