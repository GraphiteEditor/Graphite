use crate::message_prelude::*;

use std::collections::VecDeque;

#[derive(Debug, Default)]
pub struct GlobalMessageHandler {}

impl MessageHandler<GlobalMessage, ()> for GlobalMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: GlobalMessage, _data: (), _responses: &mut VecDeque<Message>) {
		use GlobalMessage::*;

		#[remain::sorted]
		match message {
			LogDebug => {
				log::set_max_level(log::LevelFilter::Debug);
				log::info!("Set log verbosity to debug");
			}
			LogInfo => {
				log::set_max_level(log::LevelFilter::Info);
				log::info!("Set log verbosity to info");
			}
			LogTrace => {
				log::set_max_level(log::LevelFilter::Trace);
				log::info!("Set log verbosity to trace");
			}
		}
	}

	advertise_actions!(GlobalMessageDiscriminant;
		LogInfo,
		LogDebug,
		LogTrace,
	);
}
