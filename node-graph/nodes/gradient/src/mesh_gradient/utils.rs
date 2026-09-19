use core_types::{bounds::RenderBoundingBox, math::bbox::AxisAlignedBbox, transform::Footprint};
use glam::{DAffine2, DVec2, UVec2};
use vector_types::{GradientInterpolation, GradientSpace, mesh_gradient::MeshGradientEvaluator};

const MAX_RESOLUTION: u32 = 8192;

/// Returns the transforms and texture size required for mesh gradient rendering.
/// mesh_to_output: Transform from mesh gradient space to render output space
/// mesh_to_texture: Transform from mesh gradient space to normalized texture space
/// texture_transform: Transform from normalized texture space to paint target space
/// texture_size: Size of the render texture in pixels
pub(crate) fn transforms(mesh_gradient_evaluator: &MeshGradientEvaluator, footprint: Footprint, mesh_to_target: DAffine2) -> Option<(DAffine2, DAffine2, DAffine2, UVec2)> {
	let mesh_to_output = footprint.transform * mesh_to_target;
	let mesh_bbox = mesh_gradient_evaluator.bounding_box(mesh_to_output);
	let (texture_to_output, texture_size) = calc_texture_to_output(mesh_bbox, footprint)?;
	if !texture_to_output.matrix2.determinant().recip().is_finite() {
		return None;
	}
	let mesh_to_texture = texture_to_output.inverse() * mesh_to_output;
	// Need to offset the paint target's transform to prevent duplicated application
	let texture_transform = footprint.transform.inverse() * texture_to_output;
	Some((mesh_to_output, mesh_to_texture, texture_transform, texture_size))
}

/// Calculates the transform from normalized texture space to render output space and the texture size.
/// The texture covers the visible portion of the mesh gradient within the render output.
pub(crate) fn calc_texture_to_output(mesh_bbox: RenderBoundingBox, footprint: Footprint) -> Option<(DAffine2, UVec2)> {
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
