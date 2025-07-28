use crate::messages::prelude::*;

#[impl_message(Message, Defer)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DeferMessage {
	TriggerGraphRun,
	AfterGraphRun { messages: Vec<Message> },
	TriggerViewportResize,
	AfterViewportResize { messages: Vec<Message> },
}
