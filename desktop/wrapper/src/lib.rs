use graph_craft::wasm_application_io::WasmApplicationIo;
use graphite_editor::application::Editor;
use graphite_editor::messages::prelude::{FrontendMessage, Message};

// TODO: Remove usage of this reexport in desktop create and remove this line
pub use graphene_std::Color;

pub use wgpu_executor::WgpuContext;
pub use wgpu_executor::WgpuContextBuilder;
pub use wgpu_executor::WgpuExecutor;
pub use wgpu_executor::WgpuFeatures;

pub mod messages;
use messages::{DesktopFrontendMessage, DesktopWrapperMessage};

mod message_dispatcher;
use message_dispatcher::DesktopWrapperMessageDispatcher;

mod handle_desktop_wrapper_message;
mod intercept_editor_message;
mod intercept_frontend_message;

pub struct DesktopWrapper {
	editor: Editor,
}

impl DesktopWrapper {
	pub fn new() -> Self {
		Self { editor: Editor::new() }
	}

	pub fn init(&self, wgpu_context: WgpuContext) {
		let application_io = WasmApplicationIo::new_with_context(wgpu_context);
		futures::executor::block_on(graphite_editor::node_graph_executor::replace_application_io(application_io));
	}

	pub fn dispatch(&mut self, message: DesktopWrapperMessage) -> Vec<DesktopFrontendMessage> {
		let mut executor = DesktopWrapperMessageDispatcher::new(&mut self.editor);
		executor.queue_desktop_wrapper_message(message);
		executor.execute()
	}

	pub async fn execute_node_graph() -> NodeGraphExecutionResult {
		let result = graphite_editor::node_graph_executor::run_node_graph().await;
		match result {
			(true, texture) => NodeGraphExecutionResult::HasRun(texture.map(|t| t.texture)),
			(false, _) => NodeGraphExecutionResult::NotRun,
		}
	}
}

impl Default for DesktopWrapper {
	fn default() -> Self {
		Self::new()
	}
}

pub enum NodeGraphExecutionResult {
	HasRun(Option<wgpu::Texture>),
	NotRun,
}

pub fn deserialize_editor_message(data: &[u8]) -> Option<DesktopWrapperMessage> {
	if let Ok(string) = std::str::from_utf8(data) {
		if let Ok(message) = ron::de::from_str::<Message>(string) {
			Some(DesktopWrapperMessage::FromWeb(message.into()))
		} else {
			None
		}
	} else {
		None
	}
}

pub fn serialize_frontend_messages(messages: Vec<FrontendMessage>) -> Option<Vec<u8>> {
	if let Ok(serialized) = ron::ser::to_string(&messages) {
		Some(serialized.into_bytes())
	} else {
		None
	}
}
