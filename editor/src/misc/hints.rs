use crate::input::keyboard::{Key, MouseMotion};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintData(pub Vec<HintGroup>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintGroup(pub Vec<HintInfo>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintInfo {
	pub key_groups: Vec<KeysGroup>,
	pub mouse: Option<MouseMotion>,
	pub label: String,
	/// Prepend the "+" symbol indicating that this is a refinement upon a previous entry in the group.
	pub plus: bool,
}

/// Only `Key`s that exist on a physical keyboard should be used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeysGroup(pub Vec<Key>);
