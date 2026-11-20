use crate::{NetworkId, NodeId, PeerId, RegistryDelta, Rev, TimeStamp};
use serde::{Deserialize, Serialize};

impl std::fmt::Display for NodeId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl std::fmt::Display for NetworkId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl Rev {
	/// Wrap a raw value, or `None` if it is zero.
	pub fn new(value: u128) -> Option<Self> {
		std::num::NonZeroU128::new(value).map(Self)
	}

	pub fn get(self) -> u128 {
		self.0.get()
	}
}

impl std::fmt::Display for Rev {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// Root network ID. The renderable graph lives in `networks[&ROOT_NETWORK]`.
pub const ROOT_NETWORK: NetworkId = NetworkId(0);

/// Upper bound on a network's export slot count, guarding `SetExport` against a malicious or corrupted
/// slot index forcing an unbounded `exports` allocation.
pub(crate) const MAX_EXPORT_SLOTS: usize = 1 << 16;
/// A per-slot write can bring a slot past the end of the list into being, so it is capped the same way.
pub(crate) const MAX_INPUT_SLOTS: usize = 1 << 16;

impl TimeStamp {
	/// Pre-edit origin. Used by initial `from_runtime` conversion before any edits have happened.
	pub const ORIGIN: Self = TimeStamp { counter: 0, peer: PeerId(0) };
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct LamportClock {
	pub(crate) counter: u64,
	peer: PeerId,
}

impl LamportClock {
	pub fn new(peer: PeerId) -> Self {
		Self { counter: 0, peer }
	}

	/// Mints a fresh local timestamp.
	pub fn tick(&mut self) -> TimeStamp {
		self.counter += 1;
		TimeStamp {
			counter: self.counter,
			peer: self.peer,
		}
	}

	/// Advances past an incoming op so future local ticks are causally later.
	pub fn observe(&mut self, incoming: TimeStamp) {
		self.counter = self.counter.max(incoming.counter);
	}
}

/// Hash the identity-bearing fields of a `Delta` with blake3 and truncate to 128 bits.
///
/// A [`RegistryDelta::Merge`] is addressed by its sorted parent set alone (author and timestamp
/// excluded), so two peers merging the same tips mint the identical `Rev` and dedup. Every other
/// delta hashes `(parent, author, timestamp, kind)`.
pub(crate) fn compute_rev(parent: Option<Rev>, author: PeerId, timestamp: TimeStamp, delta_type: &RegistryDelta) -> Rev {
	let mut hasher = blake3::Hasher::new();
	let bytes = match delta_type {
		RegistryDelta::Merge { extra_parents } => {
			let mut parents: Vec<Rev> = parent.into_iter().chain(extra_parents.iter().copied()).collect();
			parents.sort_unstable();
			parents.dedup();
			postcard::to_stdvec(&("merge", parents)).expect("Merge identity fields must serialize")
		}
		_ => postcard::to_stdvec(&(parent, author, timestamp, delta_type)).expect("Delta identity fields must serialize"),
	};
	hasher.update(&bytes);
	let digest = hasher.finalize();
	let mut truncated = [0u8; 16];
	truncated.copy_from_slice(&digest.as_bytes()[..16]);
	// A 128-bit blake3 truncation is zero with probability 2^-128 (never in practice); map it to 1 so
	// the non-zero invariant is total rather than relying on a panic that can't realistically fire.
	Rev::new(u128::from_le_bytes(truncated)).unwrap_or(Rev(std::num::NonZeroU128::MIN))
}
