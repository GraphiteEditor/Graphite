use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, Broadcast)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum BroadcastMessage {
	SubscribeSignal { on: BroadcastSignal, send: Box<Message> },
	UnsubscribeSignal { on: BroadcastSignal, message: Box<Message> },
	TriggerSignal { signal: BroadcastSignal },
	TriggerSignalFront { signal: BroadcastSignal },
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Hash, ToDiscriminant)]
#[discriminant_attr(derive(Debug, Eq, PartialEq))]
pub enum BroadcastSignal {
	DocumentIsDirty,
	ToolAbort,
	SelectionChanged,
}

impl BroadcastSignal {
	pub fn into_front(self) -> Message {
		BroadcastMessage::TriggerSignalFront { signal: self }.into()
	}
}

impl From<BroadcastSignal> for Message {
	fn from(signal: BroadcastSignal) -> Self {
		BroadcastMessage::TriggerSignal { signal }.into()
	}
}
