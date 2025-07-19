use std::sync::Arc;

use thiserror::Error;
use winit::window::Window;

pub(crate) struct FrameBuffer {
	buffer: Vec<u8>,
	width: usize,
	height: usize,
}

#[derive(Error, Debug)]
pub(crate) enum FrameBufferError {
	#[error("Invalid buffer size {buffer_size}, expected {expected_size} for width {width} multiplied with height {height} multiplied by 4 channels")]
	InvalidSize { buffer_size: usize, expected_size: usize, width: usize, height: usize },
}

impl FrameBuffer {
	pub(crate) fn new(buffer: Vec<u8>, width: usize, height: usize) -> Result<Self, FrameBufferError> {
		let fb = Self { buffer, width, height };
		fb.validate_size()?;
		Ok(fb)
	}

	pub(crate) fn buffer(&self) -> &[u8] {
		&self.buffer
	}

	pub(crate) fn width(&self) -> usize {
		self.width
	}

	pub(crate) fn height(&self) -> usize {
		self.height
	}

	fn validate_size(&self) -> Result<(), FrameBufferError> {
		if self.buffer.len() != self.width * self.height * 4 {
			Err(FrameBufferError::InvalidSize {
				buffer_size: self.buffer.len(),
				expected_size: self.width * self.height * 4,
				width: self.width,
				height: self.height,
			})
		} else {
			Ok(())
		}
	}
}

pub(crate) struct GraphicsState {
	surface: wgpu::Surface<'static>,
	device: wgpu::Device,
	queue: wgpu::Queue,
	config: wgpu::SurfaceConfiguration,
	texture: Option<wgpu::Texture>,
	bind_group: Option<wgpu::BindGroup>,
	render_pipeline: wgpu::RenderPipeline,
	sampler: wgpu::Sampler,
}

impl GraphicsState {
	pub(crate) async fn new(window: Arc<Window>) -> Self {
		let size = window.inner_size();

		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
			backends: wgpu::Backends::PRIMARY,
			..Default::default()
		});

		let surface = instance.create_surface(window).unwrap();

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

		surface.configure(&device, &config);

		// Create shader module
		let shader = device.create_shader_module(wgpu::include_wgsl!("fullscreen_texture.wgsl"));

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

		let mut graphics_state = Self {
			surface,
			device,
			queue,
			config,
			texture: None,
			bind_group: None,
			render_pipeline,
			sampler,
		};

		// Initialize with a test pattern so we always have something to render
		let width = 800;
		let height = 600;
		let initial_data = vec![34u8; width * height * 4]; // Gray texture #222222FF

		let fb = FrameBuffer::new(initial_data, width, height)
			.map_err(|e| {
				panic!("Failed to create initial FrameBuffer: {}", e);
			})
			.unwrap();

		graphics_state.update_texture(&fb);

		graphics_state
	}

	pub(crate) fn resize(&mut self, width: usize, height: usize) {
		if width > 0 && height > 0 && (self.config.width != width as u32 || self.config.height != height as u32) {
			self.config.width = width as u32;
			self.config.height = height as u32;
			self.surface.configure(&self.device, &self.config);
		}
	}

	pub(crate) fn update_texture(&mut self, frame_buffer: &FrameBuffer) {
		let data = frame_buffer.buffer();
		let width = frame_buffer.width() as u32;
		let height = frame_buffer.height() as u32;

		if width > 0 && height > 0 && (self.config.width != width || self.config.height != height) {
			self.config.width = width;
			self.config.height = height;
			self.surface.configure(&self.device, &self.config);
		}

		let texture = self.device.create_texture(&wgpu::TextureDescriptor {
			label: Some("CEF Texture"),
			size: wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Bgra8UnormSrgb,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});

		self.queue.write_texture(
			wgpu::ImageCopyTexture {
				texture: &texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			data,
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: Some(4 * width),
				rows_per_image: Some(height),
			},
			wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
		);

		let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

		let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &self.render_pipeline.get_bind_group_layout(0),
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&texture_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&self.sampler),
				},
			],
			label: Some("texture_bind_group"),
		});

		self.texture = Some(texture);
		self.bind_group = Some(bind_group);
	}

	pub(crate) fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
		let output = self.surface.get_current_texture()?;
		let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

		let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") });

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &view,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.01, g: 0.01, b: 0.01, a: 1.0 }),
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: None,
				occlusion_query_set: None,
				timestamp_writes: None,
			});

			render_pass.set_pipeline(&self.render_pipeline);
			if let Some(bind_group) = &self.bind_group {
				render_pass.set_bind_group(0, bind_group, &[]);
				render_pass.draw(0..6, 0..1); // Draw 3 vertices for fullscreen triangle
			} else {
				println!("No bind group available - showing clear color only");
			}
		}
		self.queue.submit(std::iter::once(encoder.finish()));
		output.present();

		Ok(())
	}
}
