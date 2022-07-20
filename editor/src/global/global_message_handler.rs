use crate::message_prelude::*;

use std::collections::VecDeque;

#[derive(Debug, Default)]
pub struct GlobalMessageHandler {
	pub trace_messsage_contents: bool,
}

impl MessageHandler<GlobalMessage, ()> for GlobalMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: GlobalMessage, _data: (), _responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
			GlobalMessage::LogMaxLevelDebug => {
				log::set_max_level(log::LevelFilter::Debug);
				log::info!("Set log verbosity to debug");
			}
			GlobalMessage::LogMaxLevelInfo => {
				log::set_max_level(log::LevelFilter::Info);
				log::info!("Set log verbosity to info");
			}
			GlobalMessage::LogMaxLevelTrace => {
				log::set_max_level(log::LevelFilter::Trace);
				log::info!("Set log verbosity to trace");
			}
			GlobalMessage::TraceMessageContents => {
				self.trace_messsage_contents = true;
				log::info!("Tracing message contents");
			}
			GlobalMessage::TraceMessageDiscriminants => {
				self.trace_messsage_contents = false;
				log::info!("Tracing message discriminants");
			}
		}
	}

	advertise_actions!(GlobalMessageDiscriminant; LogMaxLevelDebug, LogMaxLevelInfo, LogMaxLevelTrace, TraceMessageContents, TraceMessageDiscriminants);
}
