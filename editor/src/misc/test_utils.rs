use crate::{
	input::{
		mouse::{MouseKeys, MouseState, ScrollDelta},
		InputPreprocessorMessage, ModifierKeys,
	},
	message_prelude::{Message, ToolMessage},
	tool::ToolType,
	Editor,
};
use graphene::color::Color;

/// A set of utility functions to make the writing of editor test more declarative
pub trait EditorTestUtils {
	fn draw_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64);
	fn draw_shape(&mut self, x1: f64, y1: f64, x2: f64, y2: f64);
	fn draw_ellipse(&mut self, x1: f64, y1: f64, x2: f64, y2: f64);

	/// Select given tool and drag it from (x1, y1) to (x2, y2)
	fn drag_tool(&mut self, typ: ToolType, x1: f64, y1: f64, x2: f64, y2: f64);
	fn move_mouse(&mut self, x: f64, y: f64);
	fn mousedown(&mut self, state: MouseState);
	fn mouseup(&mut self, state: MouseState);
	fn lmb_mousedown(&mut self, x: f64, y: f64);
	fn input(&mut self, message: InputPreprocessorMessage);
	fn select_tool(&mut self, typ: ToolType);
	fn select_primary_color(&mut self, color: Color);
}

impl EditorTestUtils for Editor {
	fn draw_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Rectangle, x1, y1, x2, y2);
	}

	fn draw_shape(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Shape, x1, y1, x2, y2);
	}

	fn draw_ellipse(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Ellipse, x1, y1, x2, y2);
	}

	fn drag_tool(&mut self, typ: ToolType, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.select_tool(typ);
		self.move_mouse(x1, y1);
		self.lmb_mousedown(x1, y1);
		self.move_mouse(x2, y2);
		self.mouseup(MouseState {
			position: (x2, y2).into(),
			mouse_keys: MouseKeys::empty(),
			scroll_delta: ScrollDelta::default(),
		});
	}

	fn move_mouse(&mut self, x: f64, y: f64) {
		self.input(InputPreprocessorMessage::MouseMove((x, y).into(), ModifierKeys::default()));
	}

	fn mousedown(&mut self, state: MouseState) {
		self.input(InputPreprocessorMessage::MouseDown(state, ModifierKeys::default()));
	}

	fn mouseup(&mut self, state: MouseState) {
		self.handle_message(InputPreprocessorMessage::MouseUp(state, ModifierKeys::default())).unwrap()
	}

	fn lmb_mousedown(&mut self, x: f64, y: f64) {
		self.mousedown(MouseState {
			position: (x, y).into(),
			mouse_keys: MouseKeys::LEFT,
			scroll_delta: ScrollDelta::default(),
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
