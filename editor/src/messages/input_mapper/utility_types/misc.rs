use super::input_keyboard::{all_required_modifiers_pressed, KeysGroup};
use crate::messages::input_mapper::default_mapping::default_mapping;
use crate::messages::input_mapper::utility_types::input_keyboard::{KeyStates, NUMBER_OF_KEYS};
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Mapping {
	pub key_up: [KeyMappingEntries; NUMBER_OF_KEYS],
	pub key_down: [KeyMappingEntries; NUMBER_OF_KEYS],
	pub double_click: KeyMappingEntries,
	pub wheel_scroll: KeyMappingEntries,
	pub pointer_move: KeyMappingEntries,
}

impl Mapping {
	pub fn match_input_message(&self, message: InputMapperMessage, keyboard_state: &KeyStates, actions: ActionList) -> Option<Message> {
		let list = match message {
			InputMapperMessage::KeyDown(key) => &self.key_down[key as usize],
			InputMapperMessage::KeyUp(key) => &self.key_up[key as usize],
			InputMapperMessage::DoubleClick => &self.double_click,
			InputMapperMessage::WheelScroll => &self.wheel_scroll,
			InputMapperMessage::PointerMove => &self.pointer_move,
		};
		list.match_mapping(keyboard_state, actions)
	}
}

impl Default for Mapping {
	fn default() -> Self {
		default_mapping()
	}
}

#[derive(Debug, Clone)]
pub struct KeyMappingEntries(pub Vec<MappingEntry>);

impl KeyMappingEntries {
	pub fn match_mapping(&self, keyboard_state: &KeyStates, actions: ActionList) -> Option<Message> {
		for mapping in self.0.iter() {
			// Skip this entry if any of the required modifiers are missing
			if all_required_modifiers_pressed(keyboard_state, &mapping.modifiers) {
				// Search for the action in the list of available actions to see if it's currently available to activate
				let matching_action_found = actions.iter().flatten().any(|action| mapping.action.to_discriminant() == *action);
				if matching_action_found {
					return Some(mapping.action.clone());
				}
			}
		}
		None
	}

	pub fn push(&mut self, entry: MappingEntry) {
		self.0.push(entry)
	}

	pub const fn new() -> Self {
		Self(Vec::new())
	}

	pub fn key_array() -> [Self; NUMBER_OF_KEYS] {
		const DEFAULT: KeyMappingEntries = KeyMappingEntries::new();
		[DEFAULT; NUMBER_OF_KEYS]
	}
}

#[derive(PartialEq, Clone, Debug)]
pub struct MappingEntry {
	/// Serves two purposes:
	/// - This is the message that gets dispatched when the hotkey is matched
	/// - This message's discriminant is the action; it must be a currently active action to be considered as a shortcut
	pub action: Message,
	/// The user input event from an input device which this input mapping matches on
	pub input: InputMapperMessage,
	/// Any additional keys that must be also pressed for this input mapping to match
	pub modifiers: KeyStates,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ActionKeys {
	Action(MessageDiscriminant),
	#[serde(rename = "keys")]
	Keys(KeysGroup),
}

impl ActionKeys {
	pub fn to_keys(&mut self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<KeysGroup>) {
		match self {
			ActionKeys::Action(action) => {
				if let Some(keys) = action_input_mapping(action).get_mut(0) {
					let mut taken_keys = KeysGroup::default();
					std::mem::swap(keys, &mut taken_keys);

					*self = ActionKeys::Keys(taken_keys);
				} else {
					*self = ActionKeys::Keys(KeysGroup::default());
				}
			}
			ActionKeys::Keys(keys) => {
				warn!("Calling `.to_keys()` on a `ActionKeys::Keys` is a mistake/bug. Keys are: {:?}.", keys);
			}
		}
	}
}
