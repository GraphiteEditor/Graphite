use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, Broadcast)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum BroadcastMessage {
	SubscribeSignal {
		on: BroadcastSignal,
		send: Box<Message>,
	},
	UnsubscribeSignal {
		on: BroadcastSignal,
		message: Box<Message>,
	},
	#[child]
	TriggerSignal(BroadcastSignal),
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Hash)]
#[impl_message(Message, BroadcastMessage, TriggerSignal)]
pub enum BroadcastSignal {
	DocumentIsDirty,
	ToolAbort,
	SelectionChanged,
}
