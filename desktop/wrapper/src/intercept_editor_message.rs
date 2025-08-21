use graphite_editor::messages::prelude::InputPreprocessorMessage;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{DesktopFrontendMessage, EditorMessage};

pub(super) fn intercept_editor_message(dispatcher: &mut DesktopWrapperMessageDispatcher, message: EditorMessage) -> Option<EditorMessage> {
	match message {
		EditorMessage::InputPreprocessor(message) => {
			if let InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports } = &message {
				let top_left = bounds_of_viewports[0].top_left;
				let bottom_right = bounds_of_viewports[0].bottom_right;
				dispatcher.respond(DesktopFrontendMessage::UpdateViewportBounds {
					x: top_left.x as f32,
					y: top_left.y as f32,
					width: (bottom_right.x - top_left.x) as f32,
					height: (bottom_right.y - top_left.y) as f32,
				});
			}
			Some(EditorMessage::InputPreprocessor(message))
		}
		m => Some(m),
	}
}
