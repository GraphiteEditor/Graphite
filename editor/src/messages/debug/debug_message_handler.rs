use super::utility_types::MessageLoggingVerbosity;
use crate::messages::prelude::*;

#[derive(Debug, Default)]
pub struct DebugMessageHandler {
	pub message_logging_verbosity: MessageLoggingVerbosity,
}

impl MessageHandler<DebugMessage, ()> for DebugMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: DebugMessage, _data: (), responses: &mut VecDeque<Message>) {
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
				self.message_logging_verbosity = MessageLoggingVerbosity::Off;

				// Refresh the checkmark beside the menu entry for this
				responses.push_back(MenuBarMessage::SendLayout.into());
			}
			DebugMessage::MessageNames => {
				self.message_logging_verbosity = MessageLoggingVerbosity::Names;

				// Refresh the checkmark beside the menu entry for this
				responses.push_back(MenuBarMessage::SendLayout.into());
			}
			DebugMessage::MessageContents => {
				self.message_logging_verbosity = MessageLoggingVerbosity::Contents;

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
