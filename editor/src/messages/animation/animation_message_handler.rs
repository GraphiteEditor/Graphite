use crate::messages::prelude::*;

use super::TimingInformation;

#[derive(Debug, Default)]
pub struct AnimationMessageHandler {
	live_preview: bool,
	timestamp: f64,
	frame_index: f64,
	frame_time: f64,
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
				if self.live_preview {
					responses.add(PortfolioMessage::SubmitActiveGraphRender)
				}
			}
			AnimationMessage::SetTime(time) => {
				self.timestamp = time;
				if self.live_preview {
					responses.add(PortfolioMessage::SubmitActiveGraphRender)
				}
			}
			AnimationMessage::IncrementFrameCounter => self.frame_index += 1.,
		}
	}

	advertise_actions!(AnimationMessageDiscriminant;
		ToggleLivePreview,
	);
}
