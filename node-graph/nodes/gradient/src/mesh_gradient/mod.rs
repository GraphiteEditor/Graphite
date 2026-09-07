mod pipeline;
mod tessellate;

use std::array;

use core_types::{
	ATTR_TRANSFORM, Ctx, ExtractFootprint, ExtractPaintRenderParams,
	list::{ATTR_TEXTURE, Item},
	transform::Transform,
};
use vector_types::{GradientInterpolation, GradientSpace, MeshGradient, mesh_gradient::MeshPatchInterpolation};
use wgpu_executor::{WgpuExecutor, WgpuPipelineCache};

use crate::mesh_gradient::{
	pipeline::{MeshGradientPipeline, MeshGradientPipelineArgs},
	tessellate::{InterpolationSetting, MeshGradientTessellator, PatchData},
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
	#[scope(mesh_gradient_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Item<MeshGradient> {
	// FIXME: debug
	let debug = *debug.element();

	let mut mesh_gradient_item = mesh_gradient;
	let mesh_gradient = mesh_gradient_item.element();
	let pipeline = pipeline.into_element();

	let paint_to_target = ctx.paint_render_params().fallback_paint_to_target.unwrap_or_default();
	let paint_to_output = ctx.footprint().transform * paint_to_target;
	let paint_to_output_scale = paint_to_output.scale_magnitudes();

	let tessellator_result = MeshGradientTessellator::try_new(mesh_gradient, GradientSpace::RgbGamma, GradientInterpolation::Smooth, paint_to_output);

	let tessellator = match tessellator_result {
		Ok(tessellator) => tessellator,
		Err(error) => {
			log::error!("Failed to create mesh gradient tessellator: {error:?}");
			return Item::default();
		}
	};

	let (vertices, indices) = match tessellator.tessellate() {
		Ok(result) => result,
		Err(error) => {
			log::error!("Failed to tessellate mesh gradient: {error:?}");
			return Item::default();
		}
	};

	let Some(evaluator) = mesh_gradient.evaluator(GradientSpace::RgbGamma, GradientInterpolation::Smooth).ok() else {
		return Item::default();
	};
	let patches = evaluator
		.patch_evaluators()
		.map(|patch| {
			let colors = patch.colors().map(|color| color.to_array());
			let [color_u_derivatives, color_v_derivatives] = match &patch.interpolation_method() {
				MeshPatchInterpolation::Stepped => todo!(),
				MeshPatchInterpolation::Linear => todo!(),
				MeshPatchInterpolation::Smooth {
					color_derivatives,
					color_bezier_net: _, // FIXME: mottainai!
				} => [array::from_fn(|i| color_derivatives[i].u.to_array()), array::from_fn(|i| color_derivatives[i].v.to_array())],
			};
			PatchData {
				colors,
				color_u_derivatives,
				color_v_derivatives,
			}
		})
		.collect::<Vec<_>>();

	let args = MeshGradientPipelineArgs {
		output_size: paint_to_output_scale.as_uvec2(),
		vertices: &vertices,
		indices: &indices,
		patches: &patches,
		interpolation_setting: &InterpolationSetting { space: 0, method: 2 },
		debug,
	};

	let Some(texture) = pipeline.run::<MeshGradientPipeline>(&args).await else {
		return Item::default();
	};

	mesh_gradient_item.set_attribute(ATTR_TRANSFORM, paint_to_target);
	mesh_gradient_item.set_attribute(ATTR_TEXTURE, Some(texture));

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
