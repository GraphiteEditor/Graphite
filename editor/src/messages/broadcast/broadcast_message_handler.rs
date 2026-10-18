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
			BroadcastMessage::SubscribeEvent { on, send } => {
				// Skip an existing listener, since some are subscribed again on every document switch
				let listeners = self.listeners.entry(on).or_default();
				if !listeners.contains(&send) {
					listeners.push(*send);
				}
			}
			BroadcastMessage::UnsubscribeEvent { on, send } => self.listeners.entry(on).or_default().retain(|msg| *msg != *send),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(EventMessageDiscriminant;)
	}
}

impl BroadcastMessageHandler {
	#[cfg(test)]
	pub fn listener_count(&self) -> usize {
		self.listeners.values().map(Vec::len).sum()
	}
}
