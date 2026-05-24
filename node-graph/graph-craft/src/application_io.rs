use dyn_any::StaticType;
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

pub mod resource;

pub use graphene_application_io::ApplicationIo;

#[derive(Default)]
pub struct PlatformApplicationIo {
	#[cfg(feature = "wgpu")]
	pub(crate) gpu_executor: Option<WgpuExecutor>,
	resources: Option<Box<dyn resource::LoadResource>>,
}

impl PlatformApplicationIo {
	pub async fn new() -> Self {
		#[cfg(feature = "wgpu")]
		let executor = WgpuExecutor::new().await;

		#[cfg(not(feature = "wgpu"))]
		let wgpu_available = false;
		#[cfg(feature = "wgpu")]
		let wgpu_available = executor.is_some();
		set_wgpu_available(wgpu_available);

		Self {
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			resources: None,
		}
	}

	#[cfg(feature = "wgpu")]
	pub fn new_with_context(context: wgpu_executor::WgpuContext) -> Self {
		let executor = WgpuExecutor::with_context(context);

		let wgpu_available = executor.is_some();
		set_wgpu_available(wgpu_available);

		Self {
			gpu_executor: executor,
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

	fn load_resource(&self, hash: resource::ResourceHash) -> resource::ResourceFuture {
		self.resources.as_ref().expect("Resource storage not initialized").load(hash)
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

static WGPU_AVAILABLE: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

/// Returns:
/// - `None` if the availability of WGPU has not been determined yet
/// - `Some(true)` if WGPU is available
/// - `Some(false)` if WGPU is not available
pub fn wgpu_available() -> Option<bool> {
	match WGPU_AVAILABLE.load(std::sync::atomic::Ordering::SeqCst) {
		-1 => None,
		0 => Some(false),
		_ => Some(true),
	}
}

pub(crate) fn set_wgpu_available(available: bool) {
	WGPU_AVAILABLE.store(available as i8, std::sync::atomic::Ordering::SeqCst);
}

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
