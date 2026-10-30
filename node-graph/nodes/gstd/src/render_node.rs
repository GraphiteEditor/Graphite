use core_types::list::{Item, List};
use core_types::ops::ConvertAsync;
use core_types::transform::{Footprint, Transform};
use core_types::{CloneVarArgs, ExtractAll, ExtractVarArgs};
use core_types::{Color, Context, Ctx, ExtractFootprint, Interrupt, OwnedContextImpl, WasmNotSend};
use graph_craft::document::value::{RenderOutput, RenderOutputType};
use graphene_application_io::{ExportFormat, RenderConfig};
use graphic_types::raster_types::{CPU, GPU, Raster, Texture};
use graphic_types::{Artboard, Graphic, Vector};
use rendering::vello_scene_builder::VelloSceneBuilder;
use rendering::{Render, RenderMetadata, RenderOutputType as RenderOutputTypeRequest, RenderParams, SvgRender, SvgRenderOutput};
use std::collections::HashMap;
use std::sync::Arc;
use vector_types::Gradient;
use wgpu_executor::WgpuExecutor;

#[derive(Clone, dyn_any::DynAny)]
pub struct RenderIntermediate {
	/// The content itself, drawn at each render since a Vello scene depends on the render's size and placement and SVG output on the GPU raster downloads
	pub(crate) content: Arc<dyn Render + Send + Sync>,
	/// Images of the content's GPU raster textures, downloaded only for SVG output, which can't reference textures
	pub(crate) gpu_raster_downloads: Arc<HashMap<Texture, Raster<CPU>>>,
	pub(crate) metadata: RenderMetadata,
}

#[node_macro::node(category(""))]
fn render_intermediate<T: 'static + Render + WasmNotSend + Send + Sync>(
	ctx: impl Ctx + ExtractVarArgs + ExtractAll + CloneVarArgs,
	#[implementations(
		Context -> List<Artboard>,
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<GPU>>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> List<String>,
	)]
	data: impl Node<Context<'static>, Output = T>,
) -> Result<Item<RenderIntermediate>, Interrupt> {
	let ctx = OwnedContextImpl::from(ctx.clone()).into_context();
	let data = data.eval(ctx)?;

	let footprint = Footprint::default();
	let mut metadata = RenderMetadata::default();
	data.collect_metadata(&mut metadata, footprint, None, None);
	let intermediate = RenderIntermediate {
		content: Arc::new(data),
		gpu_raster_downloads: Arc::default(),
		metadata,
	};

	Ok(Item::new_from_element(intermediate))
}

/// Downloads the content's GPU raster textures for SVG output to embed, leaving them undrawn without a GPU executor.
#[node_macro::node(category(""), placeholder = pending_gpu_raster_downloads)]
async fn download_gpu_rasters(
	ctx: impl Ctx + ExtractVarArgs,
	#[scope(crate::platform_application_io::try_wgpu_executor::IDENTIFIER)] executor: Item<Option<WgpuExecutor>>,
	mut data: Item<RenderIntermediate>,
) -> Item<RenderIntermediate> {
	let render_output_type = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type")
		.render_output_type;
	let (RenderOutputTypeRequest::Svg, Some(executor)) = (render_output_type, executor.into_element()) else {
		return data;
	};

	let mut textures = Vec::new();
	data.element().content.collect_gpu_textures(&mut textures);
	if textures.is_empty() {
		return data;
	}

	let rasters: List<Raster<GPU>> = textures.iter().map(|texture| Item::new_from_element(Raster::new_gpu(texture.clone()))).collect();
	let downloads = rasters.convert(Footprint::DEFAULT, executor).await;

	data.element_mut().gpu_raster_downloads = Arc::new(textures.into_iter().zip(downloads.into_iter().map(Item::into_element)).collect());
	data
}

fn pending_gpu_raster_downloads(_executor: &Item<Option<WgpuExecutor>>, data: &Item<RenderIntermediate>) -> Item<RenderIntermediate> {
	data.clone()
}

#[node_macro::node(category(""))]
fn render(
	ctx: impl Ctx + ExtractFootprint + ExtractVarArgs,
	#[scope(crate::platform_application_io::try_wgpu_executor::IDENTIFIER)] executor: Item<Option<WgpuExecutor>>,
	data: Item<RenderIntermediate>,
) -> Item<RenderOutput> {
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	let RenderIntermediate {
		content,
		gpu_raster_downloads,
		mut metadata,
	} = data.into_element();
	metadata.apply_transform(footprint.transform);

	let data = match render_params.render_output_type {
		RenderOutputTypeRequest::Svg => {
			let render_params = RenderParams {
				gpu_raster_downloads,
				..render_params.clone()
			};
			let logical_transform = glam::DAffine2::from_scale(glam::DVec2::splat(1.0 / render_params.scale)) * footprint.transform;
			let logical_resolution = footprint.resolution.as_dvec2() / render_params.scale;

			let mut render = SvgRender::new();
			content.render_svg(&mut render, &render_params);
			render.wrap_with_transform(logical_transform, Some(logical_resolution));

			let output = SvgRenderOutput::from(render);
			assert!(output.svg_defs.is_empty());

			RenderOutputType::Svg {
				svg: output.svg,
				image_data: output.image_data.into_iter().collect(),
			}
		}
		RenderOutputTypeRequest::Vello => {
			let render_params = RenderParams {
				footprint: *footprint,
				..render_params.clone()
			};
			let executor = executor.into_element().expect("GPU executor not available");

			let mut draw = |scene: &mut VelloSceneBuilder| content.render_to_vello(scene, Default::default(), &render_params);
			let texture = match executor.render_vello(footprint.resolution, footprint.transform, &mut draw) {
				Ok(texture) => texture,
				// Leave the output transparent, at a size the GPU can hold, rather than taking down the editor
				Err(error) => {
					log::error!("Failed to render Vello scene: {error}");
					let limit = glam::UVec2::splat(executor.context().device.limits().max_texture_dimension_2d);
					executor
						.render_vello(footprint.resolution.min(limit), footprint.transform, &mut |_| {})
						.expect("Failed to render an empty Vello scene")
				}
			};
			RenderOutputType::Texture(texture)
		}
	};

	Item::new_from_element(RenderOutput { data, metadata })
}

#[node_macro::node(category(""))]
fn create_context(
	// The executor boundary supplies the render config as the sole vararg (see `wrap_network_in_scope()`)
	ctx: impl Ctx + ExtractAll + CloneVarArgs + Sync,
	data: impl Node<Context<'static>, Output = Item<RenderOutput>>,
) -> Result<Item<RenderOutput>, Interrupt> {
	let render_config = ctx.vararg(0).ok().and_then(|config| config.downcast_ref::<RenderConfig>()).copied().unwrap_or_else(|| {
		log::error!("The boundary context is missing its render config vararg");
		RenderConfig::default()
	});

	let render_output_type = match render_config.export_format {
		ExportFormat::Svg => RenderOutputTypeRequest::Svg,
		ExportFormat::Raster => RenderOutputTypeRequest::Vello,
	};

	let logical_viewport = render_config.viewport;
	let footprint = Footprint {
		transform: glam::DAffine2::from_scale(glam::DVec2::splat(render_config.scale)) * logical_viewport.transform,
		..logical_viewport
	};

	// Rounded up so no image is sent below its on-screen resolution, with a tolerance for float noise at exact powers of two
	let canvas_image_scale = match render_output_type {
		RenderOutputTypeRequest::Svg => (footprint.scale_magnitudes().max_element().log2() - 1e-9).ceil().exp2(),
		RenderOutputTypeRequest::Vello => 0.,
	};

	let render_params = RenderParams {
		render_mode: render_config.render_mode,
		for_export: render_config.for_export,
		render_output_type,
		scale: render_config.scale,
		viewport_zoom: Some(logical_viewport.scale_magnitudes().x),
		canvas_image_scale,
		..Default::default()
	};

	let ctx = OwnedContextImpl::default()
		.with_footprint(footprint)
		.with_real_time(render_config.time.time)
		.with_animation_time(render_config.time.animation_time.as_secs_f64())
		.with_pointer_position(render_config.pointer)
		.with_vararg(Box::new(render_params))
		.with_generations(ctx.try_generations().cloned().unwrap_or_default())
		.into_context();

	let mut result = data.eval(ctx)?;

	result.element_mut().metadata.apply_transform(glam::DAffine2::from_scale(glam::DVec2::splat(1. / render_config.scale)));
	Ok(result)
}
