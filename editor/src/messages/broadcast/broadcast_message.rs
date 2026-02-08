use crate::messages::prelude::*;

#[impl_message(Message, Broadcast)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum BroadcastMessage {
	// Sub-messages
	#[child]
	TriggerEvent(EventMessage),

	// Messages
	SubscribeEvent {
		on: EventMessage,
		send: Box<Message>,
	},
	UnsubscribeEvent {
		on: EventMessage,
		send: Box<Message>,
	},
}
