use crate::{TimeStamp, Value, ValueError, from_value, to_value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

pub type Attributes = BTreeMap<String, AttributeValue>;

/// Write helpers for `Attributes`.
pub trait AttributesWrite {
	/// Inserts `value` under `key`.
	fn set(&mut self, key: &str, value: Value, timestamp: TimeStamp);

	/// Serializes `value` and inserts it under `key`.
	fn set_serialized<T: serde::Serialize>(&mut self, key: &str, value: &T, timestamp: TimeStamp) -> Result<(), ValueError> {
		self.set(key, to_value(value)?, timestamp);
		Ok(())
	}
	/// Inserts only when `value != default`, so the read side falls back to the same default.
	fn set_if_not_default<T: serde::Serialize + PartialEq>(&mut self, key: &str, value: &T, default: &T, timestamp: TimeStamp) -> Result<(), ValueError> {
		if value != default {
			self.set_serialized(key, value, timestamp)?;
		}
		Ok(())
	}
}

impl AttributesWrite for Attributes {
	fn set(&mut self, key: &str, value: Value, timestamp: TimeStamp) {
		self.insert(key.to_string(), AttributeValue { value, timestamp });
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
		self.get(key).and_then(|v| from_value(&v.value).ok())
	}
}
