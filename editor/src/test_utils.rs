use crate::application::{DEFAULT_ENVIRONMENT, Editor};
use crate::builder::{Builder, EditorBuilder, RuntimeBuilder};
use crate::messages::frontend::utility_types::RenderStatus;
use crate::messages::input_mapper::utility_types::keyboard::ModifierKeys;
use crate::messages::input_mapper::utility_types::pointer::{EditorPointerState, MouseKeys, ViewportPosition};
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::ingest::utility_types::IngestAction;
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::ToolType;
use crate::node_graph_executor::Instrumented;
use crate::node_graph_executor::{EvaluationErrors, NodeRuntime};
use crate::task::{Spawn, Task};
use crate::test_utils::test_prelude::LayerNodeIdentifier;
use document_container::store::MemoryStore;
use glam::{DVec2, UVec2};
use graph_craft::application_io::PlatformApplicationIo;
use graph_craft::application_io::resource::HashMapResourceStorage;
use graphene_std::raster::color::Color;
use graphene_std::uuid::NodeId;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A set of utility functions to make the writing of editor test more declarative
pub struct EditorTestUtils {
	pub editor: Editor,
	pub runtime: NodeRuntime,
	woken: Arc<AtomicBool>,
}

impl EditorTestUtils {
	pub fn create() -> Self {
		init_test_logger();

		let woken = Arc::new(AtomicBool::new(false));
		let wake = woken.clone();
		let (editor, runtime) = Builder::new()
			.with_editor(
				EditorBuilder::new()
					.with_wake(move || wake.store(true, Ordering::Relaxed))
					.with_environment(DEFAULT_ENVIRONMENT)
					.with_resource_storage(Arc::new(HashMapResourceStorage::new()))
					.with_document_store(Arc::new(MemoryStore::default())),
			)
			.with_runtime(RuntimeBuilder::new().with_wake(|| {}).with_application_io(PlatformApplicationIo::default()))
			.with_spawner(BlockingSpawner)
			.with_random_seed(0)
			.build();
		let mut editor = Self { editor, runtime, woken };

		editor.dispatch(PortfolioMessage::Init);

		// Tests run with `.gdd` storage on and the dual-write soak validation enabled, so storage round-trip drift fails loud.
		editor.dispatch(PreferencesMessage::SaveAsGdd { enabled: true });
		editor.dispatch(PreferencesMessage::ValidateStorageRoundTrip { enabled: true });

		editor
	}

	pub fn dispatch(&mut self, message: impl Into<Message>) -> Vec<FrontendMessage> {
		let mut responses = self.editor.handle_message(message);
		while self.woken.swap(false, Ordering::Relaxed) {
			responses.extend(self.editor.handle_message(Message::Wake));
		}
		responses
	}

	/// Submit an evaluation to the executor, run the runtime, and process the results.
	///
	/// If a deferred message triggers a second graph run, this will not occur. See [`Self::eval_graph_until_finished`] for this behaviour.
	async fn eval_graph_once(&mut self) -> Result<Instrumented, TestGraphError> {
		let portfolio = &mut self.portfolio_message_handler_mut();
		let document_id = portfolio.active_document_id.unwrap();
		let (executor, documents) = (&mut portfolio.executor, &mut portfolio.documents);
		let document = documents.get_mut(&document_id).unwrap();

		let instrumented = match executor.update_node_graph_instrumented(document) {
			Ok(instrumented) => instrumented,
			Err(e) => return Err(TestGraphError::Other(format!("update_node_graph_instrumented failed\n\n{e}"))),
		};

		if let Err(e) = executor.submit_current_node_graph_evaluation(document, document_id, UVec2::ONE, 1., Default::default(), DVec2::ZERO) {
			return Err(TestGraphError::Other(format!("submit_current_node_graph_evaluation failed\n\n{e}")));
		}
		self.runtime.run().await;

		let frontend_messages = self.dispatch(Message::Wake);
		let evaluation_errors = self.portfolio_message_handler_mut().executor.take_evaluation_errors();
		if evaluation_errors.is_none() && frontend_messages.contains(&FrontendMessage::UpdateRenderStatus { status: RenderStatus::Error }) {
			return Err(TestGraphError::Render);
		}
		#[cfg(feature = "gpu")]
		let evaluation_errors = evaluation_errors.filter(|errors| !errors.iter().all(|(error, _)| error.is::<wgpu_executor::NoGpuError>()));
		if let Some(errors) = evaluation_errors {
			return Err(TestGraphError::Evaluation(errors));
		}

		for message in frontend_messages {
			message.check_node_graph_error();
		}

		Ok(instrumented)
	}

	/// Repeatedly calls [`Self::eval_graph_once`] until no more runs are triggered by deferred messages.
	pub async fn eval_graph_until_finished(&mut self) -> Result<Instrumented, TestGraphError> {
		const MAX_ITERATIONS: usize = 16;
		for _iteration in 0..MAX_ITERATIONS {
			let execution_id = self.portfolio_message_handler().executor.current_execution_id();
			let result = self.eval_graph_once().await?;
			// If there have been no other executions queued (other than the one we just queued ourselves above) then finish
			if execution_id + 1 >= self.portfolio_message_handler().executor.current_execution_id() {
				return Ok(result);
			}
		}
		return Err(TestGraphError::Other(format!("eval_graph_until_finished exceeded {MAX_ITERATIONS} graph executions; stopping")));
	}

	pub async fn handle_message(&mut self, message: impl Into<Message>) -> Vec<FrontendMessage> {
		let frontend_messages_from_msg = self.dispatch(message);

		// Required to process any buffered messages
		if let Err(e) = self.eval_graph_until_finished().await {
			panic!("Failed to evaluate graph: {e}");
		}

		// Sweep the network interface's structural invariants so any desync fails at the message that caused it
		let violations = self.active_document().network_interface.validate_invariants();
		assert!(violations.is_empty(), "Network interface invariant violations:\n{}", violations.join("\n"));

		frontend_messages_from_msg
	}

	pub async fn new_document(&mut self) {
		self.handle_message(Message::Portfolio(PortfolioMessage::NewDocumentWithName { name: String::from("Test document") }))
			.await;
	}

	pub async fn draw_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Rectangle, x1, y1, x2, y2, ModifierKeys::default()).await;
	}

	pub async fn draw_polygon(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Shape, x1, y1, x2, y2, ModifierKeys::default()).await;
	}

	pub async fn draw_ellipse(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
		self.drag_tool(ToolType::Ellipse, x1, y1, x2, y2, ModifierKeys::default()).await;
	}

	pub async fn click_tool(&mut self, typ: ToolType, button: MouseKeys, position: DVec2, modifier_keys: ModifierKeys) {
		self.select_tool(typ).await;

		self.move_mouse(position.x, position.y, modifier_keys, MouseKeys::empty()).await;

		self.mousedown(
			EditorPointerState {
				editor_position: position,
				mouse_keys: button,
				..Default::default()
			},
			modifier_keys,
		)
		.await;

		self.mouseup(
			EditorPointerState {
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
			EditorPointerState {
				editor_position: (x2, y2).into(),
				mouse_keys: MouseKeys::empty(),
				..Default::default()
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
			EditorPointerState {
				editor_position: (100., 100.).into(),
				mouse_keys: MouseKeys::LEFT | MouseKeys::RIGHT,
				..Default::default()
			},
			ModifierKeys::default(),
		)
		.await;
	}

	pub fn portfolio_message_handler(&self) -> &PortfolioMessageHandler {
		&self.editor.dispatcher.message_handlers.portfolio_message_handler
	}

	pub fn portfolio_message_handler_mut(&mut self) -> &mut PortfolioMessageHandler {
		&mut self.editor.dispatcher.message_handlers.portfolio_message_handler
	}

	pub fn active_document(&self) -> &DocumentMessageHandler {
		self.portfolio_message_handler().active_document().unwrap()
	}

	pub fn active_document_mut(&mut self) -> &mut DocumentMessageHandler {
		self.portfolio_message_handler_mut().active_document_mut().unwrap()
	}

	pub async fn move_mouse(&mut self, x: f64, y: f64, modifier_keys: ModifierKeys, mouse_keys: MouseKeys) {
		let editor_mouse_state = EditorPointerState {
			editor_position: ViewportPosition::new(x, y),
			mouse_keys,
			..Default::default()
		};
		self.input(InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys }).await;
	}

	pub async fn mousedown(&mut self, editor_mouse_state: EditorPointerState, modifier_keys: ModifierKeys) {
		self.input(InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys }).await;
	}

	pub async fn mouseup(&mut self, editor_mouse_state: EditorPointerState, modifier_keys: ModifierKeys) {
		self.handle_message(InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys }).await;
	}

	pub async fn press(&mut self, key: Key, modifier_keys: ModifierKeys) {
		self.key_down(key, modifier_keys).await;
		self.key_up(key, modifier_keys).await;
	}

	/// Puts a key down and leaves it held until `key_up`.
	pub async fn key_down(&mut self, key: Key, modifier_keys: ModifierKeys) {
		let key_repeat = false;
		self.handle_message(InputPreprocessorMessage::KeyDown { key, modifier_keys, key_repeat }).await;
	}

	pub async fn key_up(&mut self, key: Key, modifier_keys: ModifierKeys) {
		let key_repeat = false;
		self.handle_message(InputPreprocessorMessage::KeyUp { key, modifier_keys, key_repeat }).await;
	}

	/// Sets the clock to a time in milliseconds and runs one animation frame, as the frontend does on each `requestAnimationFrame`.
	pub async fn animation_frame(&mut self, timestamp: u64) {
		let messages = Box::new([
			InputPreprocessorMessage::CurrentTime { timestamp }.into(),
			AnimationMessage::IncrementFrameCounter.into(),
			BroadcastMessage::TriggerEvent(EventMessage::AnimationFrame).into(),
		]);
		self.handle_message(Message::Batched { messages }).await;
	}

	pub async fn left_mousedown(&mut self, x: f64, y: f64, modifier_keys: ModifierKeys) {
		self.mousedown(
			EditorPointerState {
				editor_position: (x, y).into(),
				mouse_keys: MouseKeys::LEFT,
				..Default::default()
			},
			modifier_keys,
		)
		.await;
	}

	pub async fn left_mouseup(&mut self, x: f64, y: f64, modifier_keys: ModifierKeys) {
		self.mouseup(
			EditorPointerState {
				editor_position: (x, y).into(),
				mouse_keys: MouseKeys::empty(),
				..Default::default()
			},
			modifier_keys,
		)
		.await;
	}

	pub async fn input(&mut self, message: InputPreprocessorMessage) {
		self.handle_message(Message::InputPreprocessor(message)).await;
	}

	pub async fn select_tool(&mut self, tool_type: ToolType) {
		match tool_type {
			ToolType::Line => self.handle_message(Message::Tool(ToolMessage::ActivateToolShapeLine)).await,
			ToolType::Rectangle => self.handle_message(Message::Tool(ToolMessage::ActivateToolShapeRectangle)).await,
			ToolType::Ellipse => self.handle_message(Message::Tool(ToolMessage::ActivateToolShapeEllipse)).await,
			_ => self.handle_message(Message::Tool(ToolMessage::ActivateTool { tool_type })).await,
		};
	}

	pub async fn select_primary_color(&mut self, color: Color) {
		self.handle_message(Message::Tool(ToolMessage::SelectWorkingColor { color, primary: true })).await;
	}

	pub async fn select_secondary_color(&mut self, color: Color) {
		self.handle_message(Message::Tool(ToolMessage::SelectWorkingColor { color, primary: false })).await;
	}

	pub async fn create_raster_image(&mut self, image: graphene_std::raster::Image<Color>, mouse: Option<(f64, f64)>) {
		self.handle_message(IngestMessage::Ingest {
			data: image.to_png(),
			action: mouse.map_or(IngestAction::Paste, |mouse| IngestAction::DropOnCanvas { mouse }),
			mime_type: String::new(),
			path: None,
		})
		.await;
	}
	pub async fn draw_spline(&mut self, points: &[DVec2]) {
		self.select_tool(ToolType::Spline).await;

		for &point in points {
			self.click_tool(ToolType::Spline, MouseKeys::LEFT, point, ModifierKeys::empty()).await;
		}

		self.press(Key::Enter, ModifierKeys::empty()).await;
	}

	pub async fn get_selected_layer(&mut self) -> Option<LayerNodeIdentifier> {
		self.active_document().network_interface.selected_nodes().selected_layers(self.active_document().metadata()).next()
	}

	pub async fn double_click(&mut self, position: DVec2) {
		self.handle_message(InputPreprocessorMessage::DoubleClick {
			editor_mouse_state: EditorPointerState {
				editor_position: position,
				mouse_keys: MouseKeys::LEFT,
				..Default::default()
			},
			modifier_keys: ModifierKeys::empty(),
		})
		.await;
	}

	pub async fn drag_path(&mut self, points: &[DVec2], modifier_keys: ModifierKeys) {
		if points.is_empty() {
			return;
		}

		let first_point = points[0];
		self.move_mouse(first_point.x, first_point.y, modifier_keys, MouseKeys::empty()).await;
		self.left_mousedown(first_point.x, first_point.y, modifier_keys).await;

		for &point in &points[1..] {
			self.move_mouse(point.x, point.y, modifier_keys, MouseKeys::LEFT).await;
		}

		self.mouseup(
			EditorPointerState {
				editor_position: points[points.len() - 1],
				mouse_keys: MouseKeys::empty(),
				..Default::default()
			},
			modifier_keys,
		)
		.await;
	}

	/// Necessary for doing snapping since snaps outside of the viewport are discarded
	pub async fn set_viewport_size(&mut self, top_left: DVec2, bottom_right: DVec2) {
		self.handle_message(ViewportMessage::Update {
			x: top_left.x,
			y: top_left.y,
			width: bottom_right.x - top_left.x,
			height: bottom_right.y - top_left.y,
			scale: 1.,
		})
		.await;
	}

	pub async fn create_node_by_name(&mut self, node_type: DefinitionIdentifier) -> NodeId {
		self.create_node(node_type, None).await
	}

	pub async fn create_node_by_name_at(&mut self, node_type: DefinitionIdentifier, x: i32, y: i32) -> NodeId {
		self.create_node(node_type, Some((x, y))).await
	}

	async fn create_node(&mut self, node_type: DefinitionIdentifier, xy: Option<(i32, i32)>) -> NodeId {
		let node_id = NodeId::new();
		self.handle_message(NodeGraphMessage::CreateNodeFromContextMenu {
			node_id: Some(node_id),
			node_type,
			xy,
			add_transaction: true,
		})
		.await;
		node_id
	}
}

pub trait FrontendMessageTestUtils {
	fn check_node_graph_error(&self);
}

impl FrontendMessageTestUtils for FrontendMessage {
	fn check_node_graph_error(&self) {
		let FrontendMessage::UpdateNodeGraphErrorDiagnostics { errors } = self else { return };
		if !errors.is_empty() {
			panic!("errors: {errors:?}");
		}
	}
}

#[derive(Debug)]
pub enum TestGraphError {
	Evaluation(EvaluationErrors),
	Render,
	Other(String),
}

impl std::fmt::Display for TestGraphError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Evaluation(errors) => write!(f, "Graph evaluation failed:\n{errors}"),
			Self::Render => write!(f, "Graph should render"),
			Self::Other(message) => write!(f, "{message}"),
		}
	}
}

thread_local! {
	static CAPTURED_ERRORS: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// Prints like `env_logger`, and also records the errors logged on a thread while [`capture_logged_errors`] runs on it.
struct TestLogger {
	printer: env_logger::Logger,
}

impl log::Log for TestLogger {
	fn enabled(&self, metadata: &log::Metadata) -> bool {
		metadata.level() == log::Level::Error || self.printer.enabled(metadata)
	}

	fn log(&self, record: &log::Record) {
		if record.level() == log::Level::Error {
			CAPTURED_ERRORS.with_borrow_mut(|captured| {
				if let Some(captured) = captured {
					captured.push(record.args().to_string());
				}
			});
		}

		self.printer.log(record);
	}

	fn flush(&self) {
		self.printer.flush();
	}
}

/// Installs the logger shared by every test in the process, since only one can be installed.
fn init_test_logger() {
	static INSTALL: std::sync::Once = std::sync::Once::new();
	INSTALL.call_once(|| {
		let printer = env_logger::builder().is_test(true).build();
		let max_level = printer.filter().max(log::LevelFilter::Error);
		if log::set_boxed_logger(Box::new(TestLogger { printer })).is_ok() {
			log::set_max_level(max_level);
		}
	});
}

/// Runs `operation`, returning its result along with the errors it logged.
pub fn capture_logged_errors<T>(operation: impl FnOnce() -> T) -> (T, Vec<String>) {
	init_test_logger();

	CAPTURED_ERRORS.set(Some(Vec::new()));
	let result = operation();
	let errors = CAPTURED_ERRORS.take().unwrap_or_default();

	(result, errors)
}

struct BlockingSpawner;
impl Spawn for BlockingSpawner {
	fn spawn(&self, task: Task) {
		futures::executor::block_on(task);
	}
}

#[cfg(test)]
pub mod test_prelude {
	pub use super::FrontendMessageTestUtils;
	pub use crate::application::Editor;
	pub use crate::float_eq;
	pub use crate::messages::clipboard::utility_types::ClipboardContentRaw;
	pub use crate::messages::input_mapper::utility_types::keyboard::{Key, ModifierKeys};
	pub use crate::messages::input_mapper::utility_types::pointer::MouseKeys;
	pub use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
	pub use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
	pub use crate::messages::prelude::*;
	pub use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, is_layer_fed_by_node_of_name};
	pub use crate::messages::tool::utility_types::ToolType;
	pub use crate::node_graph_executor::NodeRuntime;
	pub use crate::test_utils::{EditorTestUtils, TestGraphError};
	pub use core::f64;
	pub use glam::{DVec2, IVec2};
	pub use graph_craft::document::DocumentNode;
	pub use graphene_std::raster::{Color, Image};
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
