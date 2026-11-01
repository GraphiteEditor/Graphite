use dyn_any::DynAny;
#[cfg(feature = "wgpu")]
use graphene_application_io::Texture;
use std::sync::Arc;
#[cfg(feature = "wgpu")]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use web_sys::js_sys::{Object, Reflect};
use web_sys::wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, window};
#[cfg(feature = "wgpu")]
use wgpu_executor::{WgpuContextBuilder, WgpuCurrentSurfaceTexture, WgpuExecutor, WgpuSurface};

const CANVASES_OBJECT_KEY: &str = "imageCanvases";

pub type CanvasId = u64;

static CANVAS_IDS: AtomicU64 = AtomicU64::new(0);

pub trait Canvas {
	fn id(&mut self) -> CanvasId;
	fn context(&mut self) -> CanvasRenderingContext2d;
	fn set_resolution(&mut self, resolution: glam::UVec2);
}

#[cfg(feature = "wgpu")]
pub trait CanvasSurface: Canvas {
	fn present(&mut self, texture: &Texture, executor: &WgpuExecutor);
}

#[derive(Clone, DynAny)]
pub struct CanvasHandle(Option<Arc<CanvasImpl>>);
impl CanvasHandle {
	pub fn new() -> Self {
		Self(None)
	}
	fn get(&mut self) -> &CanvasImpl {
		if self.0.is_none() {
			self.0 = Some(Arc::new(CanvasImpl::new()));
		}
		self.0.as_ref().unwrap()
	}
}
impl Canvas for CanvasHandle {
	fn id(&mut self) -> CanvasId {
		self.get().canvas_id
	}
	fn context(&mut self) -> CanvasRenderingContext2d {
		self.get().context()
	}
	fn set_resolution(&mut self, resolution: glam::UVec2) {
		self.get().set_resolution(resolution);
	}
}

#[cfg(feature = "wgpu")]
const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The viewport canvas the WebGL2 device was created from, and its surface.
#[cfg(feature = "wgpu")]
static DEVICE_CANVAS: OnceLock<(CanvasHandle, Arc<WgpuSurface>)> = OnceLock::new();

#[cfg(feature = "wgpu")]
pub async fn create_wgpu_executor() -> Option<WgpuExecutor> {
	// WebGL2 makes its adapter from a canvas's context, so the canvas has to exist first
	let canvas = CanvasImpl::new();
	let (context, surface) = WgpuContextBuilder::new().build_with_surface(wgpu::SurfaceTarget::Canvas(canvas.canvas.clone())).await?;

	if DEVICE_CANVAS.set((CanvasHandle(Some(Arc::new(canvas))), Arc::new(surface))).is_err() {
		log::error!("A second GPU executor was created, which can't present to the viewport canvas");
		return None;
	}

	WgpuExecutor::with_context(context)
}

#[cfg(feature = "wgpu")]
pub struct CanvasSurfaceHandle {
	canvas: CanvasHandle,
	surface: Option<Arc<WgpuSurface>>,
	blitter: Option<wgpu::util::TextureBlitter>,
}
#[cfg(feature = "wgpu")]
impl CanvasSurfaceHandle {
	pub fn new() -> Self {
		Self {
			canvas: CanvasHandle::new(),
			surface: None,
			blitter: None,
		}
	}
	fn surface(&mut self) -> &WgpuSurface {
		if self.surface.is_none() {
			// A WebGL2 device can only present to the canvas it was created from
			let (canvas, surface) = DEVICE_CANVAS.get().expect("The GPU executor should be created by `create_wgpu_executor`");
			self.canvas = canvas.clone();
			self.surface = Some(surface.clone());
		}
		self.surface.as_ref().unwrap()
	}
}
#[cfg(feature = "wgpu")]
impl Canvas for CanvasSurfaceHandle {
	fn id(&mut self) -> CanvasId {
		self.canvas.id()
	}
	fn context(&mut self) -> CanvasRenderingContext2d {
		self.canvas.context()
	}
	fn set_resolution(&mut self, resolution: glam::UVec2) {
		self.canvas.set_resolution(resolution);
	}
}
#[cfg(feature = "wgpu")]
impl CanvasSurface for CanvasSurfaceHandle {
	fn present(&mut self, texture: &Texture, executor: &WgpuExecutor) {
		let context = executor.context();

		let source_texture: &wgpu::Texture = texture.as_ref();

		let surface = self.surface();

		let mut encoder = context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("Texture to Surface Blit"),
		});

		let size = source_texture.size();

		// Configure the surface at physical resolution (for HiDPI displays)
		let surface_caps = surface.get_capabilities(&context.adapter);
		surface.configure(
			&context.device,
			&wgpu::SurfaceConfiguration {
				usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
				format: SURFACE_FORMAT,
				color_space: wgpu::SurfaceColorSpace::Auto,
				width: size.width,
				height: size.height,
				present_mode: surface_caps.present_modes[0],
				// WebGL2 only accepts opaque, but its canvas is still composited as premultiplied
				alpha_mode: wgpu::CompositeAlphaMode::Opaque,
				view_formats: vec![],
				desired_maximum_frame_latency: 2,
			},
		);

		let surface_texture = match surface.get_current_texture(&context.queue) {
			WgpuCurrentSurfaceTexture::Success(t) | WgpuCurrentSurfaceTexture::Suboptimal(t) => t,
			_ => panic!("Failed to get surface texture"),
		};

		// Draw the texture onto the surface, since WebGL2 surfaces can't be copied into
		let blitter = self.blitter.get_or_insert_with(|| wgpu::util::TextureBlitter::new(&context.device, SURFACE_FORMAT));
		let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());
		let target_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
		blitter.copy(&context.device, &mut encoder, &source_view, &target_view);

		surface_texture.queue.submit([encoder.finish()]);
		surface_texture.present();
	}
}

/// A wgpu surface backed by an HTML canvas element.
/// Holds a reference to the canvas to prevent garbage collection.
pub struct CanvasImpl {
	canvas_id: u64,
	canvas: HtmlCanvasElement,
}

impl CanvasImpl {
	fn new() -> Self {
		let document = window().expect("should have a window in this context").document().expect("window should have a document");

		let canvas: HtmlCanvasElement = document.create_element("canvas").unwrap().dyn_into::<HtmlCanvasElement>().unwrap();
		let canvas_id = CANVAS_IDS.fetch_add(1, Ordering::SeqCst);

		// Store the canvas in the global scope so it doesn't get garbage collected
		let window = window().expect("should have a window in this context");
		let window_obj = Object::from(window);

		let image_canvases_key = JsValue::from_str(CANVASES_OBJECT_KEY);

		let mut canvases = Reflect::get(&window_obj, &image_canvases_key);
		if canvases.is_err() || canvases.as_ref().map_or(false, |v| v.is_undefined() || v.is_null()) {
			Reflect::set(&window_obj.clone(), &image_canvases_key, &Object::new()).unwrap();
			canvases = Reflect::get(&window_obj, &image_canvases_key);
		}

		// Convert key and value to JsValue
		let js_key = JsValue::from_str(canvas_id.to_string().as_str());
		let js_value = JsValue::from(canvas.clone());

		let canvases = Object::from(canvases.unwrap());

		// Use Reflect API to set property
		Reflect::set(&canvases, &js_key, &js_value).unwrap();

		Self { canvas_id, canvas }
	}
	fn context(&self) -> CanvasRenderingContext2d {
		self.canvas
			.get_context("2d")
			.expect("Failed to get 2D context from canvas")
			.unwrap()
			.dyn_into::<CanvasRenderingContext2d>()
			.expect("Failed to cast context to CanvasRenderingContext2d")
	}
	fn set_resolution(&self, resolution: glam::UVec2) {
		self.canvas.set_width(resolution.x);
		self.canvas.set_height(resolution.y);
	}
}

impl Drop for CanvasImpl {
	fn drop(&mut self) {
		let canvas_id = self.canvas_id;
		let window = window().expect("should have a window in this context");
		let window_obj = Object::from(window);

		let image_canvases_key = JsValue::from_str(CANVASES_OBJECT_KEY);

		if let Ok(canvases) = Reflect::get(&window_obj, &image_canvases_key) {
			let canvases = Object::from(canvases);
			let js_key = JsValue::from_str(canvas_id.to_string().as_str());
			Reflect::delete_property(&canvases, &js_key).unwrap();
		}
	}
}

// SAFETY: WASM is single-threaded, so Send/Sync are safe
unsafe impl Send for CanvasImpl {}
unsafe impl Sync for CanvasImpl {}
