use std::time::Duration;

use crate::messages::prelude::*;

#[impl_message(Message, Animation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AnimationMessage {
	ToggleLivePreview,
	EnableLivePreview,
	DisableLivePreview,
	SetFrameIndex(f64),
	SetFrameTime(Duration),
	SetTime(f64),
	UpdateTime,
	IncrementFrameCounter,
}
