pub mod snapping;
pub mod tool;
pub mod tool_message;
pub mod tool_message_handler;
pub mod tools;
pub mod vector_editor;

#[cfg(test)]
mod tool_crash_on_layer_delete_tests {
	use crate::{Editor, DocumentMessage};
	use crate::misc::test_utils::EditorTestUtils;
	use crate::viewport_tools::tool::ToolType;
	use crate::communication::set_uuid_seed;

	#[test]
	fn should_not_crash_when_layer_is_deleted_while_using_a_tool() {
		set_uuid_seed(0);
		let mut test_editor = Editor::new();

		test_editor.select_tool(ToolType::Pen);
		test_editor.lmb_mousedown(0.0, 0.0);
		test_editor.move_mouse(100.0, 100.0);

		test_editor.handle_message(DocumentMessage::DeleteSelectedLayers);
	}
}
