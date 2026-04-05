use dyn_any::StaticType;
use graphene_application_io::{ApplicationError, ApplicationIo, EditorApi, ResourceHash};
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;
#[cfg(target_family = "wasm")]
use wasm_bindgen::JsCast;
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

#[derive(Debug, Default)]
pub struct NativeApplicationIo {
	#[cfg(feature = "wgpu")]
	pub(crate) gpu_executor: Option<WgpuExecutor>,
	pub resources: HashMap<ResourceHash, Arc<[u8]>>,
}

impl NativeApplicationIo {
	pub async fn new() -> Self {
		#[cfg(feature = "wgpu")]
		let executor = WgpuExecutor::new().await;

		#[cfg(not(feature = "wgpu"))]
		let wgpu_available = false;
		#[cfg(feature = "wgpu")]
		let wgpu_available = executor.is_some();
		super::set_wgpu_available(wgpu_available);

		Self {
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			resources: HashMap::new(),
		}
	}

	#[cfg(feature = "wgpu")]
	pub fn new_with_context(context: wgpu_executor::WgpuContext) -> Self {
		#[cfg(feature = "wgpu")]
		let executor = WgpuExecutor::with_context(context);

		#[cfg(not(feature = "wgpu"))]
		let wgpu_available = false;
		#[cfg(feature = "wgpu")]
		let wgpu_available = executor.is_some();
		super::set_wgpu_available(wgpu_available);

		Self {
			gpu_executor: executor,
			resources: HashMap::new(),
		}
	}
}

impl ApplicationIo for NativeApplicationIo {
	#[cfg(feature = "wgpu")]
	type Executor = WgpuExecutor;
	#[cfg(not(feature = "wgpu"))]
	type Executor = ();

	#[cfg(feature = "wgpu")]
	fn gpu_executor(&self) -> Option<&Self::Executor> {
		self.gpu_executor.as_ref()
	}

	fn load_resource(&self, hash: &ResourceHash) -> Option<&[u8]> {
		self.resources.get(hash).map(|v| v.as_ref())
	}
}

unsafe impl StaticType for NativeApplicationIo {
	type Static = NativeApplicationIo;
}

impl<'a> From<&'a EditorApi<NativeApplicationIo>> for &'a NativeApplicationIo {
	fn from(editor_api: &'a EditorApi<NativeApplicationIo>) -> Self {
		editor_api.application_io.as_ref().unwrap()
	}
}
#[cfg(feature = "wgpu")]
impl<'a> From<&'a NativeApplicationIo> for &'a WgpuExecutor {
	fn from(app_io: &'a NativeApplicationIo) -> Self {
		app_io.gpu_executor.as_ref().unwrap()
	}
}
