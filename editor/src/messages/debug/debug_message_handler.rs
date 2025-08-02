use super::utility_types::MessageLoggingVerbosity;
use crate::messages::prelude::*;

#[derive(Debug, Default, ExtractField)]
pub struct DebugMessageHandler {
	pub message_logging_verbosity: MessageLoggingVerbosity,
}

#[message_handler_data]
impl MessageHandler<DebugMessage, ()> for DebugMessageHandler {
	fn process_message(&mut self, message: DebugMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			DebugMessage::ToggleTraceLogs => {
				if log::max_level() == log::LevelFilter::Debug {
					log::set_max_level(log::LevelFilter::Trace);
				} else {
					log::set_max_level(log::LevelFilter::Debug);
				}

				// Refresh the checkmark beside the menu entry for this
				responses.add(MenuBarMessage::SendLayout);
			}
			DebugMessage::MessageOff => {
				self.message_logging_verbosity = MessageLoggingVerbosity::Off;

				// Refresh the checkmark beside the menu entry for this
				responses.add(MenuBarMessage::SendLayout);
			}
			DebugMessage::MessageNames => {
				self.message_logging_verbosity = MessageLoggingVerbosity::Names;

				// Refresh the checkmark beside the menu entry for this
				responses.add(MenuBarMessage::SendLayout);
			}
			DebugMessage::MessageContents => {
				self.message_logging_verbosity = MessageLoggingVerbosity::Contents;

				// Refresh the checkmark beside the menu entry for this
				responses.add(MenuBarMessage::SendLayout);
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
