use crate::{AttributeDelta, TimeStamp, Value, ValueError, from_value, to_value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, btree_map};

/// Attribute keys. Glob-import (`use crate::attr::*`) at conversion sites.
///
/// `ui::*` keys are namespaced per CRDT design so each value gets its own LWW timestamp. Per-input
/// keys live on `Node.inputs_attributes[i]`; per-network keys live on `Network.attributes`.
pub mod attr;

/// An attribute value and when it was last set. A deleted key stays as a stamped tombstone, which readers see as absent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeValue {
	pub value: Value,
	pub timestamp: TimeStamp,
	// No skipping: the registry is positional on disk.
	#[serde(default)]
	pub deleted: bool,
}

impl AttributeValue {
	pub fn new(value: Value, timestamp: TimeStamp) -> Self {
		Self { value, timestamp, deleted: false }
	}

	/// The tombstone of a key deleted at `timestamp`.
	pub fn deleted(timestamp: TimeStamp) -> Self {
		Self {
			value: Value::None,
			timestamp,
			deleted: true,
		}
	}
}

/// Attribute keys and their values, each last-writer-wins on its own timestamp. A key the map lacks is deleted as of
/// its floor, when the map was last written whole.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Attributes {
	entries: BTreeMap<String, AttributeValue>,
	#[serde(default)]
	floor: TimeStamp,
}

impl Attributes {
	pub fn new() -> Self {
		Self::default()
	}

	/// An empty map written whole at `floor`, which deletes every key older than it.
	pub fn empty_at(floor: TimeStamp) -> Self {
		Self { entries: BTreeMap::new(), floor }
	}

	/// When the map was last written whole.
	pub fn floor(&self) -> TimeStamp {
		self.floor
	}

	/// Carries `floor` with the entries, so they land as the map they were taken from rather than as a new write.
	pub fn set_floor(&mut self, floor: TimeStamp) {
		self.floor = floor;
	}

	pub fn get(&self, key: &str) -> Option<&AttributeValue> {
		self.entries.get(key)
	}

	pub fn contains_key(&self, key: &str) -> bool {
		self.entries.contains_key(key)
	}

	/// Every entry, tombstones included.
	pub fn iter(&self) -> btree_map::Iter<'_, String, AttributeValue> {
		self.entries.iter()
	}

	pub fn keys(&self) -> btree_map::Keys<'_, String, AttributeValue> {
		self.entries.keys()
	}

	/// The live entries: every key that is not a tombstone.
	pub fn live(&self) -> impl Iterator<Item = (&String, &AttributeValue)> {
		self.entries.iter().filter(|(_, value)| !value.deleted)
	}

	pub fn len(&self) -> usize {
		self.entries.len()
	}

	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	pub fn insert(&mut self, key: String, value: AttributeValue) -> Option<AttributeValue> {
		self.entries.insert(key, value)
	}

	pub fn remove(&mut self, key: &str) -> Option<AttributeValue> {
		self.entries.remove(key)
	}

	/// Lands a single-key write if it is newer than the key's value, or than the floor for a key the map lacks. A
	/// deletion leaves a tombstone.
	pub(crate) fn apply_delta(&mut self, delta: AttributeDelta, timestamp: TimeStamp) {
		let AttributeDelta { key, value } = delta;
		let decided = self.get(&key).map_or(self.floor, |existing| existing.timestamp);
		if timestamp <= decided {
			return;
		}
		let entry = match value {
			Some(value) => AttributeValue::new(value, timestamp),
			None => AttributeValue::deleted(timestamp),
		};
		self.insert(key, entry);
	}

	/// Inserts `value` under `key`.
	pub fn set(&mut self, key: &str, value: Value, timestamp: TimeStamp) {
		self.insert(key.to_string(), AttributeValue::new(value, timestamp));
	}

	/// Serializes `value` and inserts it under `key`.
	pub fn set_serialized<T: serde::Serialize>(&mut self, key: &str, value: &T, timestamp: TimeStamp) -> Result<(), ValueError> {
		self.set(key, to_value(value)?, timestamp);
		Ok(())
	}

	/// Inserts only when `value != default`, so the read side falls back to the same default.
	pub fn set_if_not_default<T: serde::Serialize + PartialEq>(&mut self, key: &str, value: &T, default: &T, timestamp: TimeStamp) -> Result<(), ValueError> {
		if value != default {
			self.set_serialized(key, value, timestamp)?;
		}
		Ok(())
	}

	/// Deserializes the value under `key`, or `None` if missing, deleted or undecodable.
	pub fn get_typed<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
		self.get(key).filter(|v| !v.deleted).and_then(|v| from_value(&v.value).ok())
	}

	/// Same as `get_typed`, falling back to `default`.
	pub fn get_or<T: serde::de::DeserializeOwned>(&self, key: &str, default: T) -> T {
		self.get_typed(key).unwrap_or(default)
	}

	/// Same as `get_typed`, falling back to `T::default()`.
	pub fn get_or_default<T: serde::de::DeserializeOwned + Default>(&self, key: &str) -> T {
		self.get_typed(key).unwrap_or_default()
	}

	/// Writes the map whole at `timestamp`: the floor deletes every other key without a tombstone per key.
	pub(crate) fn stamp(&mut self, timestamp: TimeStamp) {
		self.entries.retain(|_, value| !value.deleted);
		self.entries.values_mut().for_each(|value| value.timestamp = timestamp);
		self.floor = timestamp;
	}

	/// Stamps what arrives unstamped: the whole map if it carries no floor, else only the entries at the origin.
	pub(crate) fn stamp_unstamped(&mut self, timestamp: TimeStamp) {
		if self.floor == TimeStamp::ORIGIN {
			return self.stamp(timestamp);
		}
		self.entries
			.values_mut()
			.filter(|value| value.timestamp == TimeStamp::ORIGIN)
			.for_each(|value| value.timestamp = timestamp);
	}

	/// Folds `other` in key by key: the newer entry wins, and a key only one map holds dies if the other map's floor is
	/// newer, as that map was written whole after it.
	pub(crate) fn merge(&mut self, other: Attributes) {
		self.entries.retain(|_, value| value.timestamp >= other.floor);
		for (key, value) in other.entries {
			match self.entries.entry(key) {
				btree_map::Entry::Occupied(entry) if value.timestamp > entry.get().timestamp => *entry.into_mut() = value,
				btree_map::Entry::Vacant(entry) if value.timestamp >= self.floor => {
					entry.insert(value);
				}
				_ => {}
			}
		}
		self.floor = self.floor.max(other.floor);
	}

	/// The newest stamp the map carries, its floor included.
	pub(crate) fn newest(&self) -> TimeStamp {
		self.entries.values().map(|value| value.timestamp).fold(self.floor, TimeStamp::max)
	}
}

impl FromIterator<(String, AttributeValue)> for Attributes {
	fn from_iter<I: IntoIterator<Item = (String, AttributeValue)>>(iter: I) -> Self {
		Self {
			entries: iter.into_iter().collect(),
			floor: TimeStamp::ORIGIN,
		}
	}
}

impl Extend<(String, AttributeValue)> for Attributes {
	fn extend<I: IntoIterator<Item = (String, AttributeValue)>>(&mut self, iter: I) {
		self.entries.extend(iter);
	}
}

impl IntoIterator for Attributes {
	type Item = (String, AttributeValue);
	type IntoIter = btree_map::IntoIter<String, AttributeValue>;
	fn into_iter(self) -> Self::IntoIter {
		self.entries.into_iter()
	}
}

impl<'a> IntoIterator for &'a Attributes {
	type Item = (&'a String, &'a AttributeValue);
	type IntoIter = btree_map::Iter<'a, String, AttributeValue>;
	fn into_iter(self) -> Self::IntoIter {
		self.entries.iter()
	}
}
