mod cache;
mod pipeline;
mod renderer;
mod tessellator;
mod utils;

use std::sync::Arc;

use core_types::{
	ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPACE, ATTR_TRANSFORM, Ctx, ExtractFootprint, ExtractPaintRenderParams,
	list::{ATTR_TEXTURE, Item},
};
use glam::DVec2;
use vector_types::mesh_gradient::{MeshGradient, MeshGradientEvaluator, mesh_gradient_to_bounds_transform};
use vector_types::{GradientInterpolation, GradientSpace};
use wgpu_executor::{WgpuExecutor, WgpuPipelineCache};

use crate::mesh_gradient::{
	cache::evaluator_cache_key,
	pipeline::{MeshGradientPipeline, MeshGradientPipelineArgs},
	tessellator::MeshGradientTessellator,
};

pub use crate::mesh_gradient::cache::{MeshGradientEvaluatorCache, MeshGradientOutputCache};

// TODO: Consider how to access attributes applied downstream of this value node.
// Attributes accumulated within the paint subgraph are unavailable here but are required to rasterize the mesh gradient at the correct output resolution.
// The current design invokes the scope-injected render pipeline from this value node, before attributes from downstream nodes are applied.
// For now, the Fill node provides the paint target bounds so this node can derive a bbox-fitting transform.
// The Mesh Gradient tool also updates the gradient space and interpolation directly instead of inserting the corresponding nodes.

/// Constructs a mesh gradient value composed of a grid of patches defined by colored corners and curved boundary segments.
#[node_macro::node(category("Value"))]
pub async fn mesh_gradient_value<'a: 'n>(
	ctx: impl Ctx + ExtractFootprint + ExtractPaintRenderParams,
	_primary: (),
	#[widget(ParsedWidgetOverride::Custom = "mesh_gradient_surface")] mesh_gradient: Item<MeshGradient>,
	#[widget(ParsedWidgetOverride::Hidden)] cache: Item<MeshGradientOutputCache>,
	#[widget(ParsedWidgetOverride::Hidden)] evaluator_cache: Item<MeshGradientEvaluatorCache>,
	#[scope(mesh_gradient_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Item<MeshGradient> {
	let pipeline = pipeline.into_element();
	let (output_cache, evaluator_cache) = (&cache.into_element(), &evaluator_cache.into_element());

	let mut mesh_gradient_item = mesh_gradient;
	let mesh_gradient = mesh_gradient_item.element();

	let footprint = *ctx.footprint();
	let target_bounds = ctx.try_paint_render_params().map(|paint_params| paint_params.paint_target_bounds).unwrap_or([DVec2::ZERO, DVec2::ONE]);
	let mesh_to_target = mesh_gradient_to_bounds_transform(target_bounds);

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
		output_cache,
	};

	let render_result = pipeline.run::<MeshGradientPipeline>(&args).await;
	let texture_item = render_result.map(|(texture, texture_transform)| Item::from(texture).with_attribute(ATTR_TRANSFORM, texture_transform));
	mesh_gradient_item.set_attribute(ATTR_TEXTURE, texture_item);
	mesh_gradient_item.set_attribute(ATTR_TRANSFORM, mesh_to_target);

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
