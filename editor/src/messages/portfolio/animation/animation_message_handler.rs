use crate::{messages::prelude::*, node_graph_executor::NodeGraphExecutor};
use graphene_std::application_io::AnimationConfig;

pub struct AnimationMessageData<'a> {
	pub executor: &'a mut NodeGraphExecutor,
	pub document: &'a mut DocumentMessageHandler,
	pub ipp: &'a InputPreprocessorMessageHandler,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationMessageHandler {
	pub animation_config: AnimationConfig,
}

impl MessageHandler<AnimationMessage, AnimationMessageData<'_>> for AnimationMessageHandler {
	fn process_message(&mut self, message: AnimationMessage, responses: &mut VecDeque<Message>, data: AnimationMessageData<'_>) {
		let AnimationMessageData { executor, document, ipp } = data;
		let animation_config = &mut self.animation_config;
		match message {
			AnimationMessage::Restart => {
				animation_config.time = 0.;
				log::debug!("Animation time: {}", animation_config.time);
				let result = executor.submit_node_graph_evaluation_with_animation(document, ipp.viewport_bounds.size().as_uvec2(), *animation_config);
				if let Err(description) = result {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Unable to update node graph".to_string(),
						description,
					});
				}
			}
			AnimationMessage::NextFrame => {
				if !animation_config.is_playing {
					return;
				}
				animation_config.time += 1. / animation_config.frame_rate as f64;
				log::debug!("Animation time: {}", animation_config.time);
				let result = executor.submit_node_graph_evaluation_with_animation(document, ipp.viewport_bounds.size().as_uvec2(), *animation_config);
				if let Err(description) = result {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Unable to update node graph".to_string(),
						description,
					});
				}
			}
			AnimationMessage::Play => {
				animation_config.is_playing = true;
			}
			AnimationMessage::Pause => {
				animation_config.is_playing = false;
			}
		}
	}

	fn actions(&self) -> ActionList {
		let common = actions!(AnimationMessageDiscriminant;
			NextFrame,
			Play,
			Pause,
			Restart,
		);
		common
	}
}
