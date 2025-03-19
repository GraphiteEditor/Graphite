use std::time::Duration;

use crate::messages::prelude::*;

use super::TimingInformation;

#[derive(PartialEq, Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum AnimationTimeMode {
	#[default]
	TimeBased,
	FrameBased,
}

#[derive(Debug, Default)]
pub struct AnimationMessageHandler {
	live_preview: bool,
	/// Used to re-send the UI on the next frame after playback starts
	live_preview_recently_zero: bool,
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

	pub fn is_playing(&self) -> bool {
		self.live_preview
	}
}

impl MessageHandler<AnimationMessage, ()> for AnimationMessageHandler {
	fn process_message(&mut self, message: AnimationMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			AnimationMessage::ToggleLivePreview => {
				if self.animation_start.is_none() {
					self.animation_start = Some(self.timestamp);
				}
				self.live_preview = !self.live_preview;

				// Update the restart and pause/play buttons
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			AnimationMessage::EnableLivePreview => {
				if self.animation_start.is_none() {
					self.animation_start = Some(self.timestamp);
				}
				self.live_preview = true;

				// Update the restart and pause/play buttons
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			AnimationMessage::DisableLivePreview => {
				self.live_preview = false;

				// Update the restart and pause/play buttons
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			AnimationMessage::SetFrameIndex(frame) => {
				self.frame_index = frame;
				responses.add(PortfolioMessage::SubmitActiveGraphRender);
				// Update the restart and pause/play buttons
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			AnimationMessage::SetTime(time) => {
				self.timestamp = time;
				if self.live_preview {
					responses.add(AnimationMessage::UpdateTime);
				}
			}
			AnimationMessage::IncrementFrameCounter => {
				if self.live_preview {
					self.frame_index += 1.;
					responses.add(AnimationMessage::UpdateTime);
				}
			}
			AnimationMessage::UpdateTime => {
				if self.live_preview {
					responses.add(PortfolioMessage::SubmitActiveGraphRender);

					if self.live_preview_recently_zero {
						// Update the restart and pause/play buttons
						responses.add(PortfolioMessage::UpdateDocumentWidgets);
						self.live_preview_recently_zero = false;
					}
				}
			}
			AnimationMessage::RestartAnimation => {
				self.frame_index = 0.;
				self.animation_start = None;
				self.live_preview_recently_zero = true;
				responses.add(PortfolioMessage::SubmitActiveGraphRender);
				// Update the restart and pause/play buttons
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			AnimationMessage::SetAnimationTimeMode(animation_time_mode) => {
				self.animation_time_mode = animation_time_mode;
			}
		}
	}

	advertise_actions!(AnimationMessageDiscriminant;
		ToggleLivePreview,
		SetFrameIndex,
		RestartAnimation,
	);
}
