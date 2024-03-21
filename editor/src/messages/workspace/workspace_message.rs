use crate::messages::prelude::*;

#[impl_message(Message, Workspace)]
#[derive(PartialEq, Eq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum WorkspaceMessage {
	// Messages
	NodeGraphToggleVisibility,
}
