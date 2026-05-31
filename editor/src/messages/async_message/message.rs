use crate::messages::async_message::MessageFuture;
use crate::messages::prelude::*;

#[impl_message(Message, Async)]
#[derive(derivative::Derivative, Clone, serde::Serialize, serde::Deserialize)]
#[derivative(Debug, PartialEq)]
pub enum AsyncMessage {
	/// Spawn `future`; its resolved [`Message`] re-enters the dispatcher on the next tick.
	Await {
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore")]
		future: MessageFuture,
	},
	/// Sent by a wake callback to nudge an idle event loop into a dispatch tick.
	Wake,
}
