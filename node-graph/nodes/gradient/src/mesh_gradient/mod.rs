mod pipeline;
mod tessellate;

// FIXME: create MeshGradientCache type
use brush_types::BrushCache;

use core_types::{
	ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPACE, ATTR_TRANSFORM, Ctx, ExtractFootprint, ExtractPaintRenderParams,
	bounds::{BoundingBox, RenderBoundingBox},
	list::{ATTR_TEXTURE, Item},
	math::bbox::AxisAlignedBbox,
	transform::Footprint,
};
use glam::{DAffine2, DVec2, UVec2};
use vector_types::mesh_gradient::{MeshGradient, MeshGradientCache, MeshGradientEvaluator};
use vector_types::{GradientInterpolation, GradientSpace};
use wgpu_executor::{WgpuExecutor, WgpuPipelineCache};

use crate::mesh_gradient::{
	pipeline::{MeshGradientPipeline, MeshGradientPipelineArgs},
	tessellate::{MeshGradientTessellator, Metadata},
};

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
	#[widget(ParsedWidgetOverride::Hidden)] mesh_gradient_cache: Item<MeshGradientCache>,
	#[scope(mesh_gradient_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Item<MeshGradient> {
	// FIXME: debug
	let debug = *debug.element();
	let pipeline = pipeline.into_element();
	let (cache, mesh_gradient_cache) = (cache.into_element(), mesh_gradient_cache.into_element());

	let mut mesh_gradient_item = mesh_gradient;
	let mesh_gradient = mesh_gradient_item.element();

	let footprint = *ctx.footprint();
	let Some(paint_to_target) = ctx.paint_render_params().fallback_paint_to_target else {
		return Item::default();
	};
	let mesh_to_target = ctx.paint_render_params().fallback_paint_to_target.unwrap_or_default();

	let interpolation_space: GradientSpace = mesh_gradient_item.attribute_cloned_or_default(ATTR_GRADIENT_SPACE);
	let interpolation_method: GradientInterpolation = mesh_gradient_item.attribute_cloned_or_default(ATTR_GRADIENT_INTERPOLATION);

	let args = MeshGradientPipelineArgs {
		footprint,
		mesh_to_target,
		mesh_gradient,
		interpolation_space,
		interpolation_method,
		cache: &cache,
		mesh_gradient_cache: &mesh_gradient_cache,
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
