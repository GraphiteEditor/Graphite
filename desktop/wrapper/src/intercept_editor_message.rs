use graphite_editor::messages::prelude::*;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{DesktopFrontendMessage, EditorMessage};

pub(super) fn intercept_editor_message(dispatcher: &mut DesktopWrapperMessageDispatcher, message: EditorMessage) -> Option<EditorMessage> {
	match message {
		m => Some(m),
	}
}
