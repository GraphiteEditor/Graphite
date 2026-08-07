use super::utility_types::input_keyboard::KeysGroup;
use super::utility_types::misc::Mapping;
use crate::application::Editor;
use crate::messages::input_mapper::utility_types::input_keyboard::{self, Key};
use crate::messages::input_mapper::utility_types::misc::MappingEntry;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct InputMapperMessageContext<'a> {
	pub input: &'a InputPreprocessorMessageHandler,
	pub actions: ActionList,
}

#[derive(Debug, Default, ExtractField)]
pub struct InputMapperMessageHandler {
	mapping: Mapping,
}

#[message_handler_data]
impl MessageHandler<InputMapperMessage, InputMapperMessageContext<'_>> for InputMapperMessageHandler {
	fn process_message(&mut self, message: InputMapperMessage, responses: &mut VecDeque<Message>, context: InputMapperMessageContext) {
		let InputMapperMessageContext { input, actions } = context;

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

	pub fn action_input_mapping(&self, action_to_find: &MessageDiscriminant) -> Option<KeysGroup> {
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

		// Get the `Key` for this platform's accelerator key
		let platform_accel_key = if Editor::environment().is_mac() { Key::Command } else { Key::Control };

		let entry_to_key = |entry: &MappingEntry| {
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
			use InputMapperMessage as IMM;
			match entry.input {
				IMM::KeyDown(key) | IMM::KeyUp(key) | IMM::KeyDownNoRepeat(key) | IMM::KeyUpNoRepeat(key) => keys.push(key),
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
		};

		// If a canonical key combination is found, return it
		if let Some(canonical) = found_actions.clone().find(|entry| entry.canonical).map(entry_to_key) {
			return Some(canonical);
		}

		// Find the key combinations for all keymaps matching the desired action
		assert!(std::mem::size_of::<usize>() >= std::mem::size_of::<Key>());
		let mut key_sequences = found_actions.map(entry_to_key).collect::<Vec<_>>();

		// Return the shortest key sequence, if any
		key_sequences.sort_by_key(|keys| keys.0.len());
		key_sequences.first().cloned()
	}
}
