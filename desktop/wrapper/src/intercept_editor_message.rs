use graphite_editor::messages::prelude::*;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{DesktopFrontendMessage, EditorMessage};

pub(super) fn intercept_editor_message(dispatcher: &mut DesktopWrapperMessageDispatcher, message: EditorMessage) -> Option<EditorMessage> {
	match message {
		EditorMessage::Viewport(ViewportMessage::UpdateBounds { x, y, width, height }) => {
			dispatcher.respond(DesktopFrontendMessage::UpdateViewportBounds { x, y, width, height });
			Some(ViewportMessage::UpdateBounds { x, y, width, height }.into())
		}
		m => Some(m),
	}
}
