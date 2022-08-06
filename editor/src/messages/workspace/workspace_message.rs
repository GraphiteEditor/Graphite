use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Workspace)]
#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub enum WorkspaceMessage {
	// Messages
	NodeGraphToggleVisibility,
}
