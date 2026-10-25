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
		Self {
			#[cfg(feature = "wgpu")]
			gpu_executor: WgpuExecutor::new().await,
			resources: None,
		}
	}

	#[cfg(feature = "wgpu")]
	pub fn new_with_context(context: wgpu_executor::WgpuContext) -> Self {
		Self {
			gpu_executor: WgpuExecutor::with_context(context),
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

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EditorPreferences {
	/// Maximum render region size in pixels along one dimension of the square area.
	pub max_render_region_size: u32,
}

impl graphene_application_io::GetEditorPreferences for EditorPreferences {
	fn max_render_region_area(&self) -> u32 {
		let size = self.max_render_region_size.min(u32::MAX.isqrt());
		size.pow(2)
	}
}

impl Default for EditorPreferences {
	fn default() -> Self {
		Self { max_render_region_size: 1280 }
	}
}

unsafe impl StaticType for EditorPreferences {
	type Static = EditorPreferences;
}
