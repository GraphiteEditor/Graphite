use dyn_any::StaticType;
use graphene_application_io::{ApplicationError, ApplicationIo, ApplicationIoValue, ResourceFuture, SurfaceHandle, SurfaceId};
#[cfg(target_arch = "wasm32")]
use js_sys::{Object, Reflect};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;
#[cfg(target_arch = "wasm32")]
use web_sys::window;
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

#[derive(Debug)]
struct WindowWrapper {
	#[cfg(target_arch = "wasm32")]
	window: SurfaceHandle<HtmlCanvasElement>,
	#[cfg(not(target_arch = "wasm32"))]
	window: SurfaceHandle<Arc<winit::window::Window>>,
}

#[cfg(target_arch = "wasm32")]
impl Drop for WindowWrapper {
	fn drop(&mut self) {
		let window = window().expect("should have a window in this context");
		let window = Object::from(window);

		let image_canvases_key = JsValue::from_str("imageCanvases");

		let wrapper = || {
			if let Ok(canvases) = Reflect::get(&window, &image_canvases_key) {
				// Convert key and value to JsValue
				let js_key = JsValue::from_str(format!("canvas{}", self.window.window_id).as_str());

				// Use Reflect API to set property
				Reflect::delete_property(&canvases.into(), &js_key)?;
			}
			Ok::<_, JsValue>(())
		};

		wrapper().expect("should be able to set canvas in global scope")
	}
}

#[cfg(target_arch = "wasm32")]
unsafe impl Sync for WindowWrapper {}
#[cfg(target_arch = "wasm32")]
unsafe impl Send for WindowWrapper {}

pub type WasmApplicationIoValue = ApplicationIoValue<WasmApplicationIo>;

#[derive(Debug, Default)]
pub struct WasmApplicationIo {
	#[cfg(target_arch = "wasm32")]
	ids: AtomicU64,
	#[cfg(feature = "wgpu")]
	pub(crate) gpu_executor: Option<WgpuExecutor>,
	windows: Vec<WindowWrapper>,
	pub resources: HashMap<String, Arc<[u8]>>,
}

static WGPU_AVAILABLE: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

pub fn wgpu_available() -> Option<bool> {
	// Always enable wgpu when running with Tauri
	#[cfg(target_arch = "wasm32")]
	if let Some(window) = web_sys::window() {
		if js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("__TAURI__")).is_ok() {
			return Some(true);
		}
	}

	match WGPU_AVAILABLE.load(Ordering::SeqCst) {
		-1 => None,
		0 => Some(false),
		_ => Some(true),
	}
}

impl WasmApplicationIo {
	pub async fn new() -> Self {
		#[cfg(all(feature = "wgpu", target_arch = "wasm32"))]
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

		#[cfg(all(feature = "wgpu", not(target_arch = "wasm32")))]
		let executor = WgpuExecutor::new().await;

		#[cfg(not(feature = "wgpu"))]
		let wgpu_available = false;
		#[cfg(feature = "wgpu")]
		let wgpu_available = executor.is_some();
		WGPU_AVAILABLE.store(wgpu_available as i8, Ordering::SeqCst);

		let mut io = Self {
			#[cfg(target_arch = "wasm32")]
			ids: AtomicU64::new(0),
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			windows: Vec::new(),
			resources: HashMap::new(),
		};
		let window = io.create_window();
		io.windows.push(WindowWrapper { window });
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

		// Always enable wgpu when running with Tauri
		let mut io = Self {
			#[cfg(target_arch = "wasm32")]
			ids: AtomicU64::new(0),
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			windows: Vec::new(),
			resources: HashMap::new(),
		};

		io.resources.insert("null".to_string(), Arc::from(include_bytes!("null.png").to_vec()));

		io
	}
}

unsafe impl StaticType for WasmApplicationIo {
	type Static = WasmApplicationIo;
}

#[cfg(feature = "wgpu")]
impl<'a> From<&'a WasmApplicationIo> for &'a WgpuExecutor {
	fn from(app_io: &'a WasmApplicationIo) -> Self {
		app_io.gpu_executor.as_ref().unwrap()
	}
}
impl ApplicationIo for WasmApplicationIo {
	#[cfg(target_arch = "wasm32")]
	type Surface = HtmlCanvasElement;
	#[cfg(not(target_arch = "wasm32"))]
	type Surface = Arc<winit::window::Window>;
	#[cfg(feature = "wgpu")]
	type Executor = WgpuExecutor;
	#[cfg(not(feature = "wgpu"))]
	type Executor = ();

	#[cfg(target_arch = "wasm32")]
	fn create_window(&self) -> SurfaceHandle<Self::Surface> {
		let wrapper = || {
			let document = window().expect("should have a window in this context").document().expect("window should have a document");

			let canvas: HtmlCanvasElement = document.create_element("canvas")?.dyn_into::<HtmlCanvasElement>()?;
			let id = self.ids.fetch_add(1, Ordering::SeqCst);
			// store the canvas in the global scope so it doesn't get garbage collected
			let window = window().expect("should have a window in this context");
			let window = Object::from(window);

			let image_canvases_key = JsValue::from_str("imageCanvases");

			let mut canvases = Reflect::get(&window, &image_canvases_key);
			if canvases.is_err() {
				Reflect::set(&JsValue::from(web_sys::window().unwrap()), &image_canvases_key, &Object::new()).unwrap();
				canvases = Reflect::get(&window, &image_canvases_key);
			}

			// Convert key and value to JsValue
			let js_key = JsValue::from_str(format!("canvas{}", id).as_str());
			let js_value = JsValue::from(canvas.clone());

			let canvases = Object::from(canvases.unwrap());

			// Use Reflect API to set property
			Reflect::set(&canvases, &js_key, &js_value)?;
			Ok::<_, JsValue>(SurfaceHandle {
				window_id: SurfaceId(id),
				surface: canvas,
			})
		};

		wrapper().expect("should be able to set canvas in global scope")
	}
	#[cfg(not(target_arch = "wasm32"))]
	fn create_window(&self) -> SurfaceHandle<Self::Surface> {
		log::trace!("Spawning window");

		#[cfg(all(not(test), target_os = "linux", feature = "wayland"))]
		use winit::platform::wayland::EventLoopBuilderExtWayland;

		#[cfg(all(not(test), target_os = "linux", feature = "wayland"))]
		let event_loop = winit::event_loop::EventLoopBuilder::new().with_any_thread(true).build().unwrap();
		#[cfg(not(all(not(test), target_os = "linux", feature = "wayland")))]
		let event_loop = winit::event_loop::EventLoop::new().unwrap();

		let window = winit::window::WindowBuilder::new()
			.with_title("Graphite")
			.with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
			.build(&event_loop)
			.unwrap();

		SurfaceHandle {
			window_id: SurfaceId(window.id().into()),
			surface: Arc::new(window),
		}
	}

	#[cfg(target_arch = "wasm32")]
	fn destroy_window(&self, surface_id: SurfaceId) {
		let window = window().expect("should have a window in this context");
		let window = Object::from(window);

		let image_canvases_key = JsValue::from_str("imageCanvases");

		let wrapper = || {
			if let Ok(canvases) = Reflect::get(&window, &image_canvases_key) {
				// Convert key and value to JsValue
				let js_key = JsValue::from_str(format!("canvas{}", surface_id.0).as_str());

				// Use Reflect API to set property
				Reflect::delete_property(&canvases.into(), &js_key)?;
			}
			Ok::<_, JsValue>(())
		};

		wrapper().expect("should be able to set canvas in global scope")
	}

	#[cfg(not(target_arch = "wasm32"))]
	fn destroy_window(&self, _surface_id: SurfaceId) {}

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

	fn window(&self) -> Option<SurfaceHandle<Self::Surface>> {
		self.windows.first().map(|wrapper| wrapper.window.clone())
	}
}

#[cfg(feature = "wgpu")]
pub type WasmSurfaceHandle = SurfaceHandle<wgpu_executor::Window>;
#[cfg(feature = "wgpu")]
pub type WasmSurfaceHandleFrame = graphene_application_io::SurfaceHandleFrame<wgpu_executor::Window>;

#[derive(Clone, Debug, PartialEq, Hash, specta::Type, serde::Serialize, serde::Deserialize)]
pub struct EditorPreferences {
	pub use_vello: bool,
}

impl graphene_application_io::GetEditorPreferences for EditorPreferences {
	fn use_vello(&self) -> bool {
		self.use_vello
	}
}

impl Default for EditorPreferences {
	fn default() -> Self {
		Self {
			#[cfg(target_arch = "wasm32")]
			use_vello: false,
			#[cfg(not(target_arch = "wasm32"))]
			use_vello: true,
		}
	}
}

unsafe impl StaticType for EditorPreferences {
	type Static = EditorPreferences;
}
