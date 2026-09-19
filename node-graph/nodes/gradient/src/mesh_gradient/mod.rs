mod cache;
mod pipeline;
mod renderer;
mod tessellator;
mod utils;

use std::sync::Arc;

// FIXME: create MeshGradientRenderCache type or make the BrushCache to FootprintCache
use brush_types::BrushCache;

use core_types::{
	ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPACE, ATTR_TRANSFORM, Ctx, ExtractFootprint, ExtractPaintRenderParams,
	list::{ATTR_TEXTURE, Item},
};
use vector_types::mesh_gradient::{MeshGradient, MeshGradientEvaluator};
use vector_types::{GradientInterpolation, GradientSpace};
use wgpu_executor::{WgpuExecutor, WgpuPipelineCache};

use crate::mesh_gradient::{
	cache::evaluator_cache_key,
	pipeline::{MeshGradientPipeline, MeshGradientPipelineArgs},
	tessellator::MeshGradientTessellator,
};

pub use crate::mesh_gradient::cache::MeshGradientEvaluatorCache;

/// Constructs a mesh gradient value composed of a grid of patches defined by colored corners and curved boundary segments.
#[node_macro::node(category("Value"))]
pub async fn mesh_gradient_value<'a: 'n>(
	ctx: impl Ctx + ExtractFootprint + ExtractPaintRenderParams,
	mesh_gradient: Item<MeshGradient>,
	/// Draw triangle outlines for debugging.
	#[name("Debug Outline")]
	#[default(false)]
	debug: Item<bool>,
	#[widget(ParsedWidgetOverride::Hidden)] cache: Item<BrushCache>,
	#[widget(ParsedWidgetOverride::Hidden)] evaluator_cache: Item<MeshGradientEvaluatorCache>,
	#[scope(mesh_gradient_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Item<MeshGradient> {
	// FIXME: debug
	let debug = *debug.element();
	let pipeline = pipeline.into_element();
	let (cache, evaluator_cache) = (&cache.into_element(), &evaluator_cache.into_element());

	let mut mesh_gradient_item = mesh_gradient;
	let mesh_gradient = mesh_gradient_item.element();

	let footprint = *ctx.footprint();
	let mesh_to_target = ctx.paint_render_params().fallback_paint_to_target.unwrap_or_default();

	let interpolation_space: GradientSpace = mesh_gradient_item.attribute_cloned_or_default(ATTR_GRADIENT_SPACE);
	let interpolation_method: GradientInterpolation = mesh_gradient_item.attribute_cloned_or_default(ATTR_GRADIENT_INTERPOLATION);

	let evaluator_key = evaluator_cache_key(mesh_gradient, interpolation_space, interpolation_method);
	let cached_evaluator = evaluator_cache.get_cloned::<Arc<MeshGradientEvaluator>>(&evaluator_key);

	let mesh_gradient_evaluator = match cached_evaluator {
		Some(cached) => cached,
		None => {
			let Some(fresh_evaluator) = mesh_gradient.evaluator(interpolation_space, interpolation_method).ok() else {
				return Item::default();
			};
			evaluator_cache.store(&evaluator_key, Arc::clone(&fresh_evaluator));
			fresh_evaluator
		}
	};

	let args = MeshGradientPipelineArgs {
		footprint,
		mesh_to_target,
		mesh_gradient_evaluator,
		evaluator_key,
		cache,
		debug,
	};

	let Some((texture, texture_transform)) = pipeline.run::<MeshGradientPipeline>(&args).await else {
		return Item::default();
	};
	let texture_item = Item::from(texture).with_attribute(ATTR_TRANSFORM, texture_transform);

	mesh_gradient_item.set_attribute(ATTR_TRANSFORM, mesh_to_target);
	mesh_gradient_item.set_attribute(ATTR_TEXTURE, Some(texture_item));

	mesh_gradient_item
}

#[node_macro::node(category(""), inject_scope)]
async fn mesh_gradient_pipeline<'a: 'n>(
	_ctx: impl Ctx,
	#[scope(ProtoNodeIdentifier::new("graphene_std::platform_application_io::WgpuExecutorNode"))] executor: Item<&'a WgpuExecutor>,
	#[data] pipeline: WgpuPipelineCache,
) -> Item<WgpuPipelineCache> {
	executor.into_element().pipeline_init::<MeshGradientPipeline>(pipeline);
	Item::new_from_element(pipeline.clone())
}
