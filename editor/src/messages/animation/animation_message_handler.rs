use std::time::Duration;

use crate::messages::prelude::*;

use super::TimingInformation;

#[derive(Debug, Default)]
pub struct AnimationMessageHandler {
	live_preview: bool,
	timestamp: f64,
	frame_index: f64,
	frame_time: Duration,
}
impl AnimationMessageHandler {
	pub(crate) fn timing_information(&self) -> TimingInformation {
		TimingInformation {
			time: self.timestamp,
			frame_index: self.frame_index,
			frame_time: self.frame_time,
		}
	}
}

impl MessageHandler<AnimationMessage, ()> for AnimationMessageHandler {
	fn process_message(&mut self, message: AnimationMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			AnimationMessage::ToggleLivePreview => self.live_preview = !self.live_preview,
			AnimationMessage::EnableLivePreview => self.live_preview = true,
			AnimationMessage::DisableLivePreview => self.live_preview = false,
			AnimationMessage::SetFrameCounter(frame) => {
				self.frame_index = frame;
				responses.add(AnimationMessage::UpdateTime);
			}
			AnimationMessage::SetTime(time) => {
				self.timestamp = time;
				responses.add(AnimationMessage::UpdateTime);
			}
			AnimationMessage::IncrementFrameCounter => self.frame_index += 1.,
			AnimationMessage::SetFrameTime(duration) => {
				self.frame_time = duration;
				responses.add(AnimationMessage::UpdateTime);
			}
			AnimationMessage::UpdateTime => {
				if self.live_preview {
					responses.add(PortfolioMessage::SubmitActiveGraphRender)
				}
			}
		}
	}

	advertise_actions!(AnimationMessageDiscriminant;
		ToggleLivePreview,
		SetFrameTime,
		SetFrameIndex,
	);
}
