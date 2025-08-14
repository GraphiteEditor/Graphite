use graphite_editor::messages::prelude::{InputPreprocessorMessage, Message};

use crate::editor_api::messages::NativeMessage;

pub(super) fn intercept_message(message: Message, responses: &mut Vec<NativeMessage>) -> Option<Message> {
	match message {
		Message::InputPreprocessor(InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports }) => {
			let top_left = bounds_of_viewports[0].top_left;
			let bottom_right = bounds_of_viewports[0].bottom_right;
			responses.push(NativeMessage::UpdateViewportBounds {
				x: top_left.x as f32,
				y: top_left.y as f32,
				width: (bottom_right.x - top_left.x) as f32,
				height: (bottom_right.y - top_left.y) as f32,
			});
		}
		m => return Some(m),
	}
	None
}
