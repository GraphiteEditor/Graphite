use crate::messages::prelude::*;

#[impl_message(Message, DocumentMessage, Resource)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ResourceMessage {
	Noop,
}
