use crate::messages::input_mapper::default_mapping::default_mapping;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeyStates, NUMBER_OF_KEYS};
use crate::messages::portfolio::document::utility_types::misc::KeyboardPlatformLayout;
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
	pub fn match_input_message(&self, message: InputMapperMessage, keyboard_state: &KeyStates, actions: ActionList, keyboard_platform: KeyboardPlatformLayout) -> Option<Message> {
		let list = match message {
			InputMapperMessage::KeyDown(key) => &self.key_down[key as usize],
			InputMapperMessage::KeyUp(key) => &self.key_up[key as usize],
			InputMapperMessage::DoubleClick => &self.double_click,
			InputMapperMessage::WheelScroll => &self.wheel_scroll,
			InputMapperMessage::PointerMove => &self.pointer_move,
		};
		list.match_mapping(keyboard_state, actions, keyboard_platform)
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
	pub fn match_mapping(&self, keyboard_state: &KeyStates, actions: ActionList, keyboard_platform: KeyboardPlatformLayout) -> Option<Message> {
		for entry in self.0.iter() {
			// Skip this entry if it is platform-specific, and for a layout that does not match the user's keyboard platform layout
			if let Some(entry_platform_layout) = entry.platform_layout {
				if entry_platform_layout != keyboard_platform {
					continue;
				}
			}

			// Find which currently pressed keys are also the modifiers in this hotkey entry, then compare those against the required modifiers to see if there are zero missing
			let pressed_modifiers = *keyboard_state & entry.modifiers;
			let all_modifiers_without_pressed_modifiers = entry.modifiers ^ pressed_modifiers;
			let all_required_modifiers_pressed = all_modifiers_without_pressed_modifiers.is_empty();
			// Skip this entry if any of the required modifiers are missing
			if !all_required_modifiers_pressed {
				continue;
			}

			if actions.iter().flatten().any(|action| entry.action.to_discriminant() == *action) {
				return Some(entry.action.clone());
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
	/// The keyboard platform layout which this mapping is exclusive to, or `None` if it's platform-agnostic
	pub platform_layout: Option<KeyboardPlatformLayout>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionKeys {
	Action(MessageDiscriminant),
	#[serde(rename = "keys")]
	Keys(Vec<Key>),
}

impl ActionKeys {
	pub fn to_keys(&mut self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<Vec<Key>>) {
		match self {
			ActionKeys::Action(action) => {
				if let Some(keys) = action_input_mapping(action).get_mut(0) {
					let mut taken_keys = Vec::new();
					std::mem::swap(keys, &mut taken_keys);

					*self = ActionKeys::Keys(taken_keys);
				} else {
					*self = ActionKeys::Keys(Vec::new());
				}
			}
			ActionKeys::Keys(keys) => {
				log::warn!("Calling `.to_keys()` on a `ActionKeys::Keys` is a mistake/bug. Keys are: {:?}.", keys);
			}
		}
	}
}

pub fn keys_text_shortcut(keys: &[Key], keyboard_platform: KeyboardPlatformLayout) -> String {
	const JOINER_MARK: &str = "+";

	let mut joined = keys
		.iter()
		.map(|key| {
			let key_string = key.to_string();

			if keyboard_platform == KeyboardPlatformLayout::Mac {
				match key_string.as_str() {
					"Command" => "⌘".to_string(),
					"Control" => "⌃".to_string(),
					"Alt" => "⌥".to_string(),
					"Shift" => "⇧".to_string(),
					_ => key_string + JOINER_MARK,
				}
			} else {
				key_string + JOINER_MARK
			}
		})
		.collect::<String>();

	// Truncate to cut the joining character off the end if it's present
	if joined.ends_with(JOINER_MARK) {
		joined.truncate(joined.len() - JOINER_MARK.len());
	}

	joined
}
