use super::{TimeStamp, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An attribute value and when it was last set. A deleted key stays as a stamped tombstone, which readers see as absent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeValue {
	pub value: Value,
	pub timestamp: TimeStamp,
	// No skipping: the registry is positional on disk.
	#[serde(default)]
	pub deleted: bool,
}

/// Attribute keys and their values, each last-writer-wins on its own timestamp. A key the map lacks is deleted as of
/// its floor, when the map was last written whole.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Attributes {
	pub(crate) entries: BTreeMap<String, AttributeValue>,
	#[serde(default)]
	pub(crate) floor: TimeStamp,
}
