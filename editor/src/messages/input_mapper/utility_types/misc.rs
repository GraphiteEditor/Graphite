use super::input_keyboard::{all_required_modifiers_pressed, KeysGroup, LayoutKeysGroup};
use crate::messages::input_mapper::key_mapping::MappingVariant;
use crate::messages::input_mapper::utility_types::input_keyboard::{KeyStates, NUMBER_OF_KEYS};
use crate::messages::input_mapper::utility_types::input_mouse::NUMBER_OF_MOUSE_BUTTONS;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Mapping {
	pub key_up: [KeyMappingEntries; NUMBER_OF_KEYS],
	pub key_down: [KeyMappingEntries; NUMBER_OF_KEYS],
	pub key_up_no_repeat: [KeyMappingEntries; NUMBER_OF_KEYS],
	pub key_down_no_repeat: [KeyMappingEntries; NUMBER_OF_KEYS],
	pub double_click: [KeyMappingEntries; NUMBER_OF_MOUSE_BUTTONS],
	pub wheel_scroll: KeyMappingEntries,
	pub pointer_move: KeyMappingEntries,
}

impl Default for Mapping {
	fn default() -> Self {
		MappingVariant::Default.into()
	}
}

impl Mapping {
	pub fn match_input_message(&self, message: InputMapperMessage, keyboard_state: &KeyStates, actions: ActionList) -> Option<Message> {
		let list = self.associated_entries(&message);
		list.match_mapping(keyboard_state, actions)
	}

	pub fn remove(&mut self, target_entry: &MappingEntry) {
		let list = self.associated_entries_mut(&target_entry.input);
		list.remove(target_entry);
	}

	pub fn add(&mut self, new_entry: MappingEntry) {
		let list = self.associated_entries_mut(&new_entry.input);
		list.push(new_entry);
	}

	fn associated_entries(&self, message: &InputMapperMessage) -> &KeyMappingEntries {
		match message {
			InputMapperMessage::KeyDown(key) => &self.key_down[*key as usize],
			InputMapperMessage::KeyUp(key) => &self.key_up[*key as usize],
			InputMapperMessage::KeyDownNoRepeat(key) => &self.key_down_no_repeat[*key as usize],
			InputMapperMessage::KeyUpNoRepeat(key) => &self.key_up_no_repeat[*key as usize],
			InputMapperMessage::DoubleClick(key) => &self.double_click[*key as usize],
			InputMapperMessage::WheelScroll => &self.wheel_scroll,
			InputMapperMessage::PointerMove => &self.pointer_move,
		}
	}

	fn associated_entries_mut(&mut self, message: &InputMapperMessage) -> &mut KeyMappingEntries {
		match message {
			InputMapperMessage::KeyDown(key) => &mut self.key_down[*key as usize],
			InputMapperMessage::KeyUp(key) => &mut self.key_up[*key as usize],
			InputMapperMessage::KeyDownNoRepeat(key) => &mut self.key_down_no_repeat[*key as usize],
			InputMapperMessage::KeyUpNoRepeat(key) => &mut self.key_up_no_repeat[*key as usize],
			InputMapperMessage::DoubleClick(key) => &mut self.double_click[*key as usize],
			InputMapperMessage::WheelScroll => &mut self.wheel_scroll,
			InputMapperMessage::PointerMove => &mut self.pointer_move,
		}
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
		self.0.push(entry);
	}

	pub fn remove(&mut self, target_entry: &MappingEntry) {
		self.0.retain(|entry| entry != target_entry);
	}

	pub const fn new() -> Self {
		Self(Vec::new())
	}

	pub fn key_array() -> [Self; NUMBER_OF_KEYS] {
		const DEFAULT: KeyMappingEntries = KeyMappingEntries::new();
		[DEFAULT; NUMBER_OF_KEYS]
	}

	pub fn mouse_buttons_arrays() -> [Self; NUMBER_OF_MOUSE_BUTTONS] {
		const DEFAULT: KeyMappingEntries = KeyMappingEntries::new();
		[DEFAULT; NUMBER_OF_MOUSE_BUTTONS]
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, specta::Type)]
pub enum ActionKeys {
	Action(MessageDiscriminant),
	#[serde(rename = "keys")]
	Keys(LayoutKeysGroup),
}

impl ActionKeys {
	pub fn to_keys(&mut self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<KeysGroup>) -> String {
		match self {
			Self::Action(action) => {
				if let Some(keys) = action_input_mapping(action).get_mut(0) {
					let mut taken_keys = KeysGroup::default();
					std::mem::swap(keys, &mut taken_keys);
					let description = taken_keys.to_string();
					*self = Self::Keys(taken_keys.into());
					description
				} else {
					*self = Self::Keys(KeysGroup::default().into());
					String::new()
				}
			}
			Self::Keys(keys) => {
				warn!("Calling `.to_keys()` on a `ActionKeys::Keys` is a mistake/bug. Keys are: {keys:?}.");
				String::new()
			}
		}
	}
}
