use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct EventMessageContext<'a> {
	pub listeners: &'a mut HashMap<EventMessage, Vec<Message>>,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct EventMessageHandler {}

#[message_handler_data]
impl MessageHandler<EventMessage, EventMessageContext<'_>> for EventMessageHandler {
	fn process_message(&mut self, message: EventMessage, responses: &mut VecDeque<Message>, context: EventMessageContext) {
		for message in context.listeners.entry(message).or_default() {
			responses.add_front(message.clone())
		}
	}

	fn actions(&self) -> ActionList {
		actions!(EventMessageDiscriminant;)
	}
}
