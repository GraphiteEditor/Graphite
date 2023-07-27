use std::cell::RefCell;

use core::future::Future;
use dyn_any::StaticType;
use graphene_core::application_io::{ApplicationError, ApplicationIo, ResourceFuture, SurfaceHandle, SurfaceHandleFrame, SurfaceId};
use graphene_core::raster::Image;
use graphene_core::Color;
use graphene_core::{
	raster::{color::SRGBA8, ImageFrame},
	Node,
};
#[cfg(target_arch = "wasm32")]
use js_sys::{Object, Reflect};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
use wasm_bindgen::{Clamped, JsCast};
#[cfg(target_arch = "wasm32")]
use web_sys::window;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

pub struct Canvas(CanvasRenderingContext2d);

#[derive(Debug, Default)]
pub struct WasmApplicationIo {
	#[cfg(target_arch = "wasm32")]
	ids: RefCell<u64>,
	#[cfg(feature = "wgpu")]
	pub(crate) gpu_executor: Option<WgpuExecutor>,
	#[cfg(not(target_arch = "wasm32"))]
	windows: RefCell<Vec<Arc<winit::window::Window>>>,
	pub resources: HashMap<String, Arc<[u8]>>,
}

impl WasmApplicationIo {
	pub async fn new() -> Self {
		let mut io = Self {
			#[cfg(target_arch = "wasm32")]
			ids: RefCell::new(0),
			#[cfg(feature = "wgpu")]
			gpu_executor: WgpuExecutor::new().await,
			#[cfg(not(target_arch = "wasm32"))]
			windows: RefCell::new(Vec::new()),
			resources: HashMap::new(),
		};
		io.resources.insert("null".to_string(), Arc::from(include_bytes!("null.png").to_vec()));
		io
	}
}

unsafe impl StaticType for WasmApplicationIo {
	type Static = WasmApplicationIo;
}

impl<'a> From<WasmEditorApi<'a>> for &'a WasmApplicationIo {
	fn from(editor_api: WasmEditorApi<'a>) -> Self {
		editor_api.application_io
	}
}
#[cfg(feature = "wgpu")]
impl<'a> From<&'a WasmApplicationIo> for &'a WgpuExecutor {
	fn from(app_io: &'a WasmApplicationIo) -> Self {
		app_io.gpu_executor.as_ref().unwrap()
	}
}

pub type WasmEditorApi<'a> = graphene_core::application_io::EditorApi<'a, WasmApplicationIo>;

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
			let mut guard = self.ids.borrow_mut();
			let id = SurfaceId(*guard);
			*guard += 1;
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
			let js_key = JsValue::from_str(format!("canvas{}", id.0).as_str());
			let js_value = JsValue::from(canvas.clone());

			let canvases = Object::from(canvases.unwrap());

			// Use Reflect API to set property
			Reflect::set(&canvases, &js_key, &js_value)?;
			Ok::<_, JsValue>(SurfaceHandle { surface_id: id, surface: canvas })
		};

		wrapper().expect("should be able to set canvas in global scope")
	}
	#[cfg(not(target_arch = "wasm32"))]
	fn create_surface(&self) -> SurfaceHandle<Self::Surface> {
		#[cfg(feature = "wayland")]
		use winit::platform::wayland::EventLoopBuilderExtWayland;

		#[cfg(feature = "wayland")]
		let event_loop = winit::event_loop::EventLoopBuilder::new().with_any_thread(true).build();
		#[cfg(not(feature = "wayland"))]
		let event_loop = winit::event_loop::EventLoop::new();
		let window = winit::window::WindowBuilder::new()
			.with_title("Graphite")
			.with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
			.build(&event_loop)
			.unwrap();
		let window = Arc::new(window);
		self.windows.borrow_mut().push(window.clone());
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
		log::trace!("Loading resource: {:?}", url);
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
				log::trace!("Loading local resource: {}", path);
				let data = self.resources.get(&path).ok_or(ApplicationError::NotFound)?.clone();
				Ok(Box::pin(async move { Ok(data.clone()) }) as Pin<Box<dyn Future<Output = Result<Arc<[u8]>, _>>>>)
			}
			_ => Err(ApplicationError::NotFound),
		}
	}
}

pub type WasmSurfaceHandle = SurfaceHandle<HtmlCanvasElement>;
pub type WasmSurfaceHandleFrame = SurfaceHandleFrame<HtmlCanvasElement>;

pub struct CreateSurfaceNode {}

#[node_macro::node_fn(CreateSurfaceNode)]
async fn create_surface_node<'a: 'input>(editor: WasmEditorApi<'a>) -> Arc<SurfaceHandle<<WasmApplicationIo as ApplicationIo>::Surface>> {
	editor.application_io.create_surface().into()
}

pub struct DrawImageFrameNode<Surface> {
	surface_handle: Surface,
}

#[node_macro::node_fn(DrawImageFrameNode)]
async fn draw_image_frame_node<'a: 'input>(image: ImageFrame<SRGBA8>, surface_handle: Arc<SurfaceHandle<HtmlCanvasElement>>) -> SurfaceHandleFrame<HtmlCanvasElement> {
	let image_data = image.image.data;
	let array: Clamped<&[u8]> = Clamped(bytemuck::cast_slice(image_data.as_slice()));
	if image.image.width > 0 && image.image.height > 0 {
		let canvas = &surface_handle.surface;
		canvas.set_width(image.image.width);
		canvas.set_height(image.image.height);
		// TODO: replace "2d" with "bitmaprenderer" once we switch to ImageBitmap (lives on gpu) from ImageData (lives on cpu)
		let context = canvas.get_context("2d").unwrap().unwrap().dyn_into::<CanvasRenderingContext2d>().unwrap();
		let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(array, image.image.width, image.image.height).expect("Failed to construct ImageData");
		context.put_image_data(&image_data, 0.0, 0.0).unwrap();
	}
	SurfaceHandleFrame {
		surface_handle,
		transform: image.transform,
	}
}

pub struct LoadResourceNode<Url> {
	url: Url,
}

#[node_macro::node_fn(LoadResourceNode)]
async fn load_resource_node<'a: 'input>(editor: WasmEditorApi<'a>, url: String) -> Arc<[u8]> {
	editor.application_io.load_resource(url).unwrap().await.unwrap()
}

pub struct DecodeImageNode;

#[node_macro::node_fn(DecodeImageNode)]
fn decode_image_node<'a: 'input>(data: Arc<[u8]>) -> ImageFrame<Color> {
	let image = image::load_from_memory(data.as_ref()).expect("Failed to decode image");
	let image = image.to_rgba32f();
	let image = ImageFrame {
		image: Image {
			data: image.chunks(4).map(|pixel| Color::from_unassociated_alpha(pixel[0], pixel[1], pixel[2], pixel[3])).collect(),
			width: image.width(),
			height: image.height(),
		},
		transform: glam::DAffine2::IDENTITY,
	};
	image
}
