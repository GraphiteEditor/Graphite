use crate::application::Editor;
use crate::application::set_uuid_seed;
use crate::messages::input_mapper::utility_types::input_keyboard::ModifierKeys;
use crate::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, MouseKeys, ScrollDelta, ViewportPosition};
use crate::messages::portfolio::utility_types::Platform;
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::ToolType;
use crate::node_graph_executor::Instrumented;
use crate::node_graph_executor::NodeRuntime;
use glam::DVec2;
use graph_craft::document::DocumentNode;
use graphene_core::InputAccessor;
use graphene_core::raster::color::Color;

/// A set of utility functions to make the writing of editor test more declarative
pub struct EditorTestUtils {
	pub editor: Editor,
	pub runtime: NodeRuntime,
}

impl EditorTestUtils {
	pub fn create() -> Self {
		let _ = env_logger::builder().is_test(true).try_init();
		set_uuid_seed(0);

		let (mut editor, runtime) = Editor::new_local_executor();

		// We have to set this directly instead of using `GlobalsMessage::SetPlatform` because race conditions with multiple tests can cause that message handler to set it more than once, which is a failure.
		// It isn't sufficient to guard the message dispatch here with a check if the once_cell is empty, because that isn't atomic and the time between checking and handling the dispatch can let multiple through.
		let _ = GLOBAL_PLATFORM.set(Platform::Windows).is_ok();

		editor.handle_message(Message::Init);

		Self { editor, runtime }
	}

	pub fn eval_graph<'a>(&'a mut self) -> impl std::future::Future<Output = Instrumented> + 'a {
		// An inner function is required since async functions in traits are a bit weird
		async fn run<'a>(editor: &'a mut Editor, runtime: &'a mut NodeRuntime) -> Instrumented {
			let portfolio = &mut editor.dispatcher.message_handlers.portfolio_message_handler;
			let exector = &mut portfolio.executor;
			let document = portfolio.documents.get_mut(&portfolio.active_document_id.unwrap()).unwrap();

			let instrumented = exector.update_node_graph_instrumented(document).expect("update_node_graph_instrumented failed");

			let viewport_resolution = glam::UVec2::ONE;
			exector
				.submit_current_node_graph_evaluation(document, viewport_resolution)
				.expect("submit_current_node_graph_evaluation failed");
			runtime.run().await;

			let mut messages = VecDeque::new();
			editor.poll_node_graph_evaluation(&mut messages).expect("Graph should render");
			let frontend_messages = messages.into_iter().flat_map(|message| editor.handle_message(message));

			for message in frontend_messages {
				message.check_node_graph_error();
			}

			instrumented
		}

		run(&mut self.editor, &mut self.runtime)
	}

	pub async fn handle_message(&mut self, message: impl Into<Message>) {
		self.editor.handle_message(message);

		// Required to process any buffered messages
		self.eval_graph().await;
	}

	pub async fn new_document(&mut self) {
		self.handle_message(Message::Portfolio(PortfolioMessage::NewDocumentWithName { name: String::from("Test document") }))
			.await;
	}

	//pub async fn draw_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
	//	self.drag_tool(ToolType::Rectangle, x1, y1, x2, y2, ModifierKeys::default()).await;
	//}

	pub async fn draw_polygon(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Polygon, x1, y1, x2, y2, ModifierKeys::default()).await;
	}

	pub async fn draw_ellipse(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Ellipse, x1, y1, x2, y2, ModifierKeys::default()).await;
	}

	pub async fn click_tool(&mut self, typ: ToolType, button: MouseKeys, position: DVec2, modifier_keys: ModifierKeys) {
		self.select_tool(typ).await;

		self.move_mouse(position.x, position.y, modifier_keys, MouseKeys::empty()).await;

		self.mousedown(
			EditorMouseState {
				editor_position: position,
				mouse_keys: button,
				..Default::default()
			},
			modifier_keys,
		)
		.await;

		self.mouseup(
			EditorMouseState {
				editor_position: position,
				..Default::default()
			},
			modifier_keys,
		)
		.await;
	}

	pub async fn drag_tool(&mut self, typ: ToolType, x1: f64, y1: f64, x2: f64, y2: f64, modifier_keys: ModifierKeys) {
		self.select_tool(typ).await;

		self.move_mouse(x1, y1, modifier_keys, MouseKeys::empty()).await;

		self.left_mousedown(x1, y1, modifier_keys).await;

		self.move_mouse(x2, y2, modifier_keys, MouseKeys::LEFT).await;

		self.mouseup(
			EditorMouseState {
				editor_position: (x2, y2).into(),
				mouse_keys: MouseKeys::empty(),
				scroll_delta: ScrollDelta::default(),
			},
			modifier_keys,
		)
		.await;
	}

	pub async fn drag_tool_cancel_rmb(&mut self, typ: ToolType) {
		self.select_tool(typ).await;

		self.move_mouse(50., 50., ModifierKeys::default(), MouseKeys::empty()).await;

		self.left_mousedown(50., 50., ModifierKeys::default()).await;

		self.move_mouse(100., 100., ModifierKeys::default(), MouseKeys::LEFT).await;

		self.mousedown(
			EditorMouseState {
				editor_position: (100., 100.).into(),
				mouse_keys: MouseKeys::LEFT | MouseKeys::RIGHT,
				scroll_delta: ScrollDelta::default(),
			},
			ModifierKeys::default(),
		)
		.await;
	}

	pub fn active_document(&self) -> &DocumentMessageHandler {
		self.editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap()
	}

	pub fn active_document_mut(&mut self) -> &mut DocumentMessageHandler {
		self.editor.dispatcher.message_handlers.portfolio_message_handler.active_document_mut().unwrap()
	}

	pub fn get_node<'a, T: InputAccessor<'a, DocumentNode>>(&'a self) -> impl Iterator<Item = T> + 'a {
		self.active_document()
			.network_interface
			.iter_recursive()
			.inspect(|node| println!("{:#?}", node.1.implementation))
			.filter_map(move |(_, document)| T::new_with_source(document))
	}

	pub async fn move_mouse(&mut self, x: f64, y: f64, modifier_keys: ModifierKeys, mouse_keys: MouseKeys) {
		let editor_mouse_state = EditorMouseState {
			editor_position: ViewportPosition::new(x, y),
			mouse_keys,
			..Default::default()
		};
		self.input(InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys }).await;
	}

	pub async fn mousedown(&mut self, editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys) {
		self.input(InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys }).await;
	}

	pub async fn mouseup(&mut self, editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys) {
		self.handle_message(InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys }).await;
	}

	pub async fn press(&mut self, key: Key, modifier_keys: ModifierKeys) {
		let key_repeat = false;

		self.handle_message(InputPreprocessorMessage::KeyDown { key, modifier_keys, key_repeat }).await;
		self.handle_message(InputPreprocessorMessage::KeyUp { key, modifier_keys, key_repeat }).await;
	}

	pub async fn left_mousedown(&mut self, x: f64, y: f64, modifier_keys: ModifierKeys) {
		self.mousedown(
			EditorMouseState {
				editor_position: (x, y).into(),
				mouse_keys: MouseKeys::LEFT,
				scroll_delta: ScrollDelta::default(),
			},
			modifier_keys,
		)
		.await;
	}

	pub async fn input(&mut self, message: InputPreprocessorMessage) {
		self.handle_message(Message::InputPreprocessor(message)).await;
	}

	pub async fn select_tool(&mut self, tool_type: ToolType) {
		self.handle_message(Message::Tool(ToolMessage::ActivateTool { tool_type })).await;
	}

	pub async fn select_primary_color(&mut self, color: Color) {
		self.handle_message(Message::Tool(ToolMessage::SelectPrimaryColor { color })).await;
	}

	pub async fn create_raster_image(&mut self, image: graphene_core::raster::Image<Color>, mouse: Option<(f64, f64)>) {
		self.handle_message(PortfolioMessage::PasteImage {
			name: None,
			image,
			mouse,
			parent_and_insert_index: None,
		})
		.await;
	}
}

pub trait FrontendMessageTestUtils {
	fn check_node_graph_error(&self);
}

impl FrontendMessageTestUtils for FrontendMessage {
	fn check_node_graph_error(&self) {
		let FrontendMessage::UpdateNodeGraph { nodes, .. } = self else { return };

		for node in nodes {
			if let Some(error) = &node.errors {
				panic!("error on {}: {}", node.display_name, error);
			}
		}
	}
}

#[cfg(test)]
pub mod test_prelude {
	pub use super::FrontendMessageTestUtils;
	pub use crate::application::Editor;
	pub use crate::float_eq;
	pub use crate::messages::input_mapper::utility_types::input_keyboard::{Key, ModifierKeys};
	pub use crate::messages::input_mapper::utility_types::input_mouse::MouseKeys;
	pub use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
	pub use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
	pub use crate::messages::prelude::*;
	pub use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, is_layer_fed_by_node_of_name};
	pub use crate::messages::tool::utility_types::ToolType;
	pub use crate::node_graph_executor::NodeRuntime;
	pub use crate::test_utils::EditorTestUtils;
	pub use core::f64;
	pub use glam::DVec2;
	pub use glam::IVec2;
	pub use graph_craft::document::DocumentNode;
	pub use graphene_core::raster::{Color, Image};
	pub use graphene_core::{InputAccessor, InputAccessorSource};
	pub use graphene_std::transform::Footprint;

	#[macro_export]
	macro_rules! float_eq {
		($left:expr, $right:expr $(,)?) => {
			match (&$left, &$right) {
				(left_val, right_val) => {
					if (*left_val - *right_val).abs() > 1e-10 {
						panic!("assertion `left == right` failed\n  left: {}\n right: {}", *left_val, *right_val)
					}
				}
			}
		};
	}
}
