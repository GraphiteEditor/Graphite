use graphite_editor::messages::prelude::{InputPreprocessorMessage, Message};

use crate::editor_api::messages::NativeMessage;

use super::EditorMessageExecutor;

pub(super) fn intercept_message(executor: &mut EditorMessageExecutor, message: Message) -> Option<Message> {
	match message {
		Message::InputPreprocessor(message) => {
			if let InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports } = &message {
				let top_left = bounds_of_viewports[0].top_left;
				let bottom_right = bounds_of_viewports[0].bottom_right;
				executor.respond(NativeMessage::UpdateViewportBounds {
					x: top_left.x as f32,
					y: top_left.y as f32,
					width: (bottom_right.x - top_left.x) as f32,
					height: (bottom_right.y - top_left.y) as f32,
				});
			}
			executor.respond(NativeMessage::RequestRedraw);
			Some(Message::InputPreprocessor(message))
		}
		m => Some(m),
	}
}
