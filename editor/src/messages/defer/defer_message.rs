use crate::messages::prelude::*;

#[impl_message(Message, Defer)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DeferMessage {
	TriggerGraphRun(u64),
	AfterGraphRun { messages: Vec<Message> },
	TriggerViewportReady,
	AfterViewportReady { messages: Vec<Message> },
}
