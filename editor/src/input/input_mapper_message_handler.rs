use super::input_mapper::Mapping;
use super::keyboard::Key;
use super::InputPreprocessorMessageHandler;
use crate::message_prelude::*;

use std::fmt::Write;

#[derive(Debug, Default)]
pub struct InputMapperMessageHandler {
	mapping: Mapping,
}

impl InputMapperMessageHandler {
	pub fn hints(&self, actions: ActionList) -> String {
		let mut output = String::new();
		let mut actions = actions
			.into_iter()
			.flatten()
			.filter(|a| !matches!(*a, MessageDiscriminant::Tool(ToolMessageDiscriminant::ActivateTool) | MessageDiscriminant::Global(_)));
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
}

impl MessageHandler<InputMapperMessage, (&InputPreprocessorMessageHandler, ActionList)> for InputMapperMessageHandler {
	fn process_action(&mut self, message: InputMapperMessage, data: (&InputPreprocessorMessageHandler, ActionList), responses: &mut VecDeque<Message>) {
		let (input, actions) = data;
		if let Some(message) = self.mapping.match_message(message, &input.keyboard, actions) {
			responses.push_back(message);
		}
	}
	advertise_actions!();
}
