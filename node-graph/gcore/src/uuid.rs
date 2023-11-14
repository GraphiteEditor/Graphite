use dyn_any::{DynAny, StaticType};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, specta::Type)]
pub struct Uuid(
	#[serde(with = "u64_string")]
	#[specta(type = String)]
	u64,
);

mod u64_string {
	use serde::{self, Deserialize, Deserializer, Serializer};
	use std::str::FromStr;

	// The signature of a serialize_with function must follow the pattern:
	//
	//    fn serialize<S>(&T, S) -> Result<S::Ok, S::Error>
	//    where
	//        S: Serializer
	//
	// although it may also be generic over the input types T.
	pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(&value.to_string())
	}

	// The signature of a deserialize_with function must follow the pattern:
	//
	//    fn deserialize<'de, D>(D) -> Result<T, D::Error>
	//    where
	//        D: Deserializer<'de>
	//
	// although it may also be generic over the output types T.
	pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		u64::from_str(&s).map_err(serde::de::Error::custom)
	}
}

mod uuid_generation {
	use core::cell::Cell;
	use rand_chacha::rand_core::{RngCore, SeedableRng};
	use rand_chacha::ChaCha20Rng;
	use std::sync::Mutex;

	static RNG: Mutex<Option<ChaCha20Rng>> = Mutex::new(None);
	thread_local! {
		pub static UUID_SEED: Cell<Option<u64>> = Cell::new(None);
	}

	pub fn set_uuid_seed(random_seed: u64) {
		UUID_SEED.with(|seed| seed.set(Some(random_seed)))
	}

	pub fn generate_uuid() -> u64 {
		let Ok(mut lock) = RNG.lock() else { panic!("UUID mutex poisoned") };
		if lock.is_none() {
			UUID_SEED.with(|seed| {
				let random_seed = seed.get().unwrap_or(42);
				*lock = Some(ChaCha20Rng::seed_from_u64(random_seed));
			})
		}
		lock.as_mut().map(ChaCha20Rng::next_u64).expect("UUID mutex poisoned")
	}
}

pub use uuid_generation::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ManipulatorGroupId(u64);

impl bezier_rs::Identifier for ManipulatorGroupId {
	fn new() -> Self {
		Self(generate_uuid())
	}
}

impl ManipulatorGroupId {
	pub const ZERO: ManipulatorGroupId = ManipulatorGroupId(0);

	pub fn next_id(&mut self) -> Self {
		let old = self.0;
		self.0 += 1;
		Self(old)
	}
}
