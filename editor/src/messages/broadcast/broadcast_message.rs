use crate::messages::prelude::*;

#[impl_message(Message, Broadcast)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum BroadcastMessage {
	// Sub-messages
	#[child]
	TriggerEvent(BroadcastEvent),

	// Messages
	SubscribeEvent {
		on: BroadcastEvent,
		send: Box<Message>,
	},
	UnsubscribeEvent {
		on: BroadcastEvent,
		message: Box<Message>,
	},
}
