use crate::messages::prelude::*;

#[impl_message(Message, Debug)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum DebugMessage {
	ToggleTraceLogs,
	MessageOff,
	MessageNames,
	MessageContents,
}
