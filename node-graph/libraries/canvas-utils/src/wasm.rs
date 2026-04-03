use dyn_any::DynAny;
#[cfg(feature = "wgpu")]
use graphene_application_io::ImageTexture;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use web_sys::js_sys::{Object, Reflect};
use web_sys::wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, window};
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

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
	fn present(&mut self, image_texture: &ImageTexture, executor: &WgpuExecutor);
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
pub struct CanvasSurfaceHandle(CanvasHandle, Option<Arc<wgpu::Surface<'static>>>);
#[cfg(feature = "wgpu")]
impl CanvasSurfaceHandle {
	pub fn new() -> Self {
		Self(CanvasHandle::new(), None)
	}
	fn surface(&mut self, executor: &WgpuExecutor) -> &wgpu::Surface<'_> {
		if self.1.is_none() {
			let canvas = self.0.get().canvas.clone();
			let surface = executor
				.context
				.instance
				.create_surface(wgpu::SurfaceTarget::Canvas(canvas))
				.expect("Failed to create surface from canvas");
			self.1 = Some(Arc::new(surface));
		}
		self.1.as_ref().unwrap()
	}
}
#[cfg(feature = "wgpu")]
impl Canvas for CanvasSurfaceHandle {
	fn id(&mut self) -> CanvasId {
		self.0.id()
	}
	fn context(&mut self) -> CanvasRenderingContext2d {
		self.0.context()
	}
	fn set_resolution(&mut self, resolution: glam::UVec2) {
		self.0.set_resolution(resolution);
	}
}
#[cfg(feature = "wgpu")]
impl CanvasSurface for CanvasSurfaceHandle {
	fn present(&mut self, image_texture: &ImageTexture, executor: &WgpuExecutor) {
		let source_texture: &wgpu::Texture = image_texture.as_ref();

		let surface = self.surface(executor);

		// Blit the texture to the surface
		let mut encoder = executor.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("Texture to Surface Blit"),
		});

		let size = source_texture.size();

		// Configure the surface at physical resolution (for HiDPI displays)
		let surface_caps = surface.get_capabilities(&executor.context.adapter);
		surface.configure(
			&executor.context.device,
			&wgpu::SurfaceConfiguration {
				usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
				format: wgpu::TextureFormat::Rgba8Unorm,
				width: size.width,
				height: size.height,
				present_mode: surface_caps.present_modes[0],
				alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
				view_formats: vec![],
				desired_maximum_frame_latency: 2,
			},
		);

		let surface_texture = surface.get_current_texture().expect("Failed to get surface texture");

		encoder.copy_texture_to_texture(
			wgpu::TexelCopyTextureInfoBase {
				texture: source_texture,
				mip_level: 0,
				origin: Default::default(),
				aspect: Default::default(),
			},
			wgpu::TexelCopyTextureInfoBase {
				texture: &surface_texture.texture,
				mip_level: 0,
				origin: Default::default(),
				aspect: Default::default(),
			},
			source_texture.size(),
		);

		executor.context.queue.submit([encoder.finish()]);
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
		if canvases.is_err() {
			Reflect::set(&JsValue::from(web_sys::window().unwrap()), &image_canvases_key, &Object::new()).unwrap();
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
