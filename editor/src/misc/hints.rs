use crate::input::keyboard::{Key, MouseMotion};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintData(pub Vec<HintGroup>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintGroup(pub Vec<HintInfo>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintInfo {
	/// A `KeysGroup` specifies all the keys pressed simultaneously to perform an action (like "Ctrl C" to copy).
	/// Usually at most one is given, but less commonly, multiple can be used to describe additional hotkeys not used simultaneously (like the four different arrow keys to nudge a layer).
	pub key_groups: Vec<KeysGroup>,
	/// `None` means that `key_groups` should be used for both platforms, `Some` is an override for Mac only
	pub key_groups_mac: Option<Vec<KeysGroup>>,
	/// An optional `MouseMotion` that can indicate the mouse action, like which mouse button is used and whether a drag occurs.
	/// No such icon is shown if `None` is given, and it can be combined with `key_groups` if desired.
	pub mouse: Option<MouseMotion>,
	/// The text describing what occurs with this input combination.
	pub label: String,
	/// Draws a prepended "+" symbol which indicates that this is a refinement upon a previous hint in the group.
	pub plus: bool,
}

/// Only `Key`s that exist on a physical keyboard should be used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeysGroup(pub Vec<Key>);
