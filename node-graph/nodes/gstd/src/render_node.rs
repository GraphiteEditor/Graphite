mod svg_readback;

use core_types::list::{Item, List};
use core_types::transform::{Footprint, Transform};
use core_types::{CloneVarArgs, ExtractAll, ExtractVarArgs, Poll, RuntimeHandle, SourceId};
use core_types::{Color, Context, Ctx, ExtractFootprint, Interrupt, OwnedContextImpl, WasmNotSend};
use graph_craft::document::value::{RenderOutput, RenderOutputType};
use graphene_application_io::{ExportFormat, RenderConfig};
use graphic_types::Generator;
use graphic_types::raster_types::{CPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use rendering::vello_scene_builder::VelloSceneBuilder;
use rendering::{Render, RenderMetadata, RenderOutputType as RenderOutputTypeRequest, RenderParams, SvgRender, SvgRenderOutput};
use std::sync::Arc;
use svg_readback::SvgReadbackSlot;
use vector_types::Gradient;
use wgpu_executor::WgpuExecutor;

#[derive(Clone, dyn_any::DynAny)]
pub enum RenderIntermediateType {
	/// The content itself, drawn into a Vello scene at each render since the scene depends on the render's size and placement
	Vello(Arc<dyn Render + Send + Sync>),
	Svg(Arc<SvgRenderOutput>),
	/// The content itself, for content that draws only the part of it in view, drawn as SVG at each render for its view
	SvgInView(Arc<dyn Render + Send + Sync>),
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
		Context -> List<Generator>,
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
		RenderOutputTypeRequest::Vello => RenderIntermediate {
			ty: RenderIntermediateType::Vello(Arc::new(data)),
			metadata,
		},
		RenderOutputTypeRequest::Svg => {
			let render_params = RenderParams {
				view_wanted: Default::default(),
				..render_params.clone()
			};
			let mut render = SvgRender::new();

			data.render_svg(&mut render, &render_params);

			// Markup leaving out content that draws only the part of it in view is drawn again for each view, and the rest only once here
			let ty = match render_params.view_wanted.load(std::sync::atomic::Ordering::Relaxed) {
				true => RenderIntermediateType::SvgInView(Arc::new(data)),
				false => RenderIntermediateType::Svg(Arc::new(render.into())),
			};
			RenderIntermediate { ty, metadata }
		}
	};

	Ok(Item::new_from_element(intermediate))
}

#[node_macro::node(category(""), version(3))]
fn render(
	ctx: impl Ctx + ExtractFootprint + ExtractVarArgs,
	data: Item<RenderIntermediate>,
	#[scope(crate::platform_application_io::wgpu_executor::IDENTIFIER)] executor: impl Node<Context<'static>, Output = Item<WgpuExecutor>>,
	#[scope("runtime")] runtime: Item<RuntimeHandle>,
	#[source_id] source: Item<SourceId>,
	#[data] svg_readback: SvgReadbackSlot,
) -> Poll<Item<RenderOutput>> {
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	let RenderIntermediate { ty, mut metadata } = data.into_element();
	metadata.apply_transform(footprint.transform);

	// Content drawing only the part of it in view draws its markup for this view
	let ty = match ty {
		RenderIntermediateType::SvgInView(content) => {
			let mut render = SvgRender::new();
			let in_view = RenderParams {
				view: Some(*footprint),
				..render_params.clone()
			};
			content.render_svg(&mut render, &in_view);
			RenderIntermediateType::Svg(Arc::new(render.into()))
		}
		ty => ty,
	};

	let mut render_params = render_params.clone();
	render_params.footprint = *footprint;

	let data = match (render_params.render_output_type, ty) {
		(RenderOutputTypeRequest::Svg, RenderIntermediateType::Svg(data)) => {
			let logical_transform = glam::DAffine2::from_scale(glam::DVec2::splat(1.0 / render_params.scale)) * footprint.transform;
			let logical_resolution = footprint.resolution.as_dvec2() / render_params.scale;

			let mut render = SvgRender::from(data.as_ref());
			render.wrap_with_transform(logical_transform, Some(logical_resolution));

			let output = SvgRenderOutput::from(render);
			assert!(output.svg_defs.is_empty());

			let executor = executor.eval(&Context::default()).value().map(Item::into_element);
			svg_readback.poll(output, *footprint, executor, runtime.element(), *source.element())
		}
		(RenderOutputTypeRequest::Vello, RenderIntermediateType::Vello(data)) => {
			// The latest SVG render isn't needed anymore, and a finished one holds a whole SVG export
			svg_readback.clear();

			let (executor, executor_partial, executor_error) = match executor.eval(&Context::default()) {
				Poll::Final(executor) => (executor.into_element(), false, None),
				Poll::Partial(executor) => (executor.into_element(), true, None),
				Poll::Fallback(boxed) => {
					let (executor, error) = *boxed;
					(executor.into_element(), false, Some(error))
				}
				Poll::Pending => return Poll::Pending,
				Poll::Error(error) => return Poll::Error(error),
			};

			let mut draw = |scene: &mut VelloSceneBuilder| data.render_to_vello(scene, Default::default(), &render_params);
			let texture = match executor.render_vello(footprint.resolution, footprint.transform, &mut draw) {
				Ok(texture) => texture,
				// Leave the output transparent, at a size the GPU can hold, rather than taking down the editor
				Err(error) => {
					log::error!("Failed to render Vello scene: {error}");
					let limit = glam::UVec2::splat(executor.context().device.limits().max_texture_dimension_2d);
					match executor.render_vello(footprint.resolution.min(limit), footprint.transform, &mut |_| {}) {
						Ok(texture) => texture,
						Err(error) => return Poll::error(error),
					}
				}
			};

			let texture = RenderOutputType::Texture(texture);
			match (executor_partial, executor_error) {
				(true, _) => Poll::Partial(texture),
				(false, Some(error)) => Poll::Fallback(Box::new((texture, error))),
				(false, None) => Poll::Final(texture),
			}
		}
		_ => unreachable!("Render node did not receive its requested data type"),
	};

	data.map(|data| Item::new_from_element(RenderOutput { data, metadata }))
}

#[node_macro::node(category(""))]
fn create_context(
	// The executor boundary supplies the render config as the sole vararg (see `wrap_network_with_render_node_in_scope()`)
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
