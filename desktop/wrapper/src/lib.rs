use graph_craft::application_io::PlatformApplicationIo;
use graph_craft::application_io::resource::ResourceStorage;
use graphite_editor::application::{Editor, Environment, Host, Platform};
use graphite_editor::builder::{Builder, EditorBuilder, RuntimeBuilder};
use graphite_editor::messages::frontend::FrontendMessage;
use graphite_editor::node_graph_executor::NodeRuntime;

use graphite_editor::task::{Clock, Scheduling};
use message_dispatcher::DesktopWrapperMessageDispatcher;
use messages::{DesktopFrontendMessage, DesktopWrapperMessage};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use graph_craft::application_io::resource::MmapResourceStorage;
pub use graphite_editor::consts::{DOUBLE_CLICK_MILLISECONDS, FILE_EXTENSION};
pub use graphite_editor::document_store::{DocumentStore, FolderStore};
pub use graphite_editor::task::{Spawn, Task};
pub use wgpu_executor::Texture;
pub use wgpu_executor::VelloCanvas;
pub use wgpu_executor::VelloSceneBuilder;
pub use wgpu_executor::WgpuBackends;
pub use wgpu_executor::WgpuContext;
pub use wgpu_executor::WgpuContextBuilder;
pub use wgpu_executor::WgpuCurrentSurfaceTexture;
pub use wgpu_executor::WgpuExecutor;
pub use wgpu_executor::WgpuFeatures;
pub use wgpu_executor::WgpuInstance;
pub use wgpu_executor::WgpuSurface;

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
	pub fn new(
		random_seed: u64,
		resource_storage: Arc<dyn ResourceStorage>,
		document_store: Arc<dyn DocumentStore>,
		wgpu_executor: WgpuExecutor,
		spawner: impl Spawn + Send + Sync + 'static,
		editor_wake: impl Fn() + Send + Sync + 'static,
		runtime_wake: impl Fn() + Send + Sync + 'static,
	) -> (Self, NodeRuntime) {
		#[cfg(target_os = "windows")]
		let host = Host::Windows;
		#[cfg(target_os = "macos")]
		let host = Host::Mac;
		#[cfg(target_os = "linux")]
		let host = Host::Linux;
		let env = Environment { platform: Platform::Desktop, host };
		let application_io = PlatformApplicationIo::new_with_executor(wgpu_executor);

		let (editor, runtime) = Builder::new()
			.with_editor(
				EditorBuilder::new()
					.with_wake(editor_wake)
					.with_environment(env)
					.with_resource_storage(resource_storage)
					.with_document_store(document_store),
			)
			.with_runtime(
				RuntimeBuilder::new(Arc::new(InstantClock(Instant::now())))
					.with_wake(runtime_wake)
					.with_application_io(application_io)
					.with_scheduling(Scheduling::default()),
			)
			.with_spawner(spawner)
			.with_random_seed(random_seed)
			.build();

		(Self { editor }, runtime)
	}

	pub fn dispatch(&mut self, message: DesktopWrapperMessage) -> Vec<DesktopFrontendMessage> {
		let mut executor = DesktopWrapperMessageDispatcher::new(&mut self.editor);
		executor.queue_desktop_wrapper_message(message);
		executor.execute()
	}
}

struct InstantClock(Instant);

impl Clock for InstantClock {
	fn now(&self) -> Duration {
		self.0.elapsed()
	}
}

pub fn deserialize_editor_message(data: &[u8]) -> Option<DesktopWrapperMessage> {
	let message = graphite_wasm_wrapper::native_communication::decode_editor_command(data)?;
	Some(DesktopWrapperMessage::FromWeb(message.into()))
}

pub fn serialize_frontend_messages(messages: Vec<FrontendMessage>) -> Option<Vec<u8>> {
	graphite_wasm_wrapper::native_communication::encode_frontend_messages(messages)
}
