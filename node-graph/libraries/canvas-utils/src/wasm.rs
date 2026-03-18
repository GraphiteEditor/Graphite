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

		// Configure the surface using the surface's preferred format
		// (Firefox WebGL prefers Bgra8Unorm, Chrome prefers Rgba8Unorm)
		let surface_caps = surface.get_capabilities(&executor.context.adapter);
		let surface_format = surface_caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(surface_caps.formats[0]);
		surface.configure(
			&executor.context.device,
			&wgpu::SurfaceConfiguration {
				usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
				format: surface_format,
				width: size.width,
				height: size.height,
				present_mode: surface_caps.present_modes[0],
				alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
				view_formats: vec![],
				desired_maximum_frame_latency: 2,
			},
		);

		let surface_texture = surface.get_current_texture().expect("Failed to get surface texture");

		// If the surface format matches the source, use a direct copy; otherwise use a shader-based blit
		// to handle format conversion (e.g., Rgba8Unorm source to Bgra8Unorm surface on Firefox)
		if surface_format == source_texture.format() {
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
		} else {
			// Different format (e.g., Firefox's Bgra8Unorm) — use a shader-based blit for format conversion
			let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());
			let target_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
			blit_texture_with_conversion(&executor.context.device, &executor.context.queue, &mut encoder, &source_view, &target_view, surface_format);
		}

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

/// Blit a texture to a render target with format conversion using a fullscreen shader pass.
/// Used when the surface format differs from the source (e.g., Rgba8Unorm -> Bgra8Unorm on Firefox).
#[cfg(feature = "wgpu")]
fn blit_texture_with_conversion(
	device: &wgpu::Device,
	_queue: &wgpu::Queue,
	encoder: &mut wgpu::CommandEncoder,
	source: &wgpu::TextureView,
	target: &wgpu::TextureView,
	target_format: wgpu::TextureFormat,
) {
	let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Blit Shader"),
		source: wgpu::ShaderSource::Wgsl(
			r"
			@group(0) @binding(0) var src: texture_2d<f32>;
			@group(0) @binding(1) var src_sampler: sampler;

			struct VertexOutput {
				@builtin(position) position: vec4<f32>,
				@location(0) uv: vec2<f32>,
			}

			@vertex
			fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
				var positions = array<vec2<f32>, 3>(
					vec2<f32>(-1.0, -1.0),
					vec2<f32>(3.0, -1.0),
					vec2<f32>(-1.0, 3.0),
				);
				var out: VertexOutput;
				let pos = positions[vertex_index];
				out.position = vec4<f32>(pos, 0.0, 1.0);
				out.uv = vec2<f32>(pos.x * 0.5 + 0.5, 1.0 - (pos.y * 0.5 + 0.5));
				return out;
			}

			@fragment
			fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
				return textureSample(src, src_sampler, in.uv);
			}
			"
			.into(),
		),
	});

	let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		label: Some("Blit Bind Group Layout"),
		entries: &[
			wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Texture {
					sample_type: wgpu::TextureSampleType::Float { filterable: true },
					view_dimension: wgpu::TextureViewDimension::D2,
					multisampled: false,
				},
				count: None,
			},
			wgpu::BindGroupLayoutEntry {
				binding: 1,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
				count: None,
			},
		],
	});

	let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: Some("Blit Pipeline Layout"),
		bind_group_layouts: &[&bind_group_layout],
		push_constant_ranges: &[],
	});

	let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some("Blit Pipeline"),
		layout: Some(&pipeline_layout),
		vertex: wgpu::VertexState {
			module: &shader,
			entry_point: Some("vs_main"),
			buffers: &[],
			compilation_options: Default::default(),
		},
		fragment: Some(wgpu::FragmentState {
			module: &shader,
			entry_point: Some("fs_main"),
			targets: &[Some(wgpu::ColorTargetState {
				format: target_format,
				blend: None,
				write_mask: wgpu::ColorWrites::ALL,
			})],
			compilation_options: Default::default(),
		}),
		primitive: wgpu::PrimitiveState::default(),
		depth_stencil: None,
		multisample: wgpu::MultisampleState::default(),
		multiview: None,
		cache: None,
	});

	let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
		mag_filter: wgpu::FilterMode::Nearest,
		min_filter: wgpu::FilterMode::Nearest,
		..Default::default()
	});

	let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		label: Some("Blit Bind Group"),
		layout: &bind_group_layout,
		entries: &[
			wgpu::BindGroupEntry {
				binding: 0,
				resource: wgpu::BindingResource::TextureView(source),
			},
			wgpu::BindGroupEntry {
				binding: 1,
				resource: wgpu::BindingResource::Sampler(&sampler),
			},
		],
	});

	let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
		label: Some("Blit Render Pass"),
		color_attachments: &[Some(wgpu::RenderPassColorAttachment {
			view: target,
			resolve_target: None,
			ops: wgpu::Operations {
				load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
				store: wgpu::StoreOp::Store,
			},
		})],
		..Default::default()
	});

	render_pass.set_pipeline(&pipeline);
	render_pass.set_bind_group(0, &bind_group, &[]);
	render_pass.draw(0..3, 0..1);
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
