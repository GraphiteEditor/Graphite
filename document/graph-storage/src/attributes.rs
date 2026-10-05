use crate::{AttributeDelta, TimeStamp, Value, ValueError, from_value, to_value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, btree_map};

/// Attribute keys. Glob-import (`use crate::attr::*`) at conversion sites.
///
/// `ui::*` keys are namespaced per CRDT design so each value gets its own LWW timestamp. Per-input
/// keys live on `Node.inputs_attributes[i]`; per-network keys live on `Network.attributes`.
pub mod attr;

/// A type-erased attribute value paired with the timestamp at which it was last set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeValue {
	pub value: Value,
	pub timestamp: TimeStamp,
}

impl AttributeValue {
	pub fn new(value: Value, timestamp: TimeStamp) -> Self {
		Self { value, timestamp }
	}
}

/// Attribute keys and their values, each last-writer-wins on its own timestamp.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Attributes {
	entries: BTreeMap<String, AttributeValue>,
}

impl Attributes {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn get(&self, key: &str) -> Option<&AttributeValue> {
		self.entries.get(key)
	}

	pub fn contains_key(&self, key: &str) -> bool {
		self.entries.contains_key(key)
	}

	pub fn iter(&self) -> btree_map::Iter<'_, String, AttributeValue> {
		self.entries.iter()
	}

	pub fn keys(&self) -> btree_map::Keys<'_, String, AttributeValue> {
		self.entries.keys()
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

	/// Applies `delta` written at `timestamp`: it lands if newer than the key's value, or always when forced.
	pub(crate) fn apply_delta(&mut self, delta: AttributeDelta, timestamp: TimeStamp, force: bool) {
		let AttributeDelta { key, value } = delta;
		if !(force || self.get(&key).is_none_or(|existing| timestamp > existing.timestamp)) {
			return;
		}
		match value {
			Some(value) => {
				self.insert(key, AttributeValue::new(value, timestamp));
			}
			None => {
				self.remove(&key);
			}
		}
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

	/// Deserializes the value under `key`, or `None` if missing or undecodable.
	pub fn get_typed<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
		self.get(key).and_then(|v| from_value(&v.value).ok())
	}

	/// Same as `get_typed`, falling back to `default`.
	pub fn get_or<T: serde::de::DeserializeOwned>(&self, key: &str, default: T) -> T {
		self.get_typed(key).unwrap_or(default)
	}

	/// Same as `get_typed`, falling back to `T::default()`.
	pub fn get_or_default<T: serde::de::DeserializeOwned + Default>(&self, key: &str) -> T {
		self.get_typed(key).unwrap_or_default()
	}
}

impl FromIterator<(String, AttributeValue)> for Attributes {
	fn from_iter<I: IntoIterator<Item = (String, AttributeValue)>>(iter: I) -> Self {
		Self { entries: iter.into_iter().collect() }
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
