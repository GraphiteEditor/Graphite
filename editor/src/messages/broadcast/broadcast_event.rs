use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Hash)]
#[impl_message(Message, BroadcastMessage, TriggerEvent)]
pub enum BroadcastEvent {
	AnimationFrame,
	CanvasTransformed,
	ToolAbort,
	SelectionChanged,
	WorkingColorChanged,
}
