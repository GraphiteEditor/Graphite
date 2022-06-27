use std::collections::HashMap;

use crate::message_prelude::*;

#[derive(Debug, Clone, Default)]
pub struct BroadcastMessageHandler {
	listeners: HashMap<BroadcastSignal, Vec<Message>>,
}

impl MessageHandler<BroadcastMessage, ()> for BroadcastMessageHandler {
	fn process_action(&mut self, action: BroadcastMessage, _data: (), responses: &mut VecDeque<Message>) {
		use BroadcastMessage::*;
		match action {
			SubscribeSignal { on, send } => self.listeners.entry(on).or_default().push(*send),
			UnsubscribeSignal { on, message } => self.listeners.entry(on).or_default().retain(|msg| *msg != *message),
			TriggerSignal { signal } => responses.extend(self.listeners.entry(signal).or_default().clone().into_iter()),
			TriggerSignalImmediate { signal } => {
				for message in self.listeners.entry(signal).or_default() {
					responses.push_front(message.clone())
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		vec![]
	}
}
