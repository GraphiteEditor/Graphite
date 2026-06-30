use graph_craft::application_io::PlatformApplicationIo;
use graph_craft::application_io::resource::ResourceStorage;
use graphite_editor::application::{Editor, Environment, Host, Platform};
use graphite_editor::messages::prelude::{FrontendMessage, Message, Wake};
use message_dispatcher::DesktopWrapperMessageDispatcher;
use messages::{DesktopFrontendMessage, DesktopWrapperMessage};
use std::sync::Arc;

pub use graph_craft::application_io::resource::MmapResourceStorage;
pub use graphite_editor::consts::{DOUBLE_CLICK_MILLISECONDS, FILE_EXTENSION};
pub use wgpu_executor::WgpuBackends;
pub use wgpu_executor::WgpuContext;
pub use wgpu_executor::WgpuContextBuilder;
pub use wgpu_executor::WgpuExecutor;
pub use wgpu_executor::WgpuFeatures;

mod handle_desktop_wrapper_message;
mod intercept_editor_message;
mod intercept_frontend_message;
mod message_dispatcher;
pub mod messages;
pub(crate) mod utils;

pub struct DesktopWrapper {
	editor: Editor,
}

impl DesktopWrapper {
	pub fn new(uuid_random_seed: u64, resource_storage: Arc<dyn ResourceStorage>, working_copy_root: std::path::PathBuf, wgpu_context: WgpuContext, schedule_wake: Wake) -> Self {
		#[cfg(target_os = "windows")]
		let host = Host::Windows;
		#[cfg(target_os = "macos")]
		let host = Host::Mac;
		#[cfg(target_os = "linux")]
		let host = Host::Linux;
		let env = Environment { platform: Platform::Desktop, host };
		let application_io = PlatformApplicationIo::new_with_context(wgpu_context);

		Self {
			editor: Editor::new(env, uuid_random_seed, resource_storage, Some(working_copy_root), application_io, schedule_wake),
		}
	}

	pub fn dispatch(&mut self, message: DesktopWrapperMessage) -> Vec<DesktopFrontendMessage> {
		let mut executor = DesktopWrapperMessageDispatcher::new(&mut self.editor);
		executor.queue_desktop_wrapper_message(message);
		executor.execute()
	}

	pub async fn execute_node_graph() -> NodeGraphExecutionResult {
		let result = graphite_editor::node_graph_executor::run_node_graph().await;
		match result {
			(true, texture) => NodeGraphExecutionResult::HasRun(texture.map(Into::into)),
			(false, _) => NodeGraphExecutionResult::NotRun,
		}
	}
}

pub enum NodeGraphExecutionResult {
	HasRun(Option<std::sync::Arc<wgpu::Texture>>),
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
