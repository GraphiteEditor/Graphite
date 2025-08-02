use crate::messages::prelude::*;

use super::animation_message_handler::AnimationTimeMode;

#[impl_message(Message, Animation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AnimationMessage {
	ToggleLivePreview,
	EnableLivePreview,
	DisableLivePreview,
	RestartAnimation,
	SetFrameIndex { frame: f64 },
	SetTime { time: f64 },
	UpdateTime,
	IncrementFrameCounter,
	SetAnimationTimeMode { animation_time_mode: AnimationTimeMode },
}
