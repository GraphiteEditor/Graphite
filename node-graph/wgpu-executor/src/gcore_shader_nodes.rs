use crate::{Context, WgpuExecutor};
use graphene_core::Ctx;
use graphene_core::application_io::{ImageTexture, TextureFrameTable};
use graphene_core::instances::Instance;
use std::borrow::Cow;
use std::sync::Arc;
use wgpu::{
	BindGroupDescriptor, BindGroupEntry, BindingResource, ColorTargetState, Device, Face, FragmentState, FrontFace, LoadOp, Operations, PolygonMode, PrimitiveState, PrimitiveTopology, Queue,
	RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, StoreOp, TextureDescriptor, TextureDimension, TextureFormat,
	TextureViewDescriptor, VertexState,
};

const WGSL_SHADER: &str = include_str!(env!("WGSL_SHADER_PATH"));

#[node_macro::node(category(""))]
async fn gpu_invert<'a: 'n>(_: impl Ctx, input: TextureFrameTable, executor: &'a WgpuExecutor) -> TextureFrameTable {
	let Context { device, queue, .. } = &executor.context;
	// this should be cached
	let graphics_pipeline = GraphitePerPixelGraphicsPipeline::new(device, "gpu_invertgpu_invert_shadergpu_invert_fragment");
	graphics_pipeline.run(input, queue)
}

pub struct GraphitePerPixelGraphicsPipeline {
	device: Arc<Device>,
	render_pipeline_f32: wgpu::RenderPipeline,
	render_pipeline_f16: wgpu::RenderPipeline,
	render_pipeline_srgb8: wgpu::RenderPipeline,
}

impl GraphitePerPixelGraphicsPipeline {
	pub fn new(device: &Arc<Device>, fragment_shader_name: &str) -> Self {
		let shader_module = device.create_shader_module(ShaderModuleDescriptor {
			label: Some("graphite wgsl shader"),
			source: ShaderSource::Wgsl(Cow::Borrowed(WGSL_SHADER)),
		});
		let create_render_pipeline = |format| {
			device.create_render_pipeline(&RenderPipelineDescriptor {
				label: Some("gpu_invert"),
				layout: None,
				vertex: VertexState {
					module: &shader_module,
					entry_point: Some("fullscreen_vertexfullscreen_vertex"),
					compilation_options: Default::default(),
					buffers: &[],
				},
				primitive: PrimitiveState {
					topology: PrimitiveTopology::TriangleList,
					strip_index_format: None,
					front_face: FrontFace::Ccw,
					cull_mode: Some(Face::Back),
					unclipped_depth: false,
					polygon_mode: PolygonMode::Fill,
					conservative: false,
				},
				depth_stencil: None,
				multisample: Default::default(),
				fragment: Some(FragmentState {
					module: &shader_module,
					entry_point: Some(fragment_shader_name),
					compilation_options: Default::default(),
					targets: &[Some(ColorTargetState {
						format,
						blend: None,
						write_mask: Default::default(),
					})],
				}),
				multiview: None,
				cache: None,
			})
		};
		Self {
			device: device.clone(),
			render_pipeline_f32: create_render_pipeline(TextureFormat::Rgba32Float),
			render_pipeline_f16: create_render_pipeline(TextureFormat::Rgba16Float),
			render_pipeline_srgb8: create_render_pipeline(TextureFormat::Rgba8UnormSrgb),
		}
	}

	pub fn get(&self, format: TextureFormat) -> &wgpu::RenderPipeline {
		match format {
			TextureFormat::Rgba32Float => &self.render_pipeline_f32,
			TextureFormat::Rgba16Float => &self.render_pipeline_f16,
			TextureFormat::Rgba8UnormSrgb => &self.render_pipeline_srgb8,
			_ => panic!("unsupported"),
		}
	}

	pub fn run(&self, input: TextureFrameTable, queue: &Arc<Queue>) -> TextureFrameTable {
		let device = &self.device;
		let mut cmd = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("gpu_invert") });
		let out = input
			.instance_ref_iter()
			.map(|instance| {
				let view_in = instance.instance.texture.create_view(&TextureViewDescriptor::default());
				let format = instance.instance.texture.format();
				let pipeline = self.get(format);

				let bind_group = device.create_bind_group(&BindGroupDescriptor {
					label: Some("gpu_invert bind group"),
					// `get_bind_group_layout` allocates unnecessary memory, we could create it manually to not do that
					layout: &pipeline.get_bind_group_layout(0),
					entries: &[BindGroupEntry {
						binding: 0,
						resource: BindingResource::TextureView(&view_in),
					}],
				});

				let tex_out = device.create_texture(&TextureDescriptor {
					label: Some("gpu_invert_out"),
					size: instance.instance.texture.size(),
					mip_level_count: 1,
					sample_count: 1,
					dimension: TextureDimension::D2,
					format,
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
					view_formats: &[format],
				});

				let view_out = tex_out.create_view(&TextureViewDescriptor::default());
				let mut rp = cmd.begin_render_pass(&RenderPassDescriptor {
					label: Some("gpu_invert rp"),
					color_attachments: &[Some(RenderPassColorAttachment {
						view: &view_out,
						resolve_target: None,
						ops: Operations {
							// should be dont_care but wgpu doesn't expose that
							load: LoadOp::Clear(wgpu::Color::BLACK),
							store: StoreOp::Store,
						},
					})],
					depth_stencil_attachment: None,
					timestamp_writes: None,
					occlusion_query_set: None,
				});
				rp.set_pipeline(&pipeline);
				rp.set_bind_group(0, Some(&bind_group), &[]);
				rp.draw(0..3, 0..1);

				Instance {
					instance: ImageTexture { texture: Arc::new(tex_out) },
					transform: *instance.transform,
					alpha_blending: *instance.alpha_blending,
					source_node_id: *instance.source_node_id,
				}
			})
			.collect::<TextureFrameTable>();
		queue.submit([cmd.finish()]);
		out
	}
}
