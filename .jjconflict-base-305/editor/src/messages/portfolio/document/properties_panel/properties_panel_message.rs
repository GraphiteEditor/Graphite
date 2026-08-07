use crate::messages::prelude::*;

#[impl_message(Message, DocumentMessage, PropertiesPanel)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PropertiesPanelMessage {
	// Messages
	Clear,
	Refresh,
}
