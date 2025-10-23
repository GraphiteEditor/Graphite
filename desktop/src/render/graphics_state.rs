use crate::window::Window;

use graphite_desktop_wrapper::{Color, WgpuContext, WgpuExecutor};

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub(crate) struct GraphicsState {
	surface: wgpu::Surface<'static>,
	context: WgpuContext,
	executor: WgpuExecutor,
	config: wgpu::SurfaceConfiguration,
	render_pipeline: wgpu::RenderPipeline,
	transparent_texture: wgpu::Texture,
	sampler: wgpu::Sampler,
	viewport_scale: [f32; 2],
	viewport_offset: [f32; 2],
	viewport_texture: Option<wgpu::Texture>,
	overlays_texture: Option<wgpu::Texture>,
	ui_texture: Option<wgpu::Texture>,
	bind_group: Option<wgpu::BindGroup>,
	#[derivative(Debug = "ignore")]
	overlays_scene: Option<vello::Scene>,
}

impl GraphicsState {
	pub(crate) fn new(window: &Window, context: WgpuContext) -> Self {
		let size = window.surface_size();
		let surface = window.create_surface(context.instance.clone());

		let surface_caps = surface.get_capabilities(&context.adapter);
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

		surface.configure(&context.device, &config);

		let transparent_texture = context.device.create_texture(&wgpu::TextureDescriptor {
			label: Some("Transparent Texture"),
			size: wgpu::Extent3d {
				width: 1,
				height: 1,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Bgra8UnormSrgb,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});

		// Create shader module
		let shader = context.device.create_shader_module(wgpu::include_wgsl!("composite_shader.wgsl"));

		// Create sampler
		let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let texture_bind_group_layout = context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 2,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 3,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
			],
			label: Some("texture_bind_group_layout"),
		});

		let render_pipeline_layout = context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Render Pipeline Layout"),
			bind_group_layouts: &[&texture_bind_group_layout],
			push_constant_ranges: &[wgpu::PushConstantRange {
				stages: wgpu::ShaderStages::FRAGMENT,
				range: 0..size_of::<Constants>() as u32,
			}],
		});

		let render_pipeline = context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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

		let wgpu_executor = WgpuExecutor::with_context(context.clone()).expect("Failed to create WgpuExecutor");

		Self {
			surface,
			context,
			executor: wgpu_executor,
			config,
			render_pipeline,
			transparent_texture,
			sampler,
			viewport_scale: [1.0, 1.0],
			viewport_offset: [0.0, 0.0],
			viewport_texture: None,
			overlays_texture: None,
			ui_texture: None,
			bind_group: None,
			overlays_scene: None,
		}
	}

	pub(crate) fn resize(&mut self, width: u32, height: u32) {
		if width > 0 && height > 0 && (self.config.width != width || self.config.height != height) {
			self.config.width = width;
			self.config.height = height;
			self.surface.configure(&self.context.device, &self.config);
		}
	}

	pub(crate) fn bind_viewport_texture(&mut self, viewport_texture: wgpu::Texture) {
		self.viewport_texture = Some(viewport_texture);
		self.update_bindgroup();
	}

	pub(crate) fn bind_overlays_texture(&mut self, overlays_texture: wgpu::Texture) {
		self.overlays_texture = Some(overlays_texture);
		self.update_bindgroup();
	}

	pub(crate) fn bind_ui_texture(&mut self, bind_ui_texture: wgpu::Texture) {
		self.ui_texture = Some(bind_ui_texture);
		self.update_bindgroup();
	}

	pub(crate) fn set_viewport_scale(&mut self, scale: [f32; 2]) {
		self.viewport_scale = scale;
	}

	pub(crate) fn set_viewport_offset(&mut self, offset: [f32; 2]) {
		self.viewport_offset = offset;
	}

	pub(crate) fn set_overlays_scene(&mut self, scene: vello::Scene) {
		self.overlays_scene = Some(scene);
	}

	fn render_overlays(&mut self, scene: vello::Scene) {
		let Some(viewport_texture) = self.viewport_texture.as_ref() else {
			tracing::warn!("No viewport texture bound, cannot render overlays");
			return;
		};
		let size = glam::UVec2::new(viewport_texture.width(), viewport_texture.height());
		let texture = futures::executor::block_on(self.executor.render_vello_scene_to_texture(&scene, size, &Default::default(), Color::TRANSPARENT));
		let Ok(texture) = texture else {
			tracing::error!("Error rendering overlays");
			return;
		};
		self.bind_overlays_texture(texture);
	}

	pub(crate) fn render(&mut self, window: &Window) -> Result<(), wgpu::SurfaceError> {
		if let Some(scene) = self.overlays_scene.take() {
			self.render_overlays(scene);
		}

		let output = self.surface.get_current_texture()?;
		let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

		let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") });

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
			render_pass.set_push_constants(
				wgpu::ShaderStages::FRAGMENT,
				0,
				bytemuck::bytes_of(&Constants {
					viewport_scale: self.viewport_scale,
					viewport_offset: self.viewport_offset,
				}),
			);
			if let Some(bind_group) = &self.bind_group {
				render_pass.set_bind_group(0, bind_group, &[]);
				render_pass.draw(0..6, 0..1); // Draw 3 vertices for fullscreen triangle
			} else {
				tracing::warn!("No bind group available - showing clear color only");
			}
		}
		self.context.queue.submit(std::iter::once(encoder.finish()));
		window.pre_present_notify();
		output.present();

		Ok(())
	}

	fn update_bindgroup(&mut self) {
		let viewport_texture_view = self.viewport_texture.as_ref().unwrap_or(&self.transparent_texture).create_view(&wgpu::TextureViewDescriptor::default());
		let overlays_texture_view = self.overlays_texture.as_ref().unwrap_or(&self.transparent_texture).create_view(&wgpu::TextureViewDescriptor::default());
		let ui_texture_view = self.ui_texture.as_ref().unwrap_or(&self.transparent_texture).create_view(&wgpu::TextureViewDescriptor::default());

		let bind_group = self.context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &self.render_pipeline.get_bind_group_layout(0),
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&viewport_texture_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(&overlays_texture_view),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&ui_texture_view),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&self.sampler),
				},
			],
			label: Some("texture_bind_group"),
		});

		self.bind_group = Some(bind_group);
	}
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Constants {
	viewport_scale: [f32; 2],
	viewport_offset: [f32; 2],
}
