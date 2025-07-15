use crate::messages::prelude::*;

#[derive(Debug, Default, ExtractField)]
pub struct GlobalsMessageHandler {}

#[message_handler_data]
impl MessageHandler<GlobalsMessage, ()> for GlobalsMessageHandler {
	fn process_message(&mut self, message: GlobalsMessage, _responses: &mut VecDeque<Message>, _: ()) {
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
