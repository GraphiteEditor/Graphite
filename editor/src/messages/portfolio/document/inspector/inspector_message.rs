use crate::messages::prelude::*;

#[impl_message(Message, DocumentMessage, Inspector)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum InspectorMessage {
	// Messages
	Clear,
	Refresh,
}
