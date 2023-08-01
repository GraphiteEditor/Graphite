use crate::application::set_uuid_seed;
use crate::application::Editor;
use crate::messages::input_mapper::utility_types::input_keyboard::ModifierKeys;
use crate::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, MouseKeys, ScrollDelta, ViewportPosition};
use crate::messages::portfolio::utility_types::Platform;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::ToolType;

use graphene_core::raster::color::Color;

/// A set of utility functions to make the writing of editor test more declarative
pub trait EditorTestUtils {
	fn create() -> Editor;

	fn new_document(&mut self);

	fn draw_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64);
	fn draw_polygon(&mut self, x1: f64, y1: f64, x2: f64, y2: f64);
	fn draw_ellipse(&mut self, x1: f64, y1: f64, x2: f64, y2: f64);

	/// Select given tool and drag it from (x1, y1) to (x2, y2)
	fn drag_tool(&mut self, typ: ToolType, x1: f64, y1: f64, x2: f64, y2: f64);
	fn move_mouse(&mut self, x: f64, y: f64);
	fn mousedown(&mut self, state: EditorMouseState);
	fn mouseup(&mut self, state: EditorMouseState);
	fn lmb_mousedown(&mut self, x: f64, y: f64);
	fn input(&mut self, message: InputPreprocessorMessage);
	fn select_tool(&mut self, typ: ToolType);
	fn select_primary_color(&mut self, color: Color);
}

impl EditorTestUtils for Editor {
	fn create() -> Editor {
		set_uuid_seed(0);

		let mut editor = Editor::new();

		// We have to set this directly instead of using `GlobalsMessage::SetPlatform` because race conditions with multiple tests can cause that message handler to set it more than once, which is a failure.
		// It isn't sufficient to guard the message dispatch here with a check if the once_cell is empty, because that isn't atomic and the time between checking and handling the dispatch can let multiple through.
		let _ = GLOBAL_PLATFORM.set(Platform::Windows).is_ok();

		editor.handle_message(Message::Init);

		editor
	}

	fn new_document(&mut self) {
		self.handle_message(Message::Portfolio(PortfolioMessage::NewDocumentWithName { name: String::from("Test document") }));
	}

	fn draw_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Rectangle, x1, y1, x2, y2);
	}

	fn draw_polygon(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Polygon, x1, y1, x2, y2);
	}

	fn draw_ellipse(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Ellipse, x1, y1, x2, y2);
	}

	fn drag_tool(&mut self, typ: ToolType, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.select_tool(typ);
		self.move_mouse(x1, y1);
		self.lmb_mousedown(x1, y1);
		self.move_mouse(x2, y2);
		self.mouseup(EditorMouseState {
			editor_position: (x2, y2).into(),
			mouse_keys: MouseKeys::empty(),
			scroll_delta: ScrollDelta::default(),
		});
	}

	fn move_mouse(&mut self, x: f64, y: f64) {
		let mut editor_mouse_state = EditorMouseState::new();
		editor_mouse_state.editor_position = ViewportPosition::new(x, y);
		let modifier_keys = ModifierKeys::default();
		self.input(InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys });
	}

	fn mousedown(&mut self, editor_mouse_state: EditorMouseState) {
		let modifier_keys = ModifierKeys::default();
		self.input(InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys });
	}

	fn mouseup(&mut self, editor_mouse_state: EditorMouseState) {
		let modifier_keys = ModifierKeys::default();
		self.handle_message(InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys });
	}

	fn lmb_mousedown(&mut self, x: f64, y: f64) {
		self.mousedown(EditorMouseState {
			editor_position: (x, y).into(),
			mouse_keys: MouseKeys::LEFT,
			scroll_delta: ScrollDelta::default(),
		});
	}

	fn input(&mut self, message: InputPreprocessorMessage) {
		self.handle_message(Message::InputPreprocessor(message));
	}

	fn select_tool(&mut self, tool_type: ToolType) {
		self.handle_message(Message::Tool(ToolMessage::ActivateTool { tool_type }));
	}

	fn select_primary_color(&mut self, color: Color) {
		self.handle_message(Message::Tool(ToolMessage::SelectPrimaryColor { color }));
	}
}
