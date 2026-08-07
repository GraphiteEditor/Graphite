use crate::messages::future::MessageFuture;
use crate::messages::prelude::*;

#[impl_message(Message, Future)]
#[derive(derivative::Derivative, Clone, serde::Serialize, serde::Deserialize)]
#[derivative(Debug, PartialEq)]
pub enum FutureMessage {
	/// Spawn `future`; its resolved [`Message`] re-enters the dispatcher on the next tick.
	Await {
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore")]
		future: MessageFuture,
	},
	/// Sent by a wake callback to nudge an idle event loop into a dispatch tick.
	Wake,
}
