use super::utility_types::input_keyboard::KeysGroup;
use super::utility_types::misc::Mapping;
use crate::messages::input_mapper::utility_types::input_keyboard::{self, Key};
use crate::messages::portfolio::utility_types::KeyboardPlatformLayout;
use crate::messages::prelude::*;

use std::fmt::Write;

#[derive(Debug, Default)]
pub struct InputMapperMessageHandler {
	mapping: Mapping,
}

impl MessageHandler<InputMapperMessage, (&InputPreprocessorMessageHandler, ActionList)> for InputMapperMessageHandler {
	fn process_message(&mut self, message: InputMapperMessage, responses: &mut VecDeque<Message>, (input, actions): (&InputPreprocessorMessageHandler, ActionList)) {
		if let Some(message) = self.mapping.match_input_message(message, &input.keyboard, actions) {
			responses.add(message);
		}
	}
	advertise_actions!();
}

impl InputMapperMessageHandler {
	pub fn set_mapping(&mut self, mapping: Mapping) {
		self.mapping = mapping;
	}

	pub fn hints(&self, actions: ActionList) -> String {
		let mut output = String::new();
		let mut actions = actions
			.into_iter()
			.flatten()
			.filter(|a| !matches!(*a, MessageDiscriminant::Tool(ToolMessageDiscriminant::ActivateTool) | MessageDiscriminant::Debug(_)));
		self.mapping
			.key_down
			.iter()
			.enumerate()
			.filter_map(|(i, m)| {
				let ma = m.0.iter().find_map(|m| actions.find_map(|a| (a == m.action.to_discriminant()).then(|| m.action.to_discriminant())));

				ma.map(|a| ((i as u8).try_into().unwrap(), a))
			})
			.for_each(|(k, a): (Key, _)| {
				let _ = write!(output, "{}: {}, ", k.to_discriminant().local_name(), a.local_name().split('.').last().unwrap());
			});
		output.replace("Key", "")
	}

	pub fn action_input_mapping(&self, action_to_find: &MessageDiscriminant) -> Vec<KeysGroup> {
		let all_key_mapping_entries = std::iter::empty()
			.chain(self.mapping.key_up.iter())
			.chain(self.mapping.key_down.iter())
			.chain(self.mapping.key_up_no_repeat.iter())
			.chain(self.mapping.key_down_no_repeat.iter())
			.chain(self.mapping.double_click.iter())
			.chain(std::iter::once(&self.mapping.wheel_scroll))
			.chain(std::iter::once(&self.mapping.pointer_move));
		let all_mapping_entries = all_key_mapping_entries.flat_map(|entry| entry.0.iter());

		// Filter for the desired message
		let found_actions = all_mapping_entries.filter(|entry| entry.action.to_discriminant() == *action_to_find);

		let keyboard_layout = || GLOBAL_PLATFORM.get().copied().unwrap_or_default().as_keyboard_platform_layout();
		let platform_accel_key = match keyboard_layout() {
			KeyboardPlatformLayout::Standard => Key::Control,
			KeyboardPlatformLayout::Mac => Key::Command,
		};

		// Find the key combinations for all keymaps matching the desired action
		assert!(std::mem::size_of::<usize>() >= std::mem::size_of::<Key>());
		found_actions
			.map(|entry| {
				// Get the modifier keys for the entry (and convert them to Key)
				let mut keys = entry
					.modifiers
					.iter()
					.map(|i| {
						// TODO: Use a safe solution eventually
						assert!(
							i < input_keyboard::NUMBER_OF_KEYS,
							"Attempting to convert a Key with enum index {i}, which is larger than the number of Key enums",
						);
						(i as u8).try_into().unwrap()
					})
					.collect::<Vec<_>>();

				// Append the key button for the entry
				match entry.input {
					InputMapperMessage::KeyDown(key) => keys.push(key),
					InputMapperMessage::KeyUp(key) => keys.push(key),
					InputMapperMessage::KeyDownNoRepeat(key) => keys.push(key),
					InputMapperMessage::KeyUpNoRepeat(key) => keys.push(key),
					_ => (),
				}

				keys.sort_by(|&a, &b| {
					// Order according to platform guidelines mentioned at https://ux.stackexchange.com/questions/58185/normative-ordering-for-modifier-key-combinations
					const ORDER: [Key; 4] = [Key::Control, Key::Alt, Key::Shift, Key::Command];

					// Treat the `Accel` virtual key as the platform's accel key for sorting comparison purposes
					let a = if a == Key::Accel { platform_accel_key } else { a };
					let b = if b == Key::Accel { platform_accel_key } else { b };

					// Find where the keys are in the order, or put them at the end if they're not found
					let a = ORDER.iter().position(|&key| key == a).unwrap_or(ORDER.len());
					let b = ORDER.iter().position(|&key| key == b).unwrap_or(ORDER.len());

					// Compare the positions of both keys
					a.cmp(&b)
				});

				KeysGroup(keys)
			})
			.collect::<Vec<_>>()
	}
}
