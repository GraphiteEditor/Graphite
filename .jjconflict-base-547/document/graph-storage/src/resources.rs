use crate::{PeerId, TimeStamp};
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

/// One entry in a resource's source chain. The `source` body is type-erased (`serde_json::Value`)
/// so the on-disk `DataSource` shape can evolve through migrations without the storage layer
/// committing to a Rust enum; `timestamp` drives LWW on re-setting this same entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceValue {
	pub source: serde_json::Value,
	pub timestamp: TimeStamp,
}

/// A single content-addressable resource: an ordered, conflict-mergeable chain of fallback sources
/// plus the resolved content hash. The source chain is an add-wins ordered set (concurrent
/// additions all survive); the hash is last-writer-wins (concurrent resolves of the same logical
/// resource agree by construction, since the hash is content-derived).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ResourceEntry {
	/// Fallback chain kept sorted by `SourceKey`, so iteration yields highest-priority first.
	pub sources: Vec<(SourceKey, SourceValue)>,
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
			sources: Vec<(SourceKey, SourceValue)>,
			hash: Option<ResourceHash>,
			hash_timestamp: TimeStamp,
		}

		let Raw { mut sources, hash, hash_timestamp } = Raw::deserialize(deserializer)?;
		sources.sort_by_key(|(a, _)| *a);
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

		Ok(Self { sources, hash, hash_timestamp })
	}
}

impl ResourceEntry {
	/// A resource backed by a single `DataSource::Embedded` fallback resolved to `hash`. Both the
	/// source entry and the resolved hash carry `timestamp` so later LWW writes order against it.
	/// The bytes themselves are persisted separately by the caller's byte store.
	pub fn embedded(hash: ResourceHash, peer: PeerId, timestamp: TimeStamp) -> Self {
		let embedded = serde_json::to_value(graphene_resource::DataSource::Embedded).expect("DataSource::Embedded serializes");
		let priority = Priority::new(0.).expect("0. is finite");
		let sources = vec![(SourceKey { priority, peer }, SourceValue { source: embedded, timestamp })];

		Self {
			sources,
			hash: Some(hash),
			hash_timestamp: timestamp,
		}
	}

	/// The source body and timestamp stored under `key`, if any.
	pub fn source(&self, key: &SourceKey) -> Option<&SourceValue> {
		self.sources.binary_search_by(|(candidate, _)| candidate.cmp(key)).ok().map(|index| &self.sources[index].1)
	}

	/// Insert or LWW-overwrite the entry at `key`. A re-set at an existing key wins only if `value`'s
	/// timestamp is strictly newer; a fresh key is inserted in sorted position.
	pub fn set_source(&mut self, key: SourceKey, value: SourceValue) {
		match self.sources.binary_search_by(|(candidate, _)| candidate.cmp(&key)) {
			Ok(index) => {
				if value.timestamp > self.sources[index].1.timestamp {
					self.sources[index].1 = value;
				}
			}
			Err(index) => self.sources.insert(index, (key, value)),
		}
	}

	/// Like [`set_source`](Self::set_source) but assigns unconditionally (silent-zone rewind), where the
	/// precomputed reverse/forward value is authoritative even if its timestamp ties what it replaces.
	pub fn force_set_source(&mut self, key: SourceKey, value: SourceValue) {
		match self.sources.binary_search_by(|(candidate, _)| candidate.cmp(&key)) {
			Ok(index) => self.sources[index].1 = value,
			Err(index) => self.sources.insert(index, (key, value)),
		}
	}

	/// Remove the entry at `key` if its timestamp is strictly older than `timestamp` (LWW). Returns
	/// whether anything was removed.
	pub fn remove_source(&mut self, key: &SourceKey, timestamp: TimeStamp) -> bool {
		match self.sources.binary_search_by(|(candidate, _)| candidate.cmp(key)) {
			Ok(index) if timestamp > self.sources[index].1.timestamp => {
				self.sources.remove(index);
				true
			}
			_ => false,
		}
	}

	/// Like [`remove_source`](Self::remove_source) but removes unconditionally (silent-zone rewind).
	pub fn force_remove_source(&mut self, key: &SourceKey) -> bool {
		match self.sources.binary_search_by(|(candidate, _)| candidate.cmp(key)) {
			Ok(index) => {
				self.sources.remove(index);
				true
			}
			_ => false,
		}
	}

	/// True if the chain already carries a `DataSource::Embedded` source. Decodes each source body into
	/// `DataSource` so a shape change in the serialized form can't slip an embedded source past detection.
	pub fn has_embedded_source(&self) -> bool {
		self.sources.iter().any(|(_, value)| {
			matches!(
				serde_json::from_value::<graphene_resource::DataSource>(value.source.clone()),
				Ok(graphene_resource::DataSource::Embedded)
			)
		})
	}

	/// A `SourceKey` ordered strictly ahead of every current source, so an inserted entry becomes the
	/// highest-precedence fallback.
	pub fn highest_precedence_key(&self, peer: PeerId) -> SourceKey {
		let min_priority = self.sources.first().map(|(key, _)| key.priority.value()).unwrap_or(0.);
		SourceKey {
			priority: Priority::new(min_priority - 1.).expect("finite priority minus one is finite"),
			peer,
		}
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn priority_rejects_non_finite() {
		assert!(Priority::new(f64::NAN).is_err());
		assert!(Priority::new(f64::INFINITY).is_err());
		assert!(Priority::new(-1.5).is_ok(), "negative finite priorities are valid");
	}

	/// Deserialization routes through `Priority::new`, so a non-finite value on disk is rejected rather
	/// than silently producing an unsound map key. MessagePack (the storage format) can carry a
	/// non-finite `f64`, unlike JSON, so this guards the real round-trip path.
	#[test]
	fn priority_deserialize_validates_finiteness() {
		let finite = rmp_serde::to_vec(&3.5_f64).unwrap();
		assert!(rmp_serde::from_slice::<Priority>(&finite).is_ok());

		let non_finite = rmp_serde::to_vec(&f64::INFINITY).unwrap();
		assert!(rmp_serde::from_slice::<Priority>(&non_finite).is_err(), "a non-finite priority on disk must be rejected");
	}

	/// `ResourceEntry`'s accessors rely on `sources` being sorted by `SourceKey`. Deserializing an
	/// out-of-order chain (older writer, hand-edited file) must restore the invariant rather than leave
	/// `binary_search` to silently misbehave.
	#[test]
	fn deserialize_sorts_sources() {
		let source = |priority: f64| {
			(
				SourceKey {
					priority: Priority::new(priority).expect("finite"),
					peer: PeerId(1),
				},
				SourceValue {
					source: serde_json::json!(priority),
					timestamp: TimeStamp::ORIGIN,
				},
			)
		};

		// Serialize a deliberately unsorted chain through the raw shape, then deserialize as `ResourceEntry`.
		let unsorted = serde_json::json!({
			"sources": [source(2.), source(0.), source(1.)],
			"hash": null,
			"hash_timestamp": TimeStamp::ORIGIN,
		});

		let entry: ResourceEntry = serde_json::from_value(unsorted).expect("deserialize");
		let priorities: Vec<f64> = entry.sources.iter().map(|(key, _)| key.priority.value()).collect();
		assert_eq!(priorities, vec![0., 1., 2.], "sources must be sorted by SourceKey after deserialization");
	}

	/// Duplicate keys on disk violate the `binary_search` uniqueness invariant. Deserialization must
	/// collapse them, keeping the higher-timestamp value (LWW).
	#[test]
	fn deserialize_dedups_sources_by_lww() {
		let key = SourceKey {
			priority: Priority::new(1.).expect("finite"),
			peer: PeerId(1),
		};
		let entry = |counter: u64, body: &str| {
			(
				key,
				SourceValue {
					source: serde_json::json!(body),
					timestamp: TimeStamp { counter, peer: PeerId(1) },
				},
			)
		};

		let with_duplicates = serde_json::json!({
			"sources": [entry(5, "newer"), entry(1, "older")],
			"hash": null,
			"hash_timestamp": TimeStamp::ORIGIN,
		});

		let resource: ResourceEntry = serde_json::from_value(with_duplicates).expect("deserialize");
		assert_eq!(resource.sources.len(), 1, "duplicate keys must collapse to one entry");
		assert_eq!(resource.sources[0].1.source, serde_json::json!("newer"), "the higher-timestamp value must win");
	}
}
