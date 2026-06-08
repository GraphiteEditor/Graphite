use crate::TimeStamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Attribute keys. Glob-import (`use crate::attr::*`) at conversion sites.
///
/// `ui::*` keys are namespaced per CRDT design so each value gets its own LWW timestamp. Per-input
/// keys live on `Node.inputs_attributes[i]`; per-network keys live on `Network.attributes`.
pub mod attr {
	pub const CALL_ARGUMENT: &str = "call_argument";
	pub const CONTEXT_FEATURES: &str = "context_features";
	pub const IMPORT_TYPE: &str = "import_type";
	pub const VISIBLE: &str = "visible";
	pub const SKIP_DEDUPLICATION: &str = "skip_deduplication";
	pub const REFLECTION_METADATA: &str = "reflection_metadata";
	pub const ORIGINAL_NODE_ID: &str = "original_node_id";
	pub const EXPORTED_NODES_TS: &str = "library::exported_nodes_ts";
	/// Whole-map LWW of a network's `scope_injections` (`key -> (storage NodeId, Type)`), stored as a
	/// serialized blob so its shape can evolve (e.g. dropping the `Type`) without a model change. The
	/// node references use stable storage IDs, resolved back to runtime-local IDs on conversion.
	pub const SCOPE_INJECTIONS: &str = "compute::scope_injections";

	pub const UI_POSITION: &str = "ui::position";
	pub const UI_IS_LAYER: &str = "ui::is_layer";
	pub const UI_DISPLAY_NAME: &str = "ui::display_name";
	pub const UI_LOCKED: &str = "ui::locked";
	pub const UI_PINNED: &str = "ui::pinned";

	pub const UI_INPUT_NAME: &str = "ui::input_name";
	pub const UI_INPUT_DESCRIPTION: &str = "ui::input_description";
	pub const UI_WIDGET_OVERRIDE: &str = "ui::widget_override";
	/// Prefix for `InputPersistentMetadata::input_data` entries. Full key: `ui::input_data::<sub_key>`.
	pub const UI_INPUT_DATA_PREFIX: &str = "ui::input_data::";

	pub const UI_OUTPUT_NAMES: &str = "ui::output_names";
	/// Lives on the *owning* node (the one with `Implementation::Network`), not on the nested network.
	pub const UI_REFERENCE: &str = "ui::reference";

	// Delta-level annotations (on `Delta.attributes`, not the registry). Local + mutable, excluded
	// from the content-addressed `Rev`.
	/// Marks the last delta of a user gesture, so the undo cursor steps per-gesture, not per-delta.
	pub const GESTURE_END: &str = "compute::gesture_end";
}

/// A type-erased attribute value paired with the timestamp at which it was last set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Value {
	pub value: serde_json::Value,
	pub timestamp: TimeStamp,
}

impl Value {
	pub fn new(value: serde_json::Value, timestamp: TimeStamp) -> Self {
		Self { value, timestamp }
	}
}

pub type Attributes = BTreeMap<String, Value>;

/// Write helpers for `Attributes`.
pub trait AttributesExt {
	/// Inserts a JSON value under `key`.
	fn set(&mut self, key: &str, value: serde_json::Value, timestamp: TimeStamp);

	/// Serializes `value` and inserts it under `key`.
	fn set_serialized<T: serde::Serialize>(&mut self, key: &str, value: &T, timestamp: TimeStamp) -> Result<(), serde_json::Error>;

	/// Inserts only when `value != default`, so the read side falls back to the same default.
	fn set_if_not_default<T: serde::Serialize + PartialEq>(&mut self, key: &str, value: &T, default: &T, timestamp: TimeStamp) -> Result<(), serde_json::Error>;
}

impl AttributesExt for Attributes {
	fn set(&mut self, key: &str, value: serde_json::Value, timestamp: TimeStamp) {
		self.insert(key.to_string(), Value { value, timestamp });
	}

	fn set_serialized<T: serde::Serialize>(&mut self, key: &str, value: &T, timestamp: TimeStamp) -> Result<(), serde_json::Error> {
		self.set(key, serde_json::to_value(value)?, timestamp);
		Ok(())
	}

	fn set_if_not_default<T: serde::Serialize + PartialEq>(&mut self, key: &str, value: &T, default: &T, timestamp: TimeStamp) -> Result<(), serde_json::Error> {
		if value != default {
			self.set_serialized(key, value, timestamp)?;
		}
		Ok(())
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
		self.get(key).and_then(|v| serde_json::from_value(v.value.clone()).ok())
	}
}

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
}
