use super::DesktopWrapperMessageDispatcher;
use super::messages::EditorMessage;

pub(super) fn intercept_editor_message(_dispatcher: &mut DesktopWrapperMessageDispatcher, message: EditorMessage) -> Option<EditorMessage> {
	// TODO: remove it turns out to be unnecessary
	Some(message)
}
