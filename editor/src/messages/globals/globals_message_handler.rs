use crate::messages::prelude::*;

#[derive(Debug, Default)]
pub struct GlobalsMessageHandler {}

impl MessageHandler<GlobalsMessage, ()> for GlobalsMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: GlobalsMessage, _data: (), _responses: &mut VecDeque<Message>) {
		match message {
			GlobalsMessage::SetPlatform { platform } => {
				if GLOBAL_PLATFORM.get() != Some(&platform) {
					GLOBAL_PLATFORM.set(platform).expect("Failed to set GLOBAL_PLATFORM");
				}
			}
		}
	}

	advertise_actions!(GlobalsMessageDiscriminant;
	);
}
