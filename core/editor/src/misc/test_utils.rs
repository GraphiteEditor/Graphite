use crate::{
	input::{
		mouse::{MouseKeys, MouseState, ViewportPosition},
		InputPreprocessorMessage,
	},
	message_prelude::{Message, ToolMessage},
	tool::ToolType,
	Editor,
};
use document_core::color::Color;

/// A set of utility functions to make the writing of editor test more declarative
pub trait EditorTestUtils {
	fn draw_rect(&mut self, x1: u32, y1: u32, x2: u32, y2: u32);
	fn draw_shape(&mut self, x1: u32, y1: u32, x2: u32, y2: u32);
	fn draw_ellipse(&mut self, x1: u32, y1: u32, x2: u32, y2: u32);

	/// Select given tool and drag it from (x1, y1) to (x2, y2)
	fn drag_tool(&mut self, typ: ToolType, x1: u32, y1: u32, x2: u32, y2: u32);
	fn move_mouse(&mut self, x: u32, y: u32);
	fn mousedown(&mut self, state: MouseState);
	fn mouseup(&mut self, x: u32, y: u32);
	fn left_mousedown(&mut self, x: u32, y: u32);
	fn input(&mut self, message: InputPreprocessorMessage);
	fn select_tool(&mut self, typ: ToolType);
	fn select_primary_color(&mut self, color: Color);
}

impl EditorTestUtils for Editor {
	fn draw_rect(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) {
		self.drag_tool(ToolType::Rectangle, x1, y1, x2, y2);
	}

	fn draw_shape(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) {
		self.drag_tool(ToolType::Shape, x1, y1, x2, y2);
	}

	fn draw_ellipse(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) {
		self.drag_tool(ToolType::Ellipse, x1, y1, x2, y2);
	}

	fn drag_tool(&mut self, typ: ToolType, x1: u32, y1: u32, x2: u32, y2: u32) {
		self.select_tool(typ);
		self.move_mouse(x1, y1);
		self.left_mousedown(x1, y1);
		self.move_mouse(x2, y2);
		self.mouseup(x2, y2);
	}

	fn move_mouse(&mut self, x: u32, y: u32) {
		self.input(InputPreprocessorMessage::MouseMove(ViewportPosition { x, y }));
	}

	fn mousedown(&mut self, state: MouseState) {
		self.input(InputPreprocessorMessage::MouseDown(state));
	}

	fn mouseup(&mut self, x: u32, y: u32) {
		self.handle_message(InputPreprocessorMessage::MouseUp(MouseState {
			position: ViewportPosition { x, y },
			mouse_keys: MouseKeys::empty(),
		}))
		.unwrap()
	}

	fn left_mousedown(&mut self, x: u32, y: u32) {
		self.mousedown(MouseState {
			position: ViewportPosition { x, y },
			mouse_keys: MouseKeys::LEFT,
		})
	}

	fn input(&mut self, message: InputPreprocessorMessage) {
		self.handle_message(Message::InputPreprocessor(message)).unwrap();
	}

	fn select_tool(&mut self, typ: ToolType) {
		self.handle_message(Message::Tool(ToolMessage::SelectTool(typ))).unwrap();
	}

	fn select_primary_color(&mut self, color: Color) {
		self.handle_message(Message::Tool(ToolMessage::SelectPrimaryColor(color))).unwrap();
	}
}
