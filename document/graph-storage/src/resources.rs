use crate::{PeerId, Priority, ResourceEntry, SourceKey, SourceValue, TimeStamp, Value, from_value, to_value};
use graphene_resource::ResourceHash;

impl SourceValue {
	/// The tombstone of a source removed at `timestamp`.
	pub fn deleted(timestamp: TimeStamp) -> Self {
		Self {
			source: Value::None,
			timestamp,
			deleted: true,
		}
	}
}

impl ResourceEntry {
	/// A resource backed by a single `DataSource::Embedded` fallback resolved to `hash`. Both the
	/// source entry and the resolved hash carry `timestamp` so later LWW writes order against it.
	/// The bytes themselves are persisted separately by the caller's byte store.
	pub fn embedded(hash: ResourceHash, peer: PeerId, timestamp: TimeStamp) -> Self {
		let embedded = to_value(&graphene_resource::DataSource::Embedded).expect("DataSource::Embedded serializes");
		let priority = Priority::new(0.).expect("0. is finite");
		let sources = vec![(
			SourceKey { priority, peer },
			SourceValue {
				source: embedded,
				timestamp,
				deleted: false,
			},
		)];

		Self {
			presence: timestamp,
			sources,
			sources_timestamp: timestamp,
			hash: Some(hash),
			hash_timestamp: timestamp,
		}
	}

	/// The live source stored under `key`, if any; a tombstone is absent.
	pub fn source(&self, key: &SourceKey) -> Option<&SourceValue> {
		self.find(key).ok().map(|index| &self.sources[index].1).filter(|value| !value.deleted)
	}

	/// The live sources in precedence order, tombstones skipped.
	pub(crate) fn live_sources(&self) -> impl Iterator<Item = (&SourceKey, &SourceValue)> {
		self.sources.iter().filter(|(_, value)| !value.deleted).map(|(key, value)| (key, value))
	}

	fn find(&self, key: &SourceKey) -> Result<usize, usize> {
		self.sources.binary_search_by(|(candidate, _)| candidate.cmp(key))
	}

	/// The stamp that decides a write to `key`: its entry's, tombstone included, or the chain's floor.
	fn deciding_timestamp(&self, key: &SourceKey) -> TimeStamp {
		self.find(key).map_or(self.sources_timestamp, |index| self.sources[index].1.timestamp)
	}

	fn put(&mut self, key: SourceKey, value: SourceValue) {
		match self.find(&key) {
			Ok(index) => self.sources[index].1 = value,
			Err(index) => self.sources.insert(index, (key, value)),
		}
	}

	/// Writes the chain whole at `timestamp`: the floor deletes every other key without a tombstone per key.
	pub(crate) fn stamp_sources(&mut self, timestamp: TimeStamp) {
		self.sources.retain(|(_, value)| !value.deleted);
		self.sources.iter_mut().for_each(|(_, value)| value.timestamp = timestamp);
		self.sources_timestamp = timestamp;
	}

	/// Folds another chain in key by key, as attribute maps merge.
	pub(crate) fn merge_sources(&mut self, other: Vec<(SourceKey, SourceValue)>, other_floor: TimeStamp) {
		self.sources.retain(|(_, value)| value.timestamp >= other_floor);
		for (key, value) in other {
			match self.find(&key) {
				Ok(index) if value.timestamp > self.sources[index].1.timestamp => self.sources[index].1 = value,
				Err(index) if value.timestamp >= self.sources_timestamp => self.sources.insert(index, (key, value)),
				_ => {}
			}
		}
		self.sources_timestamp = self.sources_timestamp.max(other_floor);
	}

	/// Insert or LWW-overwrite the entry at `key`; a re-set wins only with a strictly newer stamp.
	pub fn set_source(&mut self, key: SourceKey, value: SourceValue) {
		if value.timestamp > self.deciding_timestamp(&key) {
			self.put(key, value);
		}
	}

	/// Like [`set_source`](Self::set_source) but assigns unconditionally, for building an entry.
	pub fn force_set_source(&mut self, key: SourceKey, value: SourceValue) {
		self.put(key, value);
	}

	/// Remove the source at `key` if `timestamp` is newer than what decides it, leaving a tombstone. Returns whether it landed.
	pub fn remove_source(&mut self, key: &SourceKey, timestamp: TimeStamp) -> bool {
		if timestamp <= self.deciding_timestamp(key) {
			return false;
		}
		self.put(*key, SourceValue::deleted(timestamp));
		true
	}

	/// True if the chain already carries a `DataSource::Embedded` source. Decodes each source body into
	/// `DataSource` so a shape change in the serialized form can't slip an embedded source past detection.
	pub fn has_embedded_source(&self) -> bool {
		self.live_sources()
			.any(|(_, value)| matches!(from_value::<graphene_resource::DataSource>(&value.source), Ok(graphene_resource::DataSource::Embedded)))
	}

	/// A `SourceKey` ordered strictly ahead of every current source, so an inserted entry becomes the
	/// highest-precedence fallback.
	pub fn highest_precedence_key(&self, peer: PeerId) -> SourceKey {
		let min_priority = self.live_sources().next().map(|(key, _)| key.priority.value()).unwrap_or(0.);
		SourceKey {
			priority: Priority::new(min_priority - 1.).expect("finite priority minus one is finite"),
			peer,
		}
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
	/// than silently producing an unsound map key. Postcard (the storage format) can carry a
	/// non-finite `f64`, unlike JSON, so this guards the real round-trip path.
	#[test]
	fn priority_deserialize_validates_finiteness() {
		let finite = postcard::to_stdvec(&3.5_f64).unwrap();
		assert!(postcard::from_bytes::<Priority>(&finite).is_ok());

		let non_finite = postcard::to_stdvec(&f64::INFINITY).unwrap();
		assert!(postcard::from_bytes::<Priority>(&non_finite).is_err(), "a non-finite priority on disk must be rejected");
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
					source: Value::Float(priority),
					timestamp: TimeStamp::ORIGIN,
					deleted: false,
				},
			)
		};

		// Build a deliberately unsorted chain as a literal (bypassing the sorted-insert API), then round-trip it through `Value`.
		let unsorted = ResourceEntry {
			presence: TimeStamp::ORIGIN,
			sources_timestamp: TimeStamp::ORIGIN,
			sources: vec![source(2.), source(0.), source(1.)],
			hash: None,
			hash_timestamp: TimeStamp::ORIGIN,
		};

		let entry: ResourceEntry = from_value(&to_value(&unsorted).expect("serialize")).expect("deserialize");
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
					source: Value::Str(body.into()),
					timestamp: TimeStamp { counter, peer: PeerId(1) },
					deleted: false,
				},
			)
		};

		let with_duplicates = ResourceEntry {
			presence: TimeStamp::ORIGIN,
			sources_timestamp: TimeStamp::ORIGIN,
			sources: vec![entry(5, "newer"), entry(1, "older")],
			hash: None,
			hash_timestamp: TimeStamp::ORIGIN,
		};

		let resource: ResourceEntry = from_value(&to_value(&with_duplicates).expect("serialize")).expect("deserialize");
		assert_eq!(resource.sources.len(), 1, "duplicate keys must collapse to one entry");
		assert_eq!(resource.sources[0].1.source, Value::Str("newer".into()), "the higher-timestamp value must win");
	}
}
