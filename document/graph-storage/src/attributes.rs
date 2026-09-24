use crate::TimeStamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Attribute keys. Glob-import (`use crate::attr::*`) at conversion sites.
///
/// `ui::*` keys are namespaced per CRDT design so each value gets its own LWW timestamp. Per-input
/// keys live on `Node.inputs_attributes[i]`; per-network keys live on `Network.attributes`.
pub mod attr;

/// A type-erased attribute value paired with the timestamp at which it was last set. A deleted key
/// stays as a stamped tombstone, so a write older than the deletion is recognised as older whichever
/// order the two land in; readers see a tombstone as absent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Value {
	pub value: serde_json::Value,
	pub timestamp: TimeStamp,
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub deleted: bool,
}

impl Value {
	pub fn new(value: serde_json::Value, timestamp: TimeStamp) -> Self {
		Self { value, timestamp, deleted: false }
	}

	/// The tombstone of a key deleted at `timestamp`.
	pub fn deleted(timestamp: TimeStamp) -> Self {
		Self {
			value: serde_json::Value::Null,
			timestamp,
			deleted: true,
		}
	}
}

/// Attribute maps. Each entity carries an `attributes_timestamp` next to its map: when the map was last
/// written whole, by an addition or a whole-list input write. A key absent from the map is deleted as of
/// that stamp, so a write older than it is dropped, and a tombstone older than it is redundant.
pub type Attributes = BTreeMap<String, Value>;

/// The live entries of an attribute map: every key that is not a tombstone.
pub fn live(attributes: &Attributes) -> impl Iterator<Item = (&String, &Value)> {
	attributes.iter().filter(|(_, value)| !value.deleted)
}

/// Write helpers for `Attributes`.
pub trait AttributesWrite {
	/// Inserts a JSON value under `key`.
	fn set(&mut self, key: &str, value: serde_json::Value, timestamp: TimeStamp);

	/// Serializes `value` and inserts it under `key`.
	fn set_serialized<T: serde::Serialize>(&mut self, key: &str, value: &T, timestamp: TimeStamp) -> Result<(), serde_json::Error> {
		self.set(key, serde_json::to_value(value)?, timestamp);
		Ok(())
	}
	/// Inserts only when `value != default`, so the read side falls back to the same default.
	fn set_if_not_default<T: serde::Serialize + PartialEq>(&mut self, key: &str, value: &T, default: &T, timestamp: TimeStamp) -> Result<(), serde_json::Error> {
		if value != default {
			self.set_serialized(key, value, timestamp)?;
		}
		Ok(())
	}
}

impl AttributesWrite for Attributes {
	fn set(&mut self, key: &str, value: serde_json::Value, timestamp: TimeStamp) {
		self.insert(key.to_string(), Value::new(value, timestamp));
	}
}

/// Typed read helpers for `Attributes`.
pub trait AttributesRead {
	/// Deserializes the value under `key`, or `None` if missing or undecodable.
	fn get_typed<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T>;

	/// Same as `get_typed`, falling back to `default`.
	fn get_or<T: serde::de::DeserializeOwned>(&self, key: &str, default: T) -> T {
		self.get_typed(key).unwrap_or(default)
	}

	/// Same as `get_typed`, falling back to `T::default()`.
	fn get_or_default<T: serde::de::DeserializeOwned + Default>(&self, key: &str) -> T {
		self.get_typed(key).unwrap_or_default()
	}
}

impl AttributesRead for Attributes {
	fn get_typed<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
		self.get(key).filter(|v| !v.deleted).and_then(|v| serde_json::from_value(v.value.clone()).ok())
	}
}
