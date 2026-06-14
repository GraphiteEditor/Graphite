use core_types::transform::{Footprint, Transform};
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, OwnedContextImpl};
use glam::{DAffine2, DVec2, UVec2, Vec2};
use graph_craft::document::value::{RenderOutput, RenderOutputType};
use rendering::{RenderOutputType as RenderOutputTypeRequest, RenderParams};
use std::sync::Arc;
use vector_types::vector::style::RenderMode;
use wgpu_executor::{AsyncWgpuPipeline, WgpuExecutor, WgpuPipelineCache};

#[node_macro::node(category(""))]
pub async fn render_pixel_preview<'a: 'n>(
	ctx: impl Ctx + ExtractAll + CloneVarArgs + Sync,
	#[scope(pixel_preview_pipeline::IDENTIFIER)] pipeline: WgpuPipelineCache,
	data: impl Node<Context<'static>, Output = RenderOutput> + Send + Sync,
) -> RenderOutput {
	let Some(render_params) = ctx.vararg(0).ok().and_then(|v| v.downcast_ref::<RenderParams>()).cloned() else {
		log::error!("invalid render params for pixel preview");
		let context = OwnedContextImpl::from(ctx).into_context();
		return data.eval(context).await;
	};
	let physical_scale = render_params.scale;

	let footprint = *ctx.footprint();
	let viewport_zoom = footprint.scale_magnitudes().x * physical_scale;

	if render_params.render_mode != RenderMode::PixelPreview || !matches!(render_params.render_output_type, RenderOutputTypeRequest::Vello) || viewport_zoom <= 1. {
		let context = OwnedContextImpl::from(ctx).into_context();
		return data.eval(context).await;
	}

	let physical_resolution = footprint.resolution;
	let logical_resolution = physical_resolution.as_dvec2() / physical_scale;

	let logical_footprint = Footprint {
		resolution: logical_resolution.as_uvec2().max(UVec2::ONE),
		..footprint
	};

	let bounds = logical_footprint.viewport_bounds_in_local_space();

	let upstream_min = bounds.start.floor();
	let upstream_max = bounds.end.ceil();

	let upstream_size = (upstream_max - upstream_min).max(DVec2::ONE);
	let upstream_resolution = upstream_size.as_uvec2().max(UVec2::ONE);

	let upstream_footprint = Footprint {
		transform: DAffine2::from_scale(DVec2::splat(1. / physical_scale)) * DAffine2::from_translation(-upstream_min),
		resolution: upstream_resolution,
		quality: footprint.quality,
	};

	let new_ctx = OwnedContextImpl::from(ctx).with_footprint(upstream_footprint).with_vararg(Box::new(render_params)).into_context();
	let mut result = data.eval(new_ctx).await;

	let RenderOutputType::Texture(ref source_texture) = result.data else { return result };

	let transform = DAffine2::from_translation(-upstream_min) * footprint.transform.inverse() * DAffine2::from_scale(logical_resolution);

	let resampled = pipeline
		.run::<PixelPreview>(&ResamplerArgs {
			source: source_texture.as_ref(),
			transform: &transform,
			size: physical_resolution,
		})
		.await;

	result.data = RenderOutputType::Texture(resampled.into());

	result
		.metadata
		.apply_transform(footprint.transform * DAffine2::from_translation(upstream_min) * DAffine2::from_scale(DVec2::splat(physical_scale)));

	result
}

#[node_macro::node(category(""), inject_scope)]
async fn pixel_preview_pipeline<'a: 'n>(
	_ctx: impl Ctx,
	#[scope(crate::platform_application_io::wgpu_executor::IDENTIFIER)] executor: &'a WgpuExecutor,
	#[data] pipeline: WgpuPipelineCache,
) -> WgpuPipelineCache {
	executor.pipeline_init::<PixelPreview>(pipeline);
	pipeline.clone()
}

pub struct PixelPreview {
	pipeline: wgpu::RenderPipeline,
	bind_group_layout: wgpu::BindGroupLayout,
}

pub struct ResamplerArgs<'a> {
	source: &'a wgpu::Texture,
	transform: &'a DAffine2,
	size: UVec2,
}

impl AsyncWgpuPipeline for PixelPreview {
	type Args<'a> = ResamplerArgs<'a>;
	type Out = Arc<wgpu::Texture>;

	fn create(executor: &WgpuExecutor) -> Self {
		let device = &executor.context().device;
		let shader = device.create_shader_module(wgpu::include_wgsl!("render_pixel_preview.wgsl"));

		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("resample_bind_group_layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: false },
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				},
			],
		});

		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("resample_pipeline_layout"),
			bind_group_layouts: &[Some(&bind_group_layout)],
			..Default::default()
		});

		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("resample_pipeline"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs_main"),
				buffers: &[],
				compilation_options: wgpu::PipelineCompilationOptions::default(),
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: Some("fs_main"),
				targets: &[Some(wgpu::ColorTargetState {
					format: wgpu::TextureFormat::Rgba8Unorm,
					blend: None,
					write_mask: wgpu::ColorWrites::ALL,
				})],
				compilation_options: wgpu::PipelineCompilationOptions::default(),
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				..Default::default()
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview_mask: None,
			cache: None,
		});

		PixelPreview { pipeline, bind_group_layout }
	}

	async fn run<'a>(&'a self, executor: &'a WgpuExecutor, args: &'a Self::Args<'_>) -> Self::Out {
		let context = &executor.context();
		let &ResamplerArgs { source, transform, size } = args;

		let output = executor.request_texture(size).await;

		let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
		let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());

		let params_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("resample_params"),
			size: 32,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
			mapped_at_creation: false,
		});

		let params_data = [transform.matrix2.x_axis.as_vec2(), transform.matrix2.y_axis.as_vec2(), transform.translation.as_vec2(), Vec2::ZERO];
		context.queue.write_buffer(&params_buffer, 0, bytemuck::cast_slice(&params_data));

		let bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("resample_bind_group"),
			layout: &self.bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&source_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: params_buffer.as_entire_binding(),
				},
			],
		});

		let mut encoder = context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("resample_encoder") });

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("resample_pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &output_view,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
						store: wgpu::StoreOp::Store,
					},
					depth_slice: None,
				})],
				..Default::default()
			});

			render_pass.set_pipeline(&self.pipeline);
			render_pass.set_bind_group(0, &bind_group, &[]);
			render_pass.draw(0..3, 0..1);
		}

		context.queue.submit([encoder.finish()]);

		output
	}
}
