use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Broadcast)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum BroadcastMessage {
	SubscribeSignal { on: BroadcastSignal, send: Box<Message> },
	TriggerSignal { signal: BroadcastSignal },
	TriggerSignalImmediate { signal: BroadcastSignal },
	UnsubscribeSignal { on: BroadcastSignal, message: Box<Message> },
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum BroadcastSignal {
	DocumentIsDirty,
	Abort,
	SelectionChanged,
}
