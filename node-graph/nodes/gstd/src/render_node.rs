use core_types::table::Table;
use core_types::transform::{Footprint, Transform};
use core_types::{CloneVarArgs, ExtractAll, ExtractVarArgs};
use core_types::{Color, Context, Ctx, ExtractFootprint, OwnedContextImpl, WasmNotSend};
pub use graph_craft::application_io::*;
use graph_craft::document::value::RenderOutput;
pub use graph_craft::document::value::RenderOutputType;
use graphene_application_io::{ApplicationIo, ExportFormat, RenderConfig};
use graphic_types::raster_types::Image;
use graphic_types::raster_types::{CPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use rendering::{Render, RenderBackground, RenderMetadata, RenderOutputType as RenderOutputTypeRequest, RenderParams, RenderSvgSegmentList, SvgRender, SvgSegment};
use std::collections::HashMap;
use std::sync::Arc;
use vector_types::GradientStops;
use wgpu_executor::RenderContext;

// Re-export render_output_cache from render_cache module
pub use crate::render_cache::render_output_cache;

/// List of (canvas id, image data) pairs for embedding images as canvases in the final SVG string.
type ImageData = HashMap<core_types::graphene_hash::CacheHashWrapper<Image<Color>>, u64>;

#[derive(Clone, dyn_any::DynAny)]
pub enum RenderIntermediateType {
	Vello(Arc<(vello::Scene, RenderContext)>),
	Svg(Arc<(String, ImageData, String)>),
}
#[derive(Clone, dyn_any::DynAny)]
pub struct RenderIntermediate {
	pub(crate) ty: RenderIntermediateType,
	pub(crate) metadata: RenderMetadata,
}

#[node_macro::node(category(""))]
async fn render_intermediate<'a: 'n, T: 'static + Render + WasmNotSend + Send + Sync>(
	ctx: impl Ctx + ExtractVarArgs + ExtractAll + CloneVarArgs,
	#[implementations(
		Context -> Table<Artboard>,
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	data: impl Node<Context<'static>, Output = T>,
) -> RenderIntermediate {
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	let ctx = OwnedContextImpl::from(ctx.clone()).into_context();
	let data = data.eval(ctx).await;

	let footprint = Footprint::default();
	let mut metadata = RenderMetadata::default();
	data.collect_metadata(&mut metadata, footprint, None);
	match &render_params.render_output_type {
		RenderOutputTypeRequest::Vello => {
			let mut scene = vello::Scene::new();

			let mut context = wgpu_executor::RenderContext::default();
			data.render_to_vello(&mut scene, Default::default(), &mut context, render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Vello(Arc::new((scene, context))),
				metadata,
			}
		}
		RenderOutputTypeRequest::Svg => {
			let mut render = SvgRender::new();

			data.render_svg(&mut render, render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Svg(Arc::new((render.svg.to_svg_string(), render.image_data, render.svg_defs.clone()))),
				metadata,
			}
		}
	}
}

#[node_macro::node(category(""))]
async fn render_background_intermediate<'a: 'n, T: 'static + RenderBackground + WasmNotSend + Send + Sync>(
	ctx: impl Ctx + ExtractFootprint + ExtractVarArgs + ExtractAll + CloneVarArgs,
	#[implementations(
		Context -> Table<Artboard>,
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	data: impl Node<Context<'static>, Output = T>,
) -> RenderIntermediate {
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");
	let mut render_params = render_params.clone();
	render_params.footprint = *footprint;
	let render_params = &render_params;

	let ctx = OwnedContextImpl::from(ctx.clone()).into_context();
	let data = data.eval(ctx).await;

	match &render_params.render_output_type {
		RenderOutputTypeRequest::Vello => {
			let mut scene = vello::Scene::new();

			let mut context = wgpu_executor::RenderContext::default();
			data.render_background_to_vello(&mut scene, Default::default(), &mut context, render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Vello(Arc::new((scene, context))),
				metadata: RenderMetadata::default(),
			}
		}
		RenderOutputTypeRequest::Svg => {
			let mut render = SvgRender::new();

			data.render_background_svg(&mut render, render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Svg(Arc::new((render.svg.to_svg_string(), render.image_data, render.svg_defs.clone()))),
				metadata: RenderMetadata::default(),
			}
		}
	}
}

#[node_macro::node(category(""))]
async fn render<'a: 'n>(ctx: impl Ctx + ExtractFootprint + ExtractVarArgs, editor_api: &'a PlatformEditorApi, data: RenderIntermediate) -> RenderOutput {
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");
	let mut render_params = render_params.clone();
	render_params.footprint = *footprint;
	let render_params = &render_params;

	let scale = render_params.scale;
	let physical_resolution = render_params.footprint.resolution;
	let logical_resolution = render_params.footprint.resolution.as_dvec2() / scale;

	let RenderIntermediate { ty, mut metadata } = data;
	metadata.apply_transform(footprint.transform);

	let data = match (render_params.render_output_type, &ty) {
		(RenderOutputTypeRequest::Svg, RenderIntermediateType::Svg(svg_data)) => {
			let mut rendering = SvgRender::new();
			rendering.svg.push(SvgSegment::from(svg_data.0.clone()));
			rendering.image_data = svg_data.1.clone();
			rendering.svg_defs = svg_data.2.clone();

			rendering.wrap_with_transform(footprint.transform, Some(logical_resolution));
			RenderOutputType::Svg {
				svg: rendering.svg.to_svg_string(),
				image_data: rendering.image_data.into_iter().map(|(image, id)| (id, image.0)).collect(),
			}
		}
		(RenderOutputTypeRequest::Vello, RenderIntermediateType::Vello(vello_data)) => {
			let Some(exec) = editor_api.application_io.as_ref().unwrap().gpu_executor() else {
				unreachable!("Attempted to render with Vello when no GPU executor is available");
			};
			let (child, context) = Arc::as_ref(vello_data);

			let scale_transform = glam::DAffine2::from_scale(glam::DVec2::splat(scale));
			let footprint_transform = scale_transform * footprint.transform;
			let footprint_transform_vello = vello::kurbo::Affine::new(footprint_transform.to_cols_array());

			let mut scene = vello::Scene::new();
			scene.append(child, Some(footprint_transform_vello));

			// We now replace all transforms which are supposed to be infinite with a transform which covers the entire viewport
			// See <https://xi.zulipchat.com/#narrow/channel/197075-vello/topic/Full.20screen.20color.2Fgradients/near/538435044> for more detail
			let scaled_infinite_transform = vello::kurbo::Affine::scale_non_uniform(physical_resolution.x as f64, physical_resolution.y as f64);
			for transform in scene.encoding_mut().transforms.iter_mut() {
				if transform.matrix[0] == f32::INFINITY {
					*transform = vello_encoding::Transform::from_kurbo(&scaled_infinite_transform);
				}
			}

			let texture = Arc::new(
				exec.render_vello_scene_to_texture(&scene, physical_resolution, context, None)
					.await
					.expect("Failed to render Vello scene"),
			);

			RenderOutputType::Texture(texture.into())
		}
		_ => unreachable!("Render node did not receive its requested data type"),
	};
	RenderOutput { data, metadata }
}

#[node_macro::node(category(""))]
async fn compose<'a: 'n>(
	ctx: impl Ctx + ExtractVarArgs + ExtractAll + CloneVarArgs,
	editor_api: &'a PlatformEditorApi,
	data: impl Node<Context<'static>, Output = RenderOutput>,
	background: impl Node<Context<'static>, Output = RenderOutput>,
) -> RenderOutput {
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	let eval_ctx = OwnedContextImpl::from(ctx.clone()).into_context();
	let artwork = data.eval(eval_ctx.clone()).await;

	if render_params.for_export {
		return artwork;
	}

	let background = background.eval(eval_ctx).await;
	let RenderOutput { data: foreground_data, metadata } = artwork;

	let data = match (foreground_data, background.data) {
		(RenderOutputType::Texture(foreground_texture), RenderOutputType::Texture(background_texture)) => {
			let Some(exec) = editor_api.application_io.as_ref().unwrap().gpu_executor() else {
				return RenderOutput {
					data: RenderOutputType::Texture(foreground_texture),
					metadata,
				};
			};

			let blended = exec.blend_textures(foreground_texture.as_ref(), background_texture.as_ref());
			RenderOutputType::Texture(blended.into())
		}
		(
			RenderOutputType::Svg {
				svg: foreground_svg,
				image_data: foreground_images,
			},
			RenderOutputType::Svg {
				svg: background_svg,
				image_data: background_images,
			},
		) => {
			let mut image_data = background_images;
			image_data.extend(foreground_images);

			RenderOutputType::Svg {
				svg: format!("{background_svg}{foreground_svg}"),
				image_data,
			}
		}
		(foreground_data, _) => foreground_data,
	};

	RenderOutput { data, metadata }
}

#[node_macro::node(category(""))]
async fn create_context<'a: 'n>(
	// Context injections are defined in the wrap_network_in_scope function
	render_config: RenderConfig,
	data: impl Node<Context<'static>, Output = RenderOutput>,
) -> RenderOutput {
	let footprint = render_config.viewport;

	let render_output_type = match render_config.export_format {
		ExportFormat::Svg => RenderOutputTypeRequest::Svg,
		ExportFormat::Raster => RenderOutputTypeRequest::Vello,
	};

	let render_params = RenderParams {
		render_mode: render_config.render_mode,
		for_export: render_config.for_export,
		render_output_type,
		footprint: Footprint::BOUNDLESS,
		scale: render_config.scale,
		viewport_zoom: footprint.scale_magnitudes().x,
		..Default::default()
	};

	let ctx = OwnedContextImpl::default()
		.with_footprint(footprint)
		.with_real_time(render_config.time.time)
		.with_animation_time(render_config.time.animation_time.as_secs_f64())
		.with_pointer_position(render_config.pointer)
		.with_vararg(Box::new(render_params))
		.into_context();

	data.eval(ctx).await
}
