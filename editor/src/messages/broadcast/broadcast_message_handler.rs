use crate::messages::prelude::*;

#[derive(Debug, Clone, Default, ExtractField)]
pub struct BroadcastMessageHandler {
	event: EventMessageHandler,
	listeners: HashMap<EventMessage, Vec<Message>>,
}

#[message_handler_data]
impl MessageHandler<BroadcastMessage, ()> for BroadcastMessageHandler {
	fn process_message(&mut self, message: BroadcastMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			// Sub-messages
			BroadcastMessage::TriggerEvent(message) => self.event.process_message(message, responses, EventMessageContext { listeners: &mut self.listeners }),

			// Messages
			BroadcastMessage::SubscribeEvent { on, send } => self.listeners.entry(on).or_default().push(*send),
			BroadcastMessage::UnsubscribeEvent { on, send } => self.listeners.entry(on).or_default().retain(|msg| *msg != *send),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(EventMessageDiscriminant;)
	}
}
