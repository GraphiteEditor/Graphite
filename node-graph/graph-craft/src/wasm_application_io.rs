use dyn_any::StaticType;
use graphene_application_io::{ApplicationError, ApplicationIo, ResourceFuture};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::atomic::Ordering;
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;
#[cfg(target_family = "wasm")]
use wasm_bindgen::JsCast;
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

#[derive(Debug, Default)]
pub struct WasmApplicationIo {
	#[cfg(feature = "wgpu")]
	pub(crate) gpu_executor: Option<WgpuExecutor>,
	pub resources: HashMap<String, Arc<[u8]>>,
}

static WGPU_AVAILABLE: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

/// Returns:
/// - `None` if the availability of WGPU has not been determined yet
/// - `Some(true)` if WGPU is available
/// - `Some(false)` if WGPU is not available
pub fn wgpu_available() -> Option<bool> {
	match WGPU_AVAILABLE.load(Ordering::SeqCst) {
		-1 => None,
		0 => Some(false),
		_ => Some(true),
	}
}

impl WasmApplicationIo {
	pub async fn new() -> Self {
		#[cfg(all(feature = "wgpu", target_family = "wasm"))]
		let executor = if let Some(gpu) = web_sys::window().map(|w| w.navigator().gpu()) {
			let request_adapter = || {
				let request_adapter = js_sys::Reflect::get(&gpu, &wasm_bindgen::JsValue::from_str("requestAdapter")).ok()?;
				let function = request_adapter.dyn_ref::<js_sys::Function>()?;
				Some(function.call0(&gpu).ok())
			};
			let result = request_adapter();
			match result {
				None => None,
				Some(_) => WgpuExecutor::new().await,
			}
		} else {
			None
		};

		#[cfg(all(feature = "wgpu", not(target_family = "wasm")))]
		let executor = WgpuExecutor::new().await;

		#[cfg(not(feature = "wgpu"))]
		let wgpu_available = false;
		#[cfg(feature = "wgpu")]
		let wgpu_available = executor.is_some();
		WGPU_AVAILABLE.store(wgpu_available as i8, Ordering::SeqCst);

		let mut io = Self {
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			resources: HashMap::new(),
		};
		io.resources.insert("null".to_string(), Arc::from(include_bytes!("null.png").to_vec()));

		io
	}

	pub async fn new_offscreen() -> Self {
		#[cfg(feature = "wgpu")]
		let executor = WgpuExecutor::new().await;

		#[cfg(not(feature = "wgpu"))]
		let wgpu_available = false;
		#[cfg(feature = "wgpu")]
		let wgpu_available = executor.is_some();
		WGPU_AVAILABLE.store(wgpu_available as i8, Ordering::SeqCst);

		let mut io = Self {
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			resources: HashMap::new(),
		};

		io.resources.insert("null".to_string(), Arc::from(include_bytes!("null.png").to_vec()));

		io
	}
	#[cfg(all(not(target_family = "wasm"), feature = "wgpu"))]
	pub fn new_with_context(context: wgpu_executor::WgpuContext) -> Self {
		#[cfg(feature = "wgpu")]
		let executor = WgpuExecutor::with_context(context);

		#[cfg(not(feature = "wgpu"))]
		let wgpu_available = false;
		#[cfg(feature = "wgpu")]
		let wgpu_available = executor.is_some();
		WGPU_AVAILABLE.store(wgpu_available as i8, Ordering::SeqCst);

		let mut io = Self {
			gpu_executor: executor,
			resources: HashMap::new(),
		};

		io.resources.insert("null".to_string(), Arc::from(include_bytes!("null.png").to_vec()));

		io
	}
}

unsafe impl StaticType for WasmApplicationIo {
	type Static = WasmApplicationIo;
}

impl<'a> From<&'a WasmEditorApi> for &'a WasmApplicationIo {
	fn from(editor_api: &'a WasmEditorApi) -> Self {
		editor_api.application_io.as_ref().unwrap()
	}
}
#[cfg(feature = "wgpu")]
impl<'a> From<&'a WasmApplicationIo> for &'a WgpuExecutor {
	fn from(app_io: &'a WasmApplicationIo) -> Self {
		app_io.gpu_executor.as_ref().unwrap()
	}
}

pub type WasmEditorApi = graphene_application_io::EditorApi<WasmApplicationIo>;

impl ApplicationIo for WasmApplicationIo {
	#[cfg(feature = "wgpu")]
	type Executor = WgpuExecutor;
	#[cfg(not(feature = "wgpu"))]
	type Executor = ();

	#[cfg(feature = "wgpu")]
	fn gpu_executor(&self) -> Option<&Self::Executor> {
		self.gpu_executor.as_ref()
	}

	fn load_resource(&self, url: impl AsRef<str>) -> Result<ResourceFuture, ApplicationError> {
		let url = url::Url::parse(url.as_ref()).map_err(|_| ApplicationError::InvalidUrl)?;
		log::trace!("Loading resource: {url:?}");
		match url.scheme() {
			#[cfg(feature = "tokio")]
			"file" => {
				let path = url.to_file_path().map_err(|_| ApplicationError::NotFound)?;
				let path = path.to_str().ok_or(ApplicationError::NotFound)?;
				let path = path.to_owned();
				Ok(Box::pin(async move {
					let file = tokio::fs::File::open(path).await.map_err(|_| ApplicationError::NotFound)?;
					let mut reader = tokio::io::BufReader::new(file);
					let mut data = Vec::new();
					reader.read_to_end(&mut data).await.map_err(|_| ApplicationError::NotFound)?;
					Ok(Arc::from(data))
				}) as ResourceFuture)
			}
			"http" | "https" => {
				let url = url.to_string();
				Ok(Box::pin(async move {
					let client = reqwest::Client::new();
					let response = client.get(url).send().await.map_err(|_| ApplicationError::NotFound)?;
					let data = response.bytes().await.map_err(|_| ApplicationError::NotFound)?;
					Ok(Arc::from(data.to_vec()))
				}) as ResourceFuture)
			}
			"graphite" => {
				let path = url.path();
				let path = path.to_owned();
				log::trace!("Loading local resource: {path}");
				let data = self.resources.get(&path).ok_or(ApplicationError::NotFound)?.clone();
				Ok(Box::pin(async move { Ok(data.clone()) }) as ResourceFuture)
			}
			_ => Err(ApplicationError::NotFound),
		}
	}
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
