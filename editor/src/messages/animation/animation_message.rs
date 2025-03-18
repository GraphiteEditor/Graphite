use crate::messages::prelude::*;

#[impl_message(Message, Animation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AnimationMessage {
	ToggleLivePreview,
	EnableLivePreview,
	DisableLivePreview,
	ResetAnimation,
	SetFrameIndex(f64),
	SetTime(f64),
	UpdateTime,
	IncrementFrameCounter,
}
