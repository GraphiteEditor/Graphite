use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, Debug)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum DebugMessage {
	ToggleTraceLogs,
	MessageOff,
	MessageNames,
	MessageContents,
}
