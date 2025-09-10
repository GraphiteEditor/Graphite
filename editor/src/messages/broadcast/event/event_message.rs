use crate::messages::prelude::*;

#[impl_message(Message, BroadcastMessage, TriggerEvent)]
#[derive(PartialEq, Eq, Clone, Debug, serde::Serialize, serde::Deserialize, Hash)]
pub enum EventMessage {
	/// Triggered by requestAnimationFrame in JS
	AnimationFrame,
	CanvasTransformed,
	ToolAbort,
	SelectionChanged,
	WorkingColorChanged,
}
