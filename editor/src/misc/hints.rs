use serde::{Deserialize, Serialize};

use crate::input::keyboard::{Key, MouseMotion};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintData(pub Vec<HintGroup>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintGroup(pub Vec<HintInfo>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintInfo {
	pub key_groups: Vec<KeysGroup>,
	pub mouse: Option<MouseMotion>,
	pub label: String,
	pub plus: bool, // Prepend the "+" symbol indicating that this is a refinement upon a previous entry in the group
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeysGroup(pub Vec<Key>); // Only use `Key`s that exist on a physical keyboard
