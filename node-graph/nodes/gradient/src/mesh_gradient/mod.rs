mod pipeline;
mod tessellate;

use std::array;

use core_types::{
	ATTR_TRANSFORM, Ctx, ExtractFootprint, ExtractPaintRenderParams,
	bounds::{BoundingBox, RenderBoundingBox},
	list::{ATTR_TEXTURE, Item},
	math::bbox::AxisAlignedBbox,
	transform::Footprint,
};
use glam::{DAffine2, DVec2, UVec2};
use vector_types::{GradientInterpolation, GradientSpace, MeshGradient, mesh_gradient::MeshPatchInterpolation};
use wgpu_executor::{WgpuExecutor, WgpuPipelineCache};

use crate::mesh_gradient::{
	pipeline::{MeshGradientPipeline, MeshGradientPipelineArgs},
	tessellate::{InterpolationSetting, MeshGradientTessellator, PatchData},
};

const MAX_RESOLUTION: u32 = 8192;

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

	let mesh_to_target = ctx.paint_render_params().fallback_paint_to_target.unwrap_or_default();
	let mesh_to_output = ctx.footprint().transform * mesh_to_target;
	let Some((texture_to_output, texture_size)) = calc_texture_to_output(mesh_gradient, mesh_to_output, *ctx.footprint()) else {
		return Item::default();
	};
	if !texture_to_output.matrix2.determinant().recip().is_finite() {
		return Item::default();
	}
	let mesh_to_texture = texture_to_output.inverse() * mesh_to_output;
	// Need to offset the paint target's transform to prevent duplicated application
	let texture_transform = ctx.footprint().transform.inverse() * texture_to_output;

	let tessellator_result = MeshGradientTessellator::try_new(mesh_gradient, GradientSpace::RgbGamma, GradientInterpolation::Smooth, mesh_to_texture, mesh_to_output);
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

	if vertices.is_empty() {
		return Item::default();
	}

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
		output_size: texture_size,
		vertices: &vertices,
		indices: &indices,
		patches: &patches,
		interpolation_setting: &InterpolationSetting { space: 0, method: 2 },
		debug,
	};

	let Some(texture) = pipeline.run::<MeshGradientPipeline>(&args).await else {
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

fn calc_texture_to_output(mesh_gradient: &MeshGradient, mesh_to_output: DAffine2, footprint: Footprint) -> Option<(DAffine2, UVec2)> {
	let mesh_bbox = mesh_gradient.bounding_box(mesh_to_output, false);
	let mesh_aabb = match mesh_bbox {
		RenderBoundingBox::Rectangle([start, end]) => AxisAlignedBbox::from((start, end)),
		RenderBoundingBox::None | RenderBoundingBox::Infinite => return None,
	};
	let output_aabb = AxisAlignedBbox::from((DVec2::ZERO, footprint.resolution.as_dvec2()));
	let mut crop_aabb = mesh_aabb.intersect(&output_aabb);

	// Cast the texture size to integer by expanding the size
	crop_aabb.start = crop_aabb.start.floor();
	crop_aabb.end = crop_aabb.end.ceil();

	let crop_size = crop_aabb.size();
	if crop_size.x <= 0. || crop_size.y <= 0. {
		return None;
	}
	let texture_to_output = DAffine2::from_scale_angle_translation(crop_size, 0., crop_aabb.start);
	let texture_scale = ((MAX_RESOLUTION as f64) / crop_size.x.max(crop_size.y)).min(1.);
	let texture_size = (crop_size * texture_scale).ceil().max(DVec2::ONE).min(DVec2::splat(MAX_RESOLUTION as f64)).as_uvec2();

	Some((texture_to_output, texture_size))
}
