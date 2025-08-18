use graph_craft::wasm_application_io::WasmApplicationIo;
use graphite_editor::application::Editor;

pub use wgpu_executor::Context as WgpuContext;

pub mod messages;
use messages::{DesktopFrontendMessage, DesktopWrapperMessage};

mod message_executor;
use message_executor::DesktopWrapperMessageExecutor;

mod handle_desktop_wrapper_message;
mod intercept_frontend_message;
mod intercept_message;

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
		let mut executor = DesktopWrapperMessageExecutor::new(&mut self.editor);
		executor.queue(message);
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

pub enum NodeGraphExecutionResult {
	HasRun(Option<wgpu::Texture>),
	NotRun,
}
