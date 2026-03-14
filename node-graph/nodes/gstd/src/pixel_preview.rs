use crate::render_node::RenderOutputType;
use core_types::transform::{Footprint, Transform};
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, OwnedContextImpl};
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::document::value::RenderOutput;
use graph_craft::wasm_application_io::WasmEditorApi;
use graphene_application_io::{ApplicationIo, ImageTexture};
use rendering::{RenderOutputType as RenderOutputTypeRequest, RenderParams};
use vector_types::vector::style::RenderMode;

#[node_macro::node(category(""))]
pub async fn pixel_preview<'a: 'n>(
	ctx: impl Ctx + ExtractAll + CloneVarArgs + Sync,
	editor_api: &'a WasmEditorApi,
	data: impl Node<Context<'static>, Output = RenderOutput> + Send + Sync,
) -> RenderOutput {
	let Some(render_params) = ctx.vararg(0).ok().and_then(|v| v.downcast_ref::<RenderParams>()).cloned() else {
		log::error!("invalid render params for pixel preview");
		let context = OwnedContextImpl::from(ctx).into_context();
		return data.eval(context).await;
	};
	let physical_scale = render_params.scale;

	let footprint = *ctx.footprint();
	let viewport_zoom = footprint.decompose_scale().x * physical_scale;

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
		transform: DAffine2::from_scale(DVec2::splat(1.0 / physical_scale)) * DAffine2::from_translation(-upstream_min),
		resolution: upstream_resolution,
		quality: footprint.quality,
	};

	let new_ctx = OwnedContextImpl::from(ctx).with_footprint(upstream_footprint).with_vararg(Box::new(render_params)).into_context();
	let mut result = data.eval(new_ctx).await;

	let RenderOutputType::Texture(ref source_texture) = result.data else { return result };

	let transform = DAffine2::from_translation(-upstream_min) * footprint.transform.inverse() * DAffine2::from_scale(logical_resolution);

	let exec = editor_api.application_io.as_ref().unwrap().gpu_executor().unwrap();
	let resampled = exec.resample_texture(&source_texture.texture, physical_resolution, &transform);

	result.data = RenderOutputType::Texture(ImageTexture { texture: resampled.into() });

	result
		.metadata
		.apply_transform(footprint.transform * DAffine2::from_translation(upstream_min) * DAffine2::from_scale(DVec2::splat(physical_scale)));

	result
}
