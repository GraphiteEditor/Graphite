use dyn_any::StaticType;
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

pub mod resource;

pub use graphene_application_io::ApplicationIo;

#[derive(Default)]
pub struct PlatformApplicationIo {
	#[cfg(feature = "wgpu")]
	gpu_executor: Option<WgpuExecutor>,
	resources: Option<Box<dyn resource::LoadResource>>,
}

impl PlatformApplicationIo {
	pub async fn new() -> Self {
		#[cfg(all(feature = "wgpu", not(target_family = "wasm")))]
		let executor = WgpuExecutor::new().await;
		#[cfg(all(feature = "wgpu", target_family = "wasm"))]
		let executor = graphene_canvas_utils::create_wgpu_executor().await;

		Self {
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			resources: None,
		}
	}

	#[cfg(feature = "wgpu")]
	pub fn new_with_executor(executor: WgpuExecutor) -> Self {
		Self {
			gpu_executor: Some(executor),
			resources: None,
		}
	}

	pub fn inject_resource_proxy(&mut self, resources: Box<dyn resource::LoadResource>) {
		self.resources = Some(resources);
	}
}

impl ApplicationIo for PlatformApplicationIo {
	#[cfg(feature = "wgpu")]
	type Executor = WgpuExecutor;
	#[cfg(not(feature = "wgpu"))]
	type Executor = ();

	#[cfg(feature = "wgpu")]
	fn gpu_executor(&self) -> Option<&Self::Executor> {
		self.gpu_executor.as_ref()
	}

	fn load_resource(&self, hash: resource::ResourceHash) -> resource::ResourceFuture<'_> {
		match self.resources.as_ref() {
			Some(resources) => resources.load(hash),
			None => {
				log::error!("load_resource called before resource storage was initialized");
				Box::pin(std::future::ready(None))
			}
		}
	}
}

impl std::fmt::Debug for PlatformApplicationIo {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("PlatformApplicationIo").finish_non_exhaustive()
	}
}

unsafe impl StaticType for PlatformApplicationIo {
	type Static = PlatformApplicationIo;
}

pub type PlatformEditorApi = graphene_application_io::EditorApi<PlatformApplicationIo>;
