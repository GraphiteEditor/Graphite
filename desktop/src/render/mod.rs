use std::sync::{Arc, Mutex};

use cef::Frame;
use thiserror::Error;
use winit::{event_loop::ActiveEventLoop, window::Window};

#[derive(Clone)]
pub struct FrameBufferHandle {
	pub inner: Arc<Mutex<FrameBuffer>>,
}

impl FrameBufferHandle {
	pub fn new() -> Self {
		Self {
			inner: Arc::new(Mutex::new(FrameBuffer::new())),
		}
	}
}

#[derive(Debug)]
pub struct FrameBuffer {
	// The buffer is only valid after the CEF on_paint, and before it is loaded into the texture
	buffer_has_new_data: bool,
	buffer: Vec<u8>,
	width: u32,
	height: u32,

	viewport_resized: bool,
	viewport_top_left_x: u32,
	viewport_top_left_y: u32,
	viewport_width: u32,
	viewport_height: u32,
}

#[derive(Error, Debug)]
pub(crate) enum FrameBufferError {
	#[error("Invalid buffer size. Expected {expected_width}x{expected_height} recieved {received_width}x{received_height}")]
	InvalidSize {
		expected_width: usize,
		expected_height: usize,
		received_width: usize,
		received_height: usize,
	},

	#[error("Buffer dimensions are correct, but the allocated vec length : {vec_length} does not match the buffer length: {buffer_length}")]
	InvalidBufferSize { vec_length: usize, buffer_length: usize },
}

impl FrameBuffer {
	//Initialize the frame buffer to 4k, but set width and height to 0 as it should be initialized when
	pub fn new() -> Self {
		Self {
			buffer: Vec::with_capacity(3840 * 2160 * 4),
			buffer_has_new_data: false,
			width: 0,
			height: 0,
			viewport_resized: false,
			viewport_top_left_x: 0,
			viewport_top_left_y: 0,
			viewport_height: 0,
			viewport_width: 0,
		}
	}

	// Always keep the frame buffer in sync with the window size
	pub fn resize(&mut self, width: u32, height: u32) -> Result<Self, FrameBufferError> {
		let new_size = width * height * 4;
		if self.buffer.len() < new_size {
			self.buffer.resize(new_size, 0);
		} else {
			self.buffer.truncate(new_size);
		}
	}

	pub fn add_buffer(&mut self, buffer_slice: &[u8], width: u32, height: u32) -> Result<(), FrameBufferError> {
		if width != self.width || height != self.height {
			Err(FrameBufferError::InvalidSize {
				expected_width: self.width,
				expected_height: self.height,
				received_width: width,
				received_height: height,
			})
		} else if buffer_slice.len() != self.buffer.len() {
			Err(FrameBufferError::InvalidBufferSize {
				vec_length: self.buffer.len(),
				buffer_length: buffer_slice.len(),
			})
		} else {
			self.buffer.copy_from_slice(buffer_slice);
			self.buffer_has_new_data = true;
			Ok(())
		}
	}

	pub(crate) fn take_buffer(&mut self) -> Option<(&[u8], u32, u32)> {
		if buffer_has_new_data {
			Some((&self.buffer, self.width, self.height));
			self.buffer_has_new_data = false;
		} else {
			None
		}
	}

	pub fn add_viewport_size(&mut self, viewport_top_left_x: u32, viewport_top_left_y: u32, viewport_width: u32, viewport_height: u32) {
		if self.viewport_top_left_x != viewport_top_left_x || self.viewport_top_left_y != viewport_top_left_y || self.viewport_width != viewport_width || self.viewport_height != viewport_height {
			self.viewport_resized = true;
		}
		self.viewport_top_left_x = viewport_top_left_x;
		self.viewport_top_left_y = viewport_top_left_y;
		self.viewport_width = viewport_width;
		self.viewport_height = viewport_height;
	}

	pub fn get_viewport_size(&mut self) -> Option<(u32, u32, u32, u32)> {
		if self.viewport_resized {
			self.viewport_resized = false;
			Some((self.viewport_top_left_x, self.viewport_top_left_y, self.viewport_width, self.viewport_height))
		} else {
			None
		}
	}

	pub(crate) fn width(&self) -> u32 {
		self.width
	}

	pub(crate) fn height(&self) -> u32 {
		self.height
	}
}

pub(crate) struct GraphicsState {
	pub window: Arc<Window>,
	pub surface: wgpu::Surface<'static>,
	pub device: wgpu::Device,
	pub queue: wgpu::Queue,
	pub config: wgpu::SurfaceConfiguration,
	pub render_pipeline: wgpu::RenderPipeline,
	pub sampler: wgpu::Sampler,
}

impl GraphicsState {
	pub(crate) async fn init(event_loop: &ActiveEventLoop) -> Self {
		let window = Arc::new(
			event_loop
				.create_window(
					Window::default_attributes()
						.with_title("CEF Offscreen Rendering Test")
						.with_inner_size(winit::dpi::LogicalSize::new(800, 600)),
				)
				.unwrap(),
		);

		let size = window.inner_size();

		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
			backends: wgpu::Backends::PRIMARY,
			..Default::default()
		});
		let surface = instance.create_surface(window.clone()).unwrap();

		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::default(),
				compatible_surface: Some(&surface),
				force_fallback_adapter: false,
			})
			.await
			.unwrap();

		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					required_features: wgpu::Features::empty(),
					required_limits: wgpu::Limits::default(),
					label: None,
					memory_hints: Default::default(),
				},
				None,
			)
			.await
			.unwrap();

		let surface_caps = surface.get_capabilities(&adapter);
		let surface_format = surface_caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(surface_caps.formats[0]);

		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: surface_format,
			width: size.width,
			height: size.height,
			present_mode: surface_caps.present_modes[0],
			alpha_mode: surface_caps.alpha_modes[0],
			view_formats: vec![],
			desired_maximum_frame_latency: 2,
		};

		// Create shader module
		let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("Shader"),
			source: wgpu::ShaderSource::Wgsl(
				r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    let pos = array(
        // 1st triangle
        vec2f( -1.0,  -1.0),  // center
        vec2f( 1.0,  -1.0),  // right, center
        vec2f( -1.0,  1.0),  // center, top
 
        // 2nd triangle
        vec2f( -1.0,  1.0),  // center, top
        vec2f( 1.0,  -1.0),  // right, center
        vec2f( 1.0,  1.0),  // right, top
    );
    let xy = pos[vertex_index];
    out.clip_position = vec4f(xy , 0.0, 1.0);
    let coords = (xy/ 2. + 0.5);
    out.tex_coords = vec2f(coords.x, 1. - coords.y);
    // // Generate a fullscreen triangle
    // let x = f32(i32(vertex_index) - 1);
    // let y = f32(i32(vertex_index & 1u) * 2 - 1);
    
    // out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    // out.tex_coords = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Test: use texture coordinates as colors to debug
    // return vec4<f32>(in.tex_coords.x, in.tex_coords.y, 0.0, 1.0);
    // Uncomment this line to use CEF texture:
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
"#
				.into(),
			),
		});

		// Create sampler
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
			label: Some("texture_bind_group_layout"),
		});

		let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Render Pipeline Layout"),
			bind_group_layouts: &[&texture_bind_group_layout],
			push_constant_ranges: &[],
		});

		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("Render Pipeline"),
			layout: Some(&render_pipeline_layout),
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
					format: config.format,
					blend: Some(wgpu::BlendState::REPLACE),
					write_mask: wgpu::ColorWrites::ALL,
				})],
				compilation_options: Default::default(),
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				strip_index_format: None,
				front_face: wgpu::FrontFace::Ccw,
				cull_mode: Some(wgpu::Face::Back),
				polygon_mode: wgpu::PolygonMode::Fill,
				unclipped_depth: false,
				conservative: false,
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState {
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
			multiview: None,
			cache: None,
		});

		Self {
			window,
			surface,
			device,
			queue,
			config,
			render_pipeline,
			sampler,
		}
	}

	// Creates the cached ui texture, reconfigures the surface
	pub(crate) fn resize_surface(&mut self, width: u32, height: u32) {
		if width > 0 && height > 0 && (self.config.width != width || self.config.height != height) {
			self.config.width = width;
			self.config.height = height;
			self.surface.configure(&self.device, &self.config);
		}
	}
}
