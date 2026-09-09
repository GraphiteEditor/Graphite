mod pipeline;
mod tessellate;

use core_types::{
	ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPACE, ATTR_TRANSFORM, Ctx, ExtractFootprint, ExtractPaintRenderParams,
	bounds::{BoundingBox, RenderBoundingBox},
	list::{ATTR_TEXTURE, Item},
	math::bbox::AxisAlignedBbox,
	transform::Footprint,
};
use glam::{DAffine2, DVec2, UVec2};
use vector_types::{GradientInterpolation, GradientSpace, MeshGradient, gradient::MeshGradientEvaluator};
use wgpu_executor::{WgpuExecutor, WgpuPipelineCache};

use crate::mesh_gradient::{
	pipeline::{MeshGradientPipeline, MeshGradientPipelineArgs},
	tessellate::{MeshGradientTessellator, Metadata},
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
	let pipeline = pipeline.into_element();

	let mut mesh_gradient_item = mesh_gradient;
	let mesh_gradient = mesh_gradient_item.element();

	let interpolation_space: GradientSpace = mesh_gradient_item.attribute_cloned_or_default(ATTR_GRADIENT_SPACE);
	let interpolation_method: GradientInterpolation = mesh_gradient_item.attribute_cloned_or_default(ATTR_GRADIENT_INTERPOLATION);

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
	let color_data = pack_color_data(&evaluator, interpolation_method);

	let Some(interpolation_space) = try_interpolation_space_to_u32(interpolation_space) else {
		return Item::default();
	};
	let interpolation_method = interpolation_method_to_u32(interpolation_method);

	let args = MeshGradientPipelineArgs {
		output_size: texture_size,
		vertices: &vertices,
		indices: &indices,
		color_data: color_data.as_slice(),
		metadata: &Metadata {
			patch_count: evaluator.patches().count() as u32,
			interpolation_space,
			interpolation_method,
		},
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

pub(crate) fn try_interpolation_space_to_u32(space: GradientSpace) -> Option<u32> {
	match space {
		GradientSpace::RgbGamma => Some(0),
		GradientSpace::RgbLinear => Some(1),
		GradientSpace::OkLab => Some(2),
		GradientSpace::Lab => Some(3),
		_ => None,
	}
}

pub(crate) fn interpolation_method_to_u32(method: GradientInterpolation) -> u32 {
	match method {
		GradientInterpolation::Stepped => 0,
		GradientInterpolation::Linear => 1,
		GradientInterpolation::Smooth => 2,
	}
}

pub(crate) fn pack_color_data(evaluator: &MeshGradientEvaluator, interpolation_method: GradientInterpolation) -> Vec<[f32; 4]> {
	match interpolation_method {
		GradientInterpolation::Stepped => evaluator.patches().map(|patch| patch.colors()[0].to_array()).collect::<Vec<_>>(),
		GradientInterpolation::Linear => evaluator.patches().flat_map(|patch| patch.colors().map(|color| color.to_array())).collect::<Vec<_>>(),
		GradientInterpolation::Smooth => evaluator.patches().flat_map(|patch| patch.color_bezier_net().to_array()).collect::<Vec<_>>(),
	}
}
