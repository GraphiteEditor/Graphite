use crate::messages::prelude::*;

#[derive(PartialEq, Eq, Clone, Debug, serde::Serialize, serde::Deserialize, Hash)]
#[impl_message(Message, BroadcastMessage, TriggerEvent)]
pub enum BroadcastEvent {
	/// Triggered by requestAnimationFrame in JS
	AnimationFrame,
	CanvasTransformed,
	ToolAbort,
	SelectionChanged,
	WorkingColorChanged,
}
