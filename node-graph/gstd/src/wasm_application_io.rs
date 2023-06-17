use std::cell::RefCell;

use dyn_any::StaticType;
use graphene_core::application_io::{ApplicationIo, SurfaceHandle, SurfaceHandleFrame, SurfaceId};
use graphene_core::{
	raster::{color::SRGBA8, ImageFrame},
	Node,
};
use js_sys::{Object, Reflect};
use std::sync::Arc;
use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{window, CanvasRenderingContext2d, HtmlCanvasElement};
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

pub struct Canvas(CanvasRenderingContext2d);

#[derive(Debug, Default)]
pub struct WasmApplicationIo {
	ids: RefCell<u64>,
	#[cfg(feature = "wgpu")]
	pub(crate) gpu_executor: Option<WgpuExecutor>,
	#[cfg(not(target_arch = "wasm32"))]
	windows: RefCell<Vec<Arc<winit::window::Window>>>,
}

impl WasmApplicationIo {
	pub async fn new() -> Self {
		Self {
			ids: RefCell::new(0),
			#[cfg(feature = "wgpu")]
			gpu_executor: WgpuExecutor::new().await,
			#[cfg(not(target_arch = "wasm32"))]
			windows: RefCell::new(Vec::new()),
		}
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
			.with_inner_size(winit::dpi::PhysicalSize::new(1920, 1080))
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
	fn destroy_surface(&self, surface_id: SurfaceId) {}

	#[cfg(feature = "wgpu")]
	fn gpu_executor(&self) -> Option<&Self::Executor> {
		self.gpu_executor.as_ref()
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
		let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(array, image.image.width as u32, image.image.height as u32).expect("Failed to construct ImageData");
		context.put_image_data(&image_data, 0.0, 0.0).unwrap();
	}
	SurfaceHandleFrame {
		surface_handle,
		transform: image.transform,
	}
}
