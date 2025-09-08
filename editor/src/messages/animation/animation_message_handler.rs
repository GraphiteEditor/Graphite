use std::time::Duration;

use crate::messages::prelude::*;

use super::TimingInformation;

#[derive(PartialEq, Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum AnimationTimeMode {
	#[default]
	TimeBased,
	FrameBased,
}

#[derive(Default, Debug, Clone, PartialEq)]
enum AnimationState {
	#[default]
	Stopped,
	Playing {
		start: f64,
	},
	Paused {
		start: f64,
		pause_time: f64,
	},
}

#[derive(Default, Debug, Clone, PartialEq, ExtractField)]
pub struct AnimationMessageHandler {
	/// Used to re-send the UI on the next frame after playback starts
	live_preview_recently_zero: bool,
	timestamp: f64,
	frame_index: f64,
	animation_state: AnimationState,
	fps: f64,
	animation_time_mode: AnimationTimeMode,
}
impl AnimationMessageHandler {
	pub(crate) fn timing_information(&self) -> TimingInformation {
		let animation_time = self.timestamp - self.animation_start();
		let animation_time = match self.animation_time_mode {
			AnimationTimeMode::TimeBased => Duration::from_millis(animation_time as u64),
			AnimationTimeMode::FrameBased => Duration::from_secs((self.frame_index / self.fps) as u64),
		};
		TimingInformation { time: self.timestamp, animation_time }
	}

	pub(crate) fn animation_start(&self) -> f64 {
		match self.animation_state {
			AnimationState::Stopped => self.timestamp,
			AnimationState::Playing { start } => start,
			AnimationState::Paused { start, pause_time } => start + self.timestamp - pause_time,
		}
	}

	pub fn is_playing(&self) -> bool {
		matches!(self.animation_state, AnimationState::Playing { .. })
	}
}

#[message_handler_data]
impl MessageHandler<AnimationMessage, ()> for AnimationMessageHandler {
	fn process_message(&mut self, message: AnimationMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			AnimationMessage::ToggleLivePreview => match self.animation_state {
				AnimationState::Stopped => responses.add(AnimationMessage::EnableLivePreview),
				AnimationState::Playing { .. } => responses.add(AnimationMessage::DisableLivePreview),
				AnimationState::Paused { .. } => responses.add(AnimationMessage::EnableLivePreview),
			},
			AnimationMessage::EnableLivePreview => {
				self.animation_state = AnimationState::Playing { start: self.animation_start() };

				// Update the restart and pause/play buttons
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			AnimationMessage::DisableLivePreview => {
				match self.animation_state {
					AnimationState::Stopped => (),
					AnimationState::Playing { start } => self.animation_state = AnimationState::Paused { start, pause_time: self.timestamp },
					AnimationState::Paused { .. } => (),
				}

				// Update the restart and pause/play buttons
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			AnimationMessage::SetFrameIndex { frame } => {
				self.frame_index = frame;
				responses.add(PortfolioMessage::SubmitActiveGraphRender);
				// Update the restart and pause/play buttons
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			AnimationMessage::SetTime { time } => {
				self.timestamp = time;
				responses.add(AnimationMessage::UpdateTime);
			}
			AnimationMessage::IncrementFrameCounter => {
				if self.is_playing() {
					self.frame_index += 1.;
					responses.add(AnimationMessage::UpdateTime);
				}
			}
			AnimationMessage::UpdateTime => {
				if self.is_playing() {
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
				self.animation_state = match self.animation_state {
					AnimationState::Playing { .. } => AnimationState::Playing { start: self.timestamp },
					_ => AnimationState::Stopped,
				};
				self.live_preview_recently_zero = true;
				responses.add(PortfolioMessage::SubmitActiveGraphRender);
				// Update the restart and pause/play buttons
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			AnimationMessage::SetAnimationTimeMode { animation_time_mode } => {
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
