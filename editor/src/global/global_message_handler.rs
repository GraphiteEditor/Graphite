use crate::message_prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[impl_message(Message, Global)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum GlobalMessage {
	LogInfo,
	LogDebug,
	LogTrace,
}

#[derive(Debug, Default)]
pub struct GlobalMessageHandler {}

impl GlobalMessageHandler {
	pub fn new() -> Self {
		Self::default()
	}
}

impl MessageHandler<GlobalMessage, ()> for GlobalMessageHandler {
	fn process_action(&mut self, message: GlobalMessage, _data: (), _responses: &mut VecDeque<Message>) {
		use GlobalMessage::*;
		match message {
			LogInfo => {
				log::set_max_level(log::LevelFilter::Info);
				log::info!("set log verbosity to info");
			}
			LogDebug => {
				log::set_max_level(log::LevelFilter::Debug);
				log::info!("set log verbosity to debug");
			}
			LogTrace => {
				log::set_max_level(log::LevelFilter::Trace);
				log::info!("set log verbosity to trace");
			}
		}
	}
	advertise_actions!(GlobalMessageDiscriminant; LogInfo, LogDebug, LogTrace);
}
