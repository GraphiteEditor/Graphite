use dyn_any::StaticType;
use graphene_core::application_io::{ApplicationError, ApplicationIo, ExportFormat, RenderConfig, ResourceFuture, SurfaceHandle, SurfaceHandleFrame, SurfaceId};
use graphene_core::raster::bbox::Bbox;
use graphene_core::raster::Image;
use graphene_core::raster::{color::SRGBA8, ImageFrame};
use graphene_core::renderer::{format_transform_matrix, GraphicElementRendered, ImageRenderMode, RenderParams, RenderSvgSegmentList, SvgRender};
use graphene_core::transform::{Footprint, TransformMut};
use graphene_core::Color;
use graphene_core::Node;
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

use base64::Engine;
use glam::DAffine2;

use core::future::Future;
#[cfg(target_arch = "wasm32")]
use js_sys::{Object, Reflect};
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{Clamped, JsCast};
#[cfg(target_arch = "wasm32")]
use web_sys::window;
#[cfg(target_arch = "wasm32")]
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

#[cfg(any(feature = "resvg", feature = "vello"))]
pub struct Canvas(CanvasRenderingContext2d);

#[derive(Debug, Default)]
pub struct WasmApplicationIo {
	#[cfg(target_arch = "wasm32")]
	ids: AtomicU64,
	#[cfg(feature = "wgpu")]
	pub(crate) gpu_executor: Option<WgpuExecutor>,
	#[cfg(not(target_arch = "wasm32"))]
	windows: Mutex<Vec<Arc<winit::window::Window>>>,
	pub resources: HashMap<String, Arc<[u8]>>,
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
		let mut io = Self {
			#[cfg(target_arch = "wasm32")]
			ids: AtomicU64::new(0),
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			#[cfg(not(target_arch = "wasm32"))]
			windows: Vec::new().into(),
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
	type Surface = Arc<winit::window::Window>;
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
		let window = Arc::new(window);
		self.windows.lock().as_mut().unwrap().push(window.clone());
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
				}) as Pin<Box<dyn Future<Output = Result<Arc<[u8]>, _>>>>)
			}
			"http" | "https" => {
				let url = url.to_string();
				Ok(Box::pin(async move {
					let client = reqwest::Client::new();
					let response = client.get(url).send().await.map_err(|_| ApplicationError::NotFound)?;
					let data = response.bytes().await.map_err(|_| ApplicationError::NotFound)?;
					Ok(Arc::from(data.to_vec()))
				}) as Pin<Box<dyn Future<Output = Result<Arc<[u8]>, _>>>>)
			}
			"graphite" => {
				let path = url.path();
				let path = path.to_owned();
				log::trace!("Loading local resource: {path}");
				let data = self.resources.get(&path).ok_or(ApplicationError::NotFound)?.clone();
				Ok(Box::pin(async move { Ok(data.clone()) }) as Pin<Box<dyn Future<Output = Result<Arc<[u8]>, _>>>>)
			}
			_ => Err(ApplicationError::NotFound),
		}
	}
}

#[cfg(target_arch = "wasm32")]
pub type WasmSurfaceHandle = SurfaceHandle<HtmlCanvasElement>;
#[cfg(target_arch = "wasm32")]
pub type WasmSurfaceHandleFrame = SurfaceHandleFrame<HtmlCanvasElement>;
