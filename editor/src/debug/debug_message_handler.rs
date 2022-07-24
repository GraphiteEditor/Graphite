use crate::debug::debug_message::LoggingMessages;
use crate::message_prelude::*;

use std::collections::VecDeque;

#[derive(Debug, Default)]
pub struct DebugMessageHandler {
	pub logging_messages_mode: LoggingMessages,
}

impl MessageHandler<DebugMessage, ()> for DebugMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: DebugMessage, _data: (), responses: &mut VecDeque<Message>) {
		match message {
			DebugMessage::ToggleTraceLogs => {
				if let log::LevelFilter::Debug = log::max_level() {
					log::set_max_level(log::LevelFilter::Trace);
				} else {
					log::set_max_level(log::LevelFilter::Debug);
				}

				// Refresh the checkmark beside the menu entry for this
				responses.push_back(MenuBarMessage::SendLayout.into());
			}
			DebugMessage::MessageOff => {
				self.logging_messages_mode = LoggingMessages::Off;

				// Refresh the checkmark beside the menu entry for this
				responses.push_back(MenuBarMessage::SendLayout.into());
			}
			DebugMessage::MessageNames => {
				self.logging_messages_mode = LoggingMessages::Names;

				// Refresh the checkmark beside the menu entry for this
				responses.push_back(MenuBarMessage::SendLayout.into());
			}
			DebugMessage::MessageContents => {
				self.logging_messages_mode = LoggingMessages::Contents;

				// Refresh the checkmark beside the menu entry for this
				responses.push_back(MenuBarMessage::SendLayout.into());
			}
		}
	}

	advertise_actions!(DebugMessageDiscriminant;
		ToggleTraceLogs,
		MessageOff,
		MessageNames,
		MessageContents,
	);
}
