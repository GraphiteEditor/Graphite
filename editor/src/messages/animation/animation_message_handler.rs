use std::time::Duration;

use crate::messages::prelude::*;

use super::TimingInformation;

#[derive(Debug, Default)]
pub enum AnimationTimeMode {
	#[default]
	TimeBased,
	FrameBased,
}

#[derive(Debug, Default)]
pub struct AnimationMessageHandler {
	live_preview: bool,
	timestamp: f64,
	frame_index: f64,
	animation_start: Option<f64>,
	fps: f64,
	animation_time_mode: AnimationTimeMode,
}
impl AnimationMessageHandler {
	pub(crate) fn timing_information(&self) -> TimingInformation {
		let animation_time = self.timestamp - self.animation_start.unwrap_or(self.timestamp);
		let animation_time = match self.animation_time_mode {
			AnimationTimeMode::TimeBased => Duration::from_millis(animation_time as u64),
			AnimationTimeMode::FrameBased => Duration::from_secs((self.frame_index / self.fps) as u64),
		};
		TimingInformation { time: self.timestamp, animation_time }
	}
}

impl MessageHandler<AnimationMessage, ()> for AnimationMessageHandler {
	fn process_message(&mut self, message: AnimationMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			AnimationMessage::ToggleLivePreview => {
				if self.animation_start.is_none() {
					self.animation_start = Some(self.timestamp);
				}
				self.live_preview = !self.live_preview
			}
			AnimationMessage::EnableLivePreview => {
				if self.animation_start.is_none() {
					self.animation_start = Some(self.timestamp);
				}
				self.live_preview = true
			}
			AnimationMessage::DisableLivePreview => self.live_preview = false,
			AnimationMessage::SetFrameIndex(frame) => {
				self.frame_index = frame;
				log::debug!("set frame index to {}", frame);
				responses.add(PortfolioMessage::SubmitActiveGraphRender)
			}
			AnimationMessage::SetTime(time) => {
				self.timestamp = time;
				responses.add(AnimationMessage::UpdateTime);
			}
			AnimationMessage::IncrementFrameCounter => {
				if self.live_preview {
					self.frame_index += 1.;
					responses.add(AnimationMessage::UpdateTime);
				}
			}
			AnimationMessage::UpdateTime => {
				if self.live_preview {
					responses.add(PortfolioMessage::SubmitActiveGraphRender)
				}
			}
			AnimationMessage::ResetAnimation => {
				self.frame_index = 0.;
				self.animation_start = None;
				responses.add(PortfolioMessage::SubmitActiveGraphRender)
			}
		}
	}

	advertise_actions!(AnimationMessageDiscriminant;
		ToggleLivePreview,
		SetFrameIndex,
		ResetAnimation,
	);
}
