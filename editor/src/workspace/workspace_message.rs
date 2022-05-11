use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Workspace)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum WorkspaceMessage {
	// Messages
	NodeGraphToggleVisibility,
}
