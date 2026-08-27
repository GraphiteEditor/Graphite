use crate::messages::prelude::*;

#[impl_message(Message, MenuBar)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum MenuBarMessage {
	// Messages
	SendLayout,
}
