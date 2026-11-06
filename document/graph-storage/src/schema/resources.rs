use super::{PeerId, TimeStamp, Value};
use graphene_resource::{ResourceHash, ResourceId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Ordering key for an entry in a resource's source chain: fractional `priority`, with `peer` as
/// the tiebreak so concurrent insertions at the same priority converge deterministically.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceKey {
	pub priority: Priority,
	pub peer: PeerId,
}

/// One entry in a resource's source chain. The `source` body is type-erased (`Value`)
/// so the on-disk `DataSource` shape can evolve through migrations without the storage layer
/// committing to a Rust enum; `timestamp` drives LWW on re-setting this same entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceValue {
	pub source: Value,
	pub timestamp: TimeStamp,
	/// A removed source stays as a stamped tombstone, which readers see as absent.
	#[serde(default)]
	pub deleted: bool,
}

/// A single content-addressable resource: an ordered, conflict-mergeable chain of fallback sources
/// plus the resolved content hash. The source chain is an add-wins ordered set (concurrent
/// additions all survive); the hash is last-writer-wins (concurrent resolves of the same logical
/// resource agree by construction, since the hash is content-derived).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ResourceEntry {
	/// The latest addition or write; see [`Node::presence`](crate::Node).
	pub(crate) presence: TimeStamp,
	/// Fallback chain kept sorted by `SourceKey`, so iteration yields highest-priority first.
	pub sources: Vec<(SourceKey, SourceValue)>,
	/// When `sources` was last written whole, by an addition; a key the chain lacks is deleted as of then.
	pub sources_timestamp: TimeStamp,
	pub hash: Option<ResourceHash>,
	pub hash_timestamp: TimeStamp,
}

impl<'de> Deserialize<'de> for ResourceEntry {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		// The `binary_search`-based accessors require `sources` sorted by `SourceKey` with unique keys.
		// On-disk data (older writers, hand edits) can't be trusted to preserve either, so re-sort and
		// collapse any duplicate keys, keeping the higher-timestamp value (LWW).
		#[derive(Deserialize)]
		struct Raw {
			#[serde(default)]
			presence: TimeStamp,
			sources: Vec<(SourceKey, SourceValue)>,
			#[serde(default)]
			sources_timestamp: TimeStamp,
			hash: Option<ResourceHash>,
			hash_timestamp: TimeStamp,
		}

		let Raw {
			presence,
			mut sources,
			sources_timestamp,
			hash,
			hash_timestamp,
		} = Raw::deserialize(deserializer)?;
		sources.sort_by_key(|(key, _)| *key);
		sources.dedup_by(|(later_key, later_value), (kept_key, kept_value)| {
			// `dedup_by` keeps the first of each run; sorting is stable, so resolve duplicates by LWW.
			if later_key != kept_key {
				return false;
			}
			if later_value.timestamp > kept_value.timestamp {
				*kept_value = later_value.clone();
			}
			true
		});

		Ok(Self {
			presence,
			sources,
			sources_timestamp,
			hash,
			hash_timestamp,
		})
	}
}

/// All resources referenced by the document, keyed by stable per-document [`ResourceId`]. Replicates
/// through the normal CmRDT path; bytes live in content-addressed storage keyed by [`ResourceHash`].
pub type ResourceStore = HashMap<ResourceId, ResourceEntry>;

/// Fractional priority for ordering a resource's source chain. New sources are inserted by picking
/// a value strictly between two neighbors, so concurrent insertions elsewhere never collide; an
/// exact tie between two peers inserting at the same gap is broken by `PeerId` in [`SourceKey`].
/// `f64` precision is ample for the short fallback chains resources carry in practice.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "f64")]
pub struct Priority(f64);

impl Priority {
	/// Rejects non-finite input. The field is private and deserialization routes through here, so a
	/// `Priority` is always finite, keeping its `Ord`/`Hash`/`Eq` agreement sound.
	pub fn new(value: f64) -> Result<Self, NonFinitePriority> {
		if value.is_finite() { Ok(Self(value)) } else { Err(NonFinitePriority(value)) }
	}

	pub fn value(self) -> f64 {
		self.0
	}
}

impl TryFrom<f64> for Priority {
	type Error = NonFinitePriority;
	fn try_from(value: f64) -> Result<Self, Self::Error> {
		Self::new(value)
	}
}

/// A [`Priority`] was constructed from a `NaN` or infinite value.
#[derive(Debug, thiserror::Error)]
#[error("priority must be finite, got {0}")]
pub struct NonFinitePriority(pub f64);

// `total_cmp` drives `Ord`, `Hash`, and `Eq` together so `Priority` is a sound `BTree`/`Hash` key:
// a derived `PartialEq` would disagree with this ordering on `-0.0` and `NaN`.
impl PartialEq for Priority {
	fn eq(&self, other: &Self) -> bool {
		self.cmp(other) == std::cmp::Ordering::Equal
	}
}

impl Eq for Priority {}

impl Ord for Priority {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.0.total_cmp(&other.0)
	}
}

impl PartialOrd for Priority {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl std::hash::Hash for Priority {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.0.to_bits().hash(state);
	}
}
