use crate::{frontend::FrontendMessageHandler, message_prelude::*, Callback, EditorError};

pub use crate::document::DocumentMessageHandler;
pub use crate::input::{InputMapper, InputPreprocessor};
pub use crate::tool::ToolMessageHandler;

use crate::global::GlobalMessageHandler;
use std::collections::VecDeque;

pub struct Dispatcher {
	frontend_message_handler: FrontendMessageHandler,
	input_preprocessor: InputPreprocessor,
	input_mapper: InputMapper,
	global_message_handler: GlobalMessageHandler,
	tool_message_handler: ToolMessageHandler,
	document_message_handler: DocumentMessageHandler,
	messages: VecDeque<Message>,
}

impl Dispatcher {
	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Result<(), EditorError> {
		let message = message.into();
		use Message::*;
		if !(matches!(
			message,
			Message::InputPreprocessor(_)
				| Message::InputMapper(_)
				| Message::Document(DocumentMessage::RenderDocument)
				| Message::Frontend(FrontendMessage::UpdateCanvas { .. })
				| Message::Document(DocumentMessage::DispatchOperation { .. })
		) || MessageDiscriminant::from(&message).local_name().ends_with("MouseMove"))
		{
			log::trace!("Message: {}", message.to_discriminant().local_name());
		}
		match message {
			NoOp => (),
			Document(message) => self.document_message_handler.process_action(message, &self.input_preprocessor, &mut self.messages),
			Global(message) => self.global_message_handler.process_action(message, (), &mut self.messages),
			Tool(message) => self
				.tool_message_handler
				.process_action(message, (&self.document_message_handler.active_document().document, &self.input_preprocessor), &mut self.messages),
			Frontend(message) => self.frontend_message_handler.process_action(message, (), &mut self.messages),
			InputPreprocessor(message) => self.input_preprocessor.process_action(message, (), &mut self.messages),
			InputMapper(message) => {
				let actions = self.collect_actions();
				self.input_mapper.process_action(message, (&self.input_preprocessor, actions), &mut self.messages)
			}
		}
		if let Some(message) = self.messages.pop_front() {
			self.handle_message(message)?;
		}
		Ok(())
	}

	pub fn collect_actions(&self) -> ActionList {
		//TODO: reduce the number of heap allocations
		let mut list = Vec::new();
		list.extend(self.frontend_message_handler.actions());
		list.extend(self.input_preprocessor.actions());
		list.extend(self.input_mapper.actions());
		list.extend(self.global_message_handler.actions());
		list.extend(self.tool_message_handler.actions());
		list.extend(self.document_message_handler.actions());
		list
	}

	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher {
			frontend_message_handler: FrontendMessageHandler::new(callback),
			input_preprocessor: InputPreprocessor::default(),
			global_message_handler: GlobalMessageHandler::new(),
			input_mapper: InputMapper::default(),
			document_message_handler: DocumentMessageHandler::default(),
			tool_message_handler: ToolMessageHandler::default(),
			messages: VecDeque::new(),
		}
	}
}

#[cfg(test)]
mod test {
	use crate::{
		input::{
			mouse::{MouseKeys, MouseState, ViewportPosition},
			InputPreprocessorMessage,
		},
		message_prelude::{DocumentMessage, Message, ToolMessage},
		tool::ToolType,
		Editor,
	};
	use document_core::color::Color;
	use log::info;

	fn init_logger() {
		let _ = env_logger::builder().is_test(true).try_init();
	}

	/// A set of utility functions to make the writing of editor test more declarative
	trait EditorTestUtils {
		fn draw_rect(&mut self, x1: u32, y1: u32, x2: u32, y2: u32);
		fn draw_shape(&mut self, x1: u32, y1: u32, x2: u32, y2: u32);
		fn draw_ellipse(&mut self, x1: u32, y1: u32, x2: u32, y2: u32);

		/// Select given tool and drag it from (x1, y1) to (x2, y2)
		fn drag_tool(&mut self, typ: ToolType, x1: u32, y1: u32, x2: u32, y2: u32);
		fn move_mouse(&mut self, x: u32, y: u32);
		fn mousedown(&mut self, state: MouseState);
		fn mouseup(&mut self, state: MouseState);
		fn left_mousedown(&mut self, x: u32, y: u32);
		fn left_mouseup(&mut self, x: u32, y: u32);
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
			self.left_mouseup(x2, y2);
		}

		fn move_mouse(&mut self, x: u32, y: u32) {
			self.input(InputPreprocessorMessage::MouseMove(ViewportPosition { x, y }));
		}

		fn mousedown(&mut self, state: MouseState) {
			self.input(InputPreprocessorMessage::MouseDown(state));
		}

		fn mouseup(&mut self, state: MouseState) {
			self.input(InputPreprocessorMessage::MouseUp(state));
		}

		fn left_mousedown(&mut self, x: u32, y: u32) {
			self.mousedown(MouseState {
				position: ViewportPosition { x, y },
				mouse_keys: MouseKeys::LEFT,
			})
		}

		fn left_mouseup(&mut self, x: u32, y: u32) {
			self.mouseup(MouseState {
				position: ViewportPosition { x, y },
				mouse_keys: MouseKeys::empty(),
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

	/// Create an editor instance with three layers
	/// 1. A red rectangle
	/// 2. A blue shape
	/// 3. A green ellipse
	fn create_editor_with_three_layers() -> Editor {
		let mut editor = Editor::new(Box::new(|e| {
			info!("Got frontend message: {:?}", e);
		}));

		editor.select_primary_color(Color::RED);
		editor.draw_rect(100, 200, 300, 400);
		editor.select_primary_color(Color::BLUE);
		editor.draw_shape(10, 1200, 1300, 400);
		editor.select_primary_color(Color::GREEN);
		editor.draw_ellipse(104, 1200, 1300, 400);

		editor
	}

	#[test]
	/// - create rect, shape and ellipse
	/// - copy
	/// - paste
	/// - assert that ellipse was copied
	fn copy_paste_single_layer() {
		init_logger();
		let mut editor = create_editor_with_three_layers();

		let document_before_copy = editor.dispatcher.document_message_handler.active_document().document.clone();
		editor.handle_message(Message::Document(DocumentMessage::CopySelectedLayers)).unwrap();
		editor.handle_message(Message::Document(DocumentMessage::PasteLayers)).unwrap();
		let document_after_copy = editor.dispatcher.document_message_handler.active_document().document.clone();

		let layers_before_copy = document_before_copy.root.layers();
		let layers_after_copy = document_after_copy.root.layers();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i]);
		}

		// The ellipse was copied
		assert_eq!(layers_before_copy[2], layers_after_copy[3]);
	}

	#[test]
	/// - create rect, shape and ellipse
	/// - select shape
	/// - copy
	/// - paste
	/// - assert that shape was copied
	fn copy_paste_single_layer_from_middle() {
		init_logger();
		let mut editor = create_editor_with_three_layers();

		let document_before_copy = editor.dispatcher.document_message_handler.active_document().document.clone();
		let shape_id = document_before_copy.root.layer_ids[1];

		editor.handle_message(Message::Document(DocumentMessage::SelectLayers(vec![vec![shape_id]]))).unwrap();
		editor.handle_message(Message::Document(DocumentMessage::CopySelectedLayers)).unwrap();
		editor.handle_message(Message::Document(DocumentMessage::PasteLayers)).unwrap();

		let document_after_copy = editor.dispatcher.document_message_handler.active_document().document.clone();

		let layers_before_copy = document_before_copy.root.layers();
		let layers_after_copy = document_after_copy.root.layers();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i]);
		}

		// The shape was copied
		assert_eq!(layers_before_copy[1], layers_after_copy[3]);
	}

	#[test]
	/// - create rect, shape and ellipse
	/// - select ellipse and rect
	/// - copy
	/// - delete
	/// - create another rect
	/// - paste
	/// - paste
	fn copy_paste_deleted_layers() {
		init_logger();
		let mut editor = create_editor_with_three_layers();

		const ELLIPSE_INDEX: usize = 2;
		const SHAPE_INDEX: usize = 1;
		const RECT_INDEX: usize = 0;

		let document_before_copy = editor.dispatcher.document_message_handler.active_document().document.clone();
		let rect_id = document_before_copy.root.layer_ids[RECT_INDEX];
		let ellipse_id = document_before_copy.root.layer_ids[ELLIPSE_INDEX];

		editor.handle_message(Message::Document(DocumentMessage::SelectLayers(vec![vec![rect_id], vec![ellipse_id]]))).unwrap();
		editor.handle_message(Message::Document(DocumentMessage::CopySelectedLayers)).unwrap();
		editor.handle_message(Message::Document(DocumentMessage::DeleteSelectedLayers)).unwrap();
		editor.draw_rect(0, 800, 12, 200);
		editor.handle_message(Message::Document(DocumentMessage::PasteLayers)).unwrap();
		editor.handle_message(Message::Document(DocumentMessage::PasteLayers)).unwrap();

		let document_after_copy = editor.dispatcher.document_message_handler.active_document().document.clone();

		let layers_before_copy = document_before_copy.root.layers();
		let layers_after_copy = document_after_copy.root.layers();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 6);

		let rect_before_copy = &layers_before_copy[RECT_INDEX];
		let ellipse_before_copy = &layers_before_copy[ELLIPSE_INDEX];

		assert_eq!(layers_after_copy[0], layers_before_copy[SHAPE_INDEX]);
		assert_eq!(&layers_after_copy[2], rect_before_copy);
		assert_eq!(&layers_after_copy[3], ellipse_before_copy);
		assert_eq!(&layers_after_copy[4], rect_before_copy);
		assert_eq!(&layers_after_copy[5], ellipse_before_copy);
	}
}
