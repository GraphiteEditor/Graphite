use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, DocumentMessage, PropertiesPanel)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PropertiesPanelMessage {
	// Messages
	Clear,
	Refresh,
}
