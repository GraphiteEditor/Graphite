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

	use test_case::test_case;

	#[test_case(ToolType::Pen ; "while using pen tool")]
	#[test_case(ToolType::Freehand ; "while using freehand tool")]
	#[test_case(ToolType::Spline ; "while using spline tool")]
	#[test_case(ToolType::Line ; "while using line tool")]
	#[test_case(ToolType::Rectangle ; "while using rectangle tool")]
	#[test_case(ToolType::Ellipse ; "while using ellipse tool")]
	#[test_case(ToolType::Shape ; "while using shape tool")]
	#[test_case(ToolType::Path ; "while using path tool")]
	fn should_not_crash_when_layer_is_deleted(tool: ToolType) {
		set_uuid_seed(0);
		let mut test_editor = Editor::new();

		test_editor.select_tool(tool);
		test_editor.lmb_mousedown(0.0, 0.0);
		test_editor.move_mouse(100.0, 100.0);

		test_editor.handle_message(DocumentMessage::DeleteSelectedLayers);
	}
}
