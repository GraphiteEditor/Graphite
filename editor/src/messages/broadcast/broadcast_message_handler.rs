use crate::messages::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct BroadcastMessageHandler {
	listeners: HashMap<BroadcastEvent, Vec<Message>>,
}

impl MessageHandler<BroadcastMessage, ()> for BroadcastMessageHandler {
	fn process_message(&mut self, message: BroadcastMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			// Sub-messages
			BroadcastMessage::TriggerEvent(event) => {
				for message in self.listeners.entry(event).or_default() {
					responses.add_front(message.clone())
				}
			}

			// Messages
			BroadcastMessage::SubscribeEvent { on, send } => self.listeners.entry(on).or_default().push(*send),
			BroadcastMessage::UnsubscribeEvent { on, message } => self.listeners.entry(on).or_default().retain(|msg| *msg != *message),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(BroadcastEventDiscriminant;)
	}
}
