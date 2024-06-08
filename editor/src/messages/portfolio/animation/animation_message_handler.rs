use crate::{
	messages::{portfolio::utility_types::PersistentData, prelude::*},
	node_graph_executor::NodeGraphExecutor,
};
pub struct AnimationMessageData<'a> {
	pub persistent_data: &'a mut PersistentData,
	pub executor: &'a mut NodeGraphExecutor,
	pub document: &'a mut DocumentMessageHandler,
	pub ipp: &'a InputPreprocessorMessageHandler,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimationMessageHandler {}

impl MessageHandler<AnimationMessage, AnimationMessageData<'_>> for AnimationMessageHandler {
	fn process_message(&mut self, message: AnimationMessage, responses: &mut VecDeque<Message>, data: AnimationMessageData<'_>) {
		let AnimationMessageData {
			persistent_data,
			executor,
			document,
			ipp,
		} = data;
		match message {
			AnimationMessage::Restart => {
				persistent_data.animation.time = 0.;
				log::debug!("Animation time: {}", persistent_data.animation.time);
				let result = executor.submit_node_graph_evaluation_with_animation(document, ipp.viewport_bounds.size().as_uvec2(), persistent_data.animation);
				if let Err(description) = result {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Unable to update node graph".to_string(),
						description,
					});
				}
			}
			AnimationMessage::NextFrame => {
				if !persistent_data.animation.is_playing {
					return;
				}
				persistent_data.animation.time += 1. / persistent_data.animation.frame_rate as f64;
				log::debug!("Animation time: {}", persistent_data.animation.time);
				let result = executor.submit_node_graph_evaluation_with_animation(document, ipp.viewport_bounds.size().as_uvec2(), persistent_data.animation);
				if let Err(description) = result {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Unable to update node graph".to_string(),
						description,
					});
				}
			}
			AnimationMessage::Play => {
				persistent_data.animation.is_playing = true;
			}
			AnimationMessage::Pause => {
				persistent_data.animation.is_playing = false;
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

// impl AnimationMessageHandler {
// 	fn window() -> web_sys::Window {
// 		web_sys::window().expect("no global `window` exists")
// 	}

// 	fn request_animation_frame(window: &web_sys::Window, f: &Closure<dyn FnMut()>) -> i32 {
// 		window.request_animation_frame(f.as_ref().unchecked_ref()).expect("should register `requestAnimationFrame` OK")
// 	}

// 	fn set_timeout(window: &web_sys::Window, f: &Closure<dyn FnMut()>, timeout_ms: i32) -> i32 {
// 		window
// 			.set_timeout_with_callback_and_timeout_and_arguments_0(f.as_ref().unchecked_ref(), timeout_ms)
// 			.expect("should register `setTimeout` OK")
// 	}
// }
