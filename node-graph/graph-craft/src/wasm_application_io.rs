use dyn_any::StaticType;
use graphene_core::application_io::SurfaceHandleFrame;
use graphene_core::application_io::{ApplicationError, ApplicationIo, ResourceFuture, SurfaceHandle, SurfaceId};
use wgpu_executor::WgpuExecutor;

#[cfg(target_arch = "wasm32")]
use js_sys::{Object, Reflect};
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
// #[cfg(not(target_arch = "wasm32"))]
// use std::sync::Mutex;
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use web_sys::window;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;

#[derive(Debug, Default)]
pub struct WasmApplicationIo {
	#[cfg(target_arch = "wasm32")]
	ids: AtomicU64,
	#[cfg(feature = "wgpu")]
	pub(crate) gpu_executor: Option<WgpuExecutor>,
	// #[cfg(not(target_arch = "wasm32"))]
	// windows: Mutex<Vec<Arc<winit::window::Window>>>,
	pub resources: HashMap<String, Arc<[u8]>>,
}

static WGPU_AVAILABLE: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

pub fn wgpu_available() -> Option<bool> {
	match WGPU_AVAILABLE.load(::std::sync::atomic::Ordering::SeqCst) {
		-1 => None,
		0 => Some(false),
		_ => Some(true),
	}
}

impl WasmApplicationIo {
	pub async fn new() -> Self {
		#[cfg(target_arch = "wasm32")]
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
		#[cfg(not(target_arch = "wasm32"))]
		let executor = WgpuExecutor::new().await;
		WGPU_AVAILABLE.store(executor.is_some() as i8, ::std::sync::atomic::Ordering::SeqCst);
		let mut io = Self {
			#[cfg(target_arch = "wasm32")]
			ids: AtomicU64::new(0),
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			// #[cfg(not(target_arch = "wasm32"))]
			// windows: Vec::new().into(),
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

pub type WasmEditorApi = graphene_core::application_io::EditorApi<WasmApplicationIo>;

impl ApplicationIo for WasmApplicationIo {
	#[cfg(target_arch = "wasm32")]
	type Surface = HtmlCanvasElement;
	#[cfg(not(target_arch = "wasm32"))]
	type Surface = winit::window::Window;
	#[cfg(feature = "wgpu")]
	type Executor = WgpuExecutor;
	#[cfg(not(feature = "wgpu"))]
	type Executor = ();

	#[cfg(target_arch = "wasm32")]
	fn create_surface(&self) -> SurfaceHandle<Self::Surface> {
		let wrapper = || {
			let document = window().expect("should have a window in this context").document().expect("window should have a document");

			let canvas: HtmlCanvasElement = document.create_element("canvas")?.dyn_into::<HtmlCanvasElement>()?;
			let id = self.ids.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
				surface_id: graphene_core::SurfaceId(id),
				surface: canvas,
			})
		};

		wrapper().expect("should be able to set canvas in global scope")
	}
	#[cfg(not(target_arch = "wasm32"))]
	fn create_surface(&self) -> SurfaceHandle<Self::Surface> {
		#[cfg(feature = "wayland")]
		use winit::platform::wayland::EventLoopBuilderExtWayland;

		#[cfg(feature = "wayland")]
		let event_loop = winit::event_loop::EventLoopBuilder::new().with_any_thread(true).build().unwrap();
		#[cfg(not(feature = "wayland"))]
		let event_loop = winit::event_loop::EventLoop::new().unwrap();
		let window = winit::window::WindowBuilder::new()
			.with_title("Graphite")
			.with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
			.build(&event_loop)
			.unwrap();
		// self.windows.lock().as_mut().unwrap().push(window.clone());
		SurfaceHandle {
			surface_id: SurfaceId(window.id().into()),
			surface: window,
		}
	}

	#[cfg(target_arch = "wasm32")]
	fn destroy_surface(&self, surface_id: SurfaceId) {
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
	fn destroy_surface(&self, _surface_id: SurfaceId) {}

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

pub type WasmSurfaceHandle = SurfaceHandle<wgpu_executor::Window>;
pub type WasmSurfaceHandleFrame = SurfaceHandleFrame<wgpu_executor::Window>;

#[derive(Clone, Debug, PartialEq, Hash, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EditorPreferences {
	pub imaginate_hostname: String,
	pub use_vello: bool,
}

impl graphene_core::application_io::GetEditorPreferences for EditorPreferences {
	fn hostname(&self) -> &str {
		&self.imaginate_hostname
	}
	fn use_vello(&self) -> bool {
		self.use_vello
	}
}

impl Default for EditorPreferences {
	fn default() -> Self {
		Self {
			imaginate_hostname: "http://localhost:7860/".into(),
			use_vello: false,
		}
	}
}

unsafe impl dyn_any::StaticType for EditorPreferences {
	type Static = EditorPreferences;
}
