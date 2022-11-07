use super::utility_types::input_keyboard::KeysGroup;
use super::utility_types::misc::Mapping;
use crate::messages::input_mapper::utility_types::input_keyboard::{self, Key};
use crate::messages::prelude::*;

use std::fmt::Write;

#[derive(Debug, Default)]
pub struct InputMapperMessageHandler {
	mapping: Mapping,
}

impl MessageHandler<InputMapperMessage, (&InputPreprocessorMessageHandler, ActionList)> for InputMapperMessageHandler {
	fn process_message(&mut self, message: InputMapperMessage, (input, actions): (&InputPreprocessorMessageHandler, ActionList), responses: &mut VecDeque<Message>) {
		if let Some(message) = self.mapping.match_input_message(message, &input.keyboard, actions) {
			responses.push_back(message);
		}
	}
	advertise_actions!();
}

impl InputMapperMessageHandler {
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

				ma.map(|a| unsafe { (std::mem::transmute_copy::<usize, Key>(&i), a) })
			})
			.for_each(|(k, a)| {
				let _ = write!(output, "{}: {}, ", k.to_discriminant().local_name(), a.local_name().split('.').last().unwrap());
			});
		output.replace("Key", "")
	}

	pub fn action_input_mapping(&self, action_to_find: &MessageDiscriminant) -> Vec<KeysGroup> {
		let key_up = self.mapping.key_up.iter();
		let key_down = self.mapping.key_down.iter();
		let double_click = std::iter::once(&self.mapping.double_click);
		let wheel_scroll = std::iter::once(&self.mapping.wheel_scroll);
		let pointer_move = std::iter::once(&self.mapping.pointer_move);

		let all_key_mapping_entries = key_up.chain(key_down).chain(double_click).chain(wheel_scroll).chain(pointer_move);
		let all_mapping_entries = all_key_mapping_entries.flat_map(|entry| entry.0.iter());

		// Filter for the desired message
		let found_actions = all_mapping_entries.filter(|entry| entry.action.to_discriminant() == *action_to_find);

		// Find the key combinations for all keymaps matching the desired action
		assert!(std::mem::size_of::<usize>() >= std::mem::size_of::<Key>());
		found_actions
			.map(|entry| {
				let mut keys = entry
					.modifiers
					.iter()
					.map(|i| {
						// TODO: Use a safe solution eventually
						assert!(
							i < input_keyboard::NUMBER_OF_KEYS,
							"Attempting to convert a Key with enum index {}, which is larger than the number of Key enums",
							i
						);
						unsafe { std::mem::transmute_copy::<usize, Key>(&i) }
					})
					.collect::<Vec<_>>();

				if let InputMapperMessage::KeyDown(key) = entry.input {
					keys.push(key);
				}

				keys.sort_by(|a, b| {
					// Order according to platform guidelines mentioned at https://ux.stackexchange.com/questions/58185/normative-ordering-for-modifier-key-combinations
					const ORDER: [Key; 4] = [Key::Control, Key::Alt, Key::Shift, Key::Command];

					match (ORDER.contains(a), ORDER.contains(b)) {
						(true, true) => ORDER.iter().position(|key| key == a).unwrap().cmp(&ORDER.iter().position(|key| key == b).unwrap()),
						(true, false) => std::cmp::Ordering::Less,
						(false, true) => std::cmp::Ordering::Greater,
						(false, false) => std::cmp::Ordering::Equal,
					}
				});

				KeysGroup(keys)
			})
			.collect::<Vec<_>>()
	}
}
