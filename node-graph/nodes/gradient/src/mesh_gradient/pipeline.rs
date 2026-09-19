use std::sync::Arc;

use brush_types::BrushCache;
use core_types::transform::Footprint;
use glam::DAffine2;
use raster_types::Texture;
use vector_types::mesh_gradient::MeshGradientEvaluator;
use wgpu_executor::AsyncWgpuPipeline;

use crate::mesh_gradient::{
	MeshGradientTessellator,
	cache::{CachedTexture, EvaluatorKey, texture_key},
	renderer::Renderer,
	tessellator::Metadata,
	utils::{interpolation_method_to_u32, pack_color_data, transforms, try_interpolation_space_to_u32},
};

pub struct MeshGradientPipeline {
	renderer: Renderer,
}

pub struct MeshGradientPipelineArgs<'a> {
	pub footprint: Footprint,
	pub mesh_to_target: DAffine2,
	pub mesh_gradient_evaluator: Arc<MeshGradientEvaluator>,
	pub evaluator_key: EvaluatorKey,
	pub cache: &'a BrushCache,
	pub debug: bool,
}

impl AsyncWgpuPipeline for MeshGradientPipeline {
	type Args<'a> = MeshGradientPipelineArgs<'a>;
	type Out = Option<(Texture, DAffine2)>;

	fn create(executor: &wgpu_executor::WgpuExecutor) -> Self {
		let device = &executor.context().device;
		Self { renderer: Renderer::new(device) }
	}

	async fn run<'a>(&'a self, executor: &'a wgpu_executor::WgpuExecutor, args: &'a Self::Args<'_>) -> Self::Out {
		let texture_key = texture_key(args.evaluator_key, args.mesh_to_target);

		if let Some(cached_texture) = args.cache.get_cloned::<CachedTexture>(&args.footprint)
			&& cached_texture.key == texture_key
		{
			let texture = cached_texture.texture.upgrade().unwrap();
			return Some((texture, cached_texture.transform));
		};

		let evaluator_ref = args.mesh_gradient_evaluator.as_ref();
		let (mesh_to_output, mesh_to_texture, texture_transform, texture_size) = transforms(evaluator_ref, args.footprint, args.mesh_to_target)?;
		let tessellator = MeshGradientTessellator::new(evaluator_ref, mesh_to_texture, mesh_to_output);

		// TODO: Possibly the quadtree can be cached too and continue the tessellation from there.
		let (vertices, indices) = match tessellator.tessellate() {
			Ok(result) => result,
			Err(error) => {
				log::error!("Failed to tessellate mesh gradient: {error:?}");
				return None;
			}
		};

		if vertices.is_empty() {
			return None;
		};

		let color_data = pack_color_data(evaluator_ref, evaluator_ref.interpolation_method());

		let metadata = Metadata {
			patch_count: evaluator_ref.patches().count() as u32,
			interpolation_space: try_interpolation_space_to_u32(evaluator_ref.space())?,
			interpolation_method: interpolation_method_to_u32(evaluator_ref.interpolation_method()),
		};

		let rendered_texture = self
			.renderer
			.render(executor, texture_size, vertices.as_slice(), indices.as_slice(), &color_data, metadata, args.debug)?;

		args.cache.store(
			&args.footprint,
			CachedTexture {
				key: texture_key,
				transform: texture_transform,
				texture: rendered_texture.downgrade(),
			},
		);

		Some((rendered_texture, texture_transform))
	}
}
