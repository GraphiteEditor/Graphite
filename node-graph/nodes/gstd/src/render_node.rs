use core_types::list::{Item, List};
use core_types::transform::{Footprint, Transform};
use core_types::{CloneVarArgs, ExtractAll, ExtractVarArgs};
use core_types::{Color, Context, Ctx, ExtractFootprint, Interrupt, OwnedContextImpl, WasmNotSend};
use graph_craft::document::value::{RenderOutput, RenderOutputType};
use graphene_application_io::{ExportFormat, RenderConfig};
use graphic_types::raster_types::{CPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use rendering::scene::Scene;
use rendering::{Render, RenderMetadata, RenderOutputType as RenderOutputTypeRequest, RenderParams, SvgRender, SvgRenderOutput};
use std::sync::Arc;
use vector_types::Gradient;
use wgpu_executor::WgpuExecutor;

#[derive(Clone, dyn_any::DynAny)]
pub enum RenderIntermediateType {
	Vello(Arc<Scene>),
	Svg(Arc<SvgRenderOutput>),
}
#[derive(Clone, dyn_any::DynAny)]
pub struct RenderIntermediate {
	pub(crate) ty: RenderIntermediateType,
	pub(crate) metadata: RenderMetadata,
}

#[node_macro::node(category(""))]
fn render_intermediate<T: 'static + Render + WasmNotSend + Send + Sync>(
	ctx: impl Ctx + ExtractVarArgs + ExtractAll + CloneVarArgs,
	#[implementations(
		Context -> List<Artboard>,
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> List<String>,
	)]
	data: impl Node<Context<'static>, Output = T>,
) -> Result<Item<RenderIntermediate>, Interrupt> {
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	let ctx = OwnedContextImpl::from(ctx.clone()).into_context();
	let data = data.eval(ctx)?;

	let footprint = Footprint::default();
	let mut metadata = RenderMetadata::default();
	data.collect_metadata(&mut metadata, footprint, None, None);
	let intermediate = match &render_params.render_output_type {
		RenderOutputTypeRequest::Vello => {
			let mut scene = Scene::new();
			data.render_to_vello(&mut scene, Default::default(), render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Vello(Arc::new(scene)),
				metadata,
			}
		}
		RenderOutputTypeRequest::Svg => {
			let mut render = SvgRender::new();

			data.render_svg(&mut render, render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Svg(Arc::new(render.into())),
				metadata,
			}
		}
	};

	Ok(Item::new_from_element(intermediate))
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
	let mut render_params = render_params.clone();
	render_params.footprint = *footprint;

	let RenderIntermediate { ty, mut metadata } = data.into_element();
	metadata.apply_transform(footprint.transform);

	let data = match (render_params.render_output_type, ty) {
		(RenderOutputTypeRequest::Svg, RenderIntermediateType::Svg(data)) => {
			let logical_transform = glam::DAffine2::from_scale(glam::DVec2::splat(1.0 / render_params.scale)) * footprint.transform;
			let logical_resolution = footprint.resolution.as_dvec2() / render_params.scale;

			let mut render = SvgRender::from(data.as_ref());
			render.wrap_with_transform(logical_transform, Some(logical_resolution));

			let output = SvgRenderOutput::from(render);
			assert!(output.svg_defs.is_empty());

			RenderOutputType::Svg {
				svg: output.svg,
				image_data: output.image_data.into_iter().collect(),
			}
		}
		(RenderOutputTypeRequest::Vello, RenderIntermediateType::Vello(scene)) => {
			let executor = executor.into_element().expect("GPU executor not available");
			let texture = match executor.render_vello_scene(&scene, footprint.transform, footprint.resolution) {
				Ok(texture) => texture,
				// Leave the output transparent, at a size the GPU can hold, rather than taking down the editor
				Err(error) => {
					log::error!("Failed to render Vello scene: {error}");
					let limit = glam::UVec2::splat(executor.context().device.limits().max_texture_dimension_2d);
					let empty_scene = wgpu_executor::Scene::new();
					executor
						.render_vello_scene(&empty_scene, footprint.transform, footprint.resolution.min(limit))
						.expect("Failed to render an empty Vello scene")
				}
			};
			RenderOutputType::Texture(texture)
		}
		_ => unreachable!("Render node did not receive its requested data type"),
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

	let render_params = RenderParams {
		render_mode: render_config.render_mode,
		for_export: render_config.for_export,
		render_output_type,
		scale: render_config.scale,
		viewport_zoom: Some(logical_viewport.scale_magnitudes().x),
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
