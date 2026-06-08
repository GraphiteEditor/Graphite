use crate::{PeerId, Priority, TimeStamp};
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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResourceEntry {
	/// Fallback chain kept sorted by `SourceKey`, so iteration yields highest-priority first.
	pub sources: Vec<(SourceKey, SourceValue)>,
	pub hash: Option<ResourceHash>,
	pub hash_timestamp: TimeStamp,
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

	/// True if the chain already carries a `DataSource::Embedded` source.
	pub fn has_embedded_source(&self) -> bool {
		let embedded = serde_json::to_value(graphene_resource::DataSource::Embedded).expect("DataSource::Embedded serializes");
		self.sources.iter().any(|(_, value)| value.source == embedded)
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
