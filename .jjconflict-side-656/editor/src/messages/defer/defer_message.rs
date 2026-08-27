use crate::messages::prelude::*;

#[impl_message(Message, Defer)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DeferMessage {
	SetGraphSubmissionIndex { execution_id: u64 },
	TriggerGraphRun { execution_id: u64, document_id: DocumentId },
	AfterGraphRun { messages: Vec<Message> },
	TriggerNavigationReady,
	AfterNavigationReady { messages: Vec<Message> },
}
