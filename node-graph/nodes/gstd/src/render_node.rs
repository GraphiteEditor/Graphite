use core_types::list::List;
use core_types::transform::{Footprint, Transform};
use core_types::{CloneVarArgs, ExtractAll, ExtractVarArgs, InjectFootprint};
use core_types::{Color, Context, Ctx, ExtractFootprint, OwnedContextImpl, WasmNotSend};
use graph_craft::document::value::{RenderOutput, RenderOutputType};
use graphene_application_io::{ExportFormat, RenderConfig};
use graphic_types::raster_types::{CPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use rendering::{Render, RenderMetadata, RenderOutputType as RenderOutputTypeRequest, RenderParams, SvgRender, SvgRenderOutput};
use std::sync::Arc;
use vector_types::GradientStops;
use wgpu_executor::{RenderContext, WgpuExecutor};

#[derive(Clone, dyn_any::DynAny)]
pub enum RenderIntermediateType {
	Vello(Arc<(vello::Scene, RenderContext)>),
	Svg(Arc<SvgRenderOutput>),
}
#[derive(Clone, dyn_any::DynAny)]
pub struct RenderIntermediate {
	pub(crate) ty: RenderIntermediateType,
	pub(crate) metadata: RenderMetadata,
}

#[node_macro::node(category(""))]
async fn render_intermediate<'a: 'n, T: 'static + Render + WasmNotSend + Send + Sync>(
	ctx: impl Ctx + ExtractVarArgs + ExtractAll + CloneVarArgs + InjectFootprint,
	#[implementations(
		Context -> List<Artboard>,
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
		Context -> List<String>,
	)]
	data: impl Node<Context<'static>, Output = T>,
) -> RenderIntermediate {
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	let logical_footprint = *ctx.footprint();
	let physical_footprint = Footprint {
		transform: glam::DAffine2::from_scale(glam::DVec2::splat(render_params.scale)) * logical_footprint.transform,
		..logical_footprint
	};
	let ctx = OwnedContextImpl::from(ctx.clone()).with_footprint(physical_footprint).into_context();
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
				ty: RenderIntermediateType::Svg(Arc::new(render.into())),
				metadata,
			}
		}
	}
}

#[node_macro::node(category(""))]
async fn render<'a: 'n>(
	ctx: impl Ctx + ExtractFootprint + ExtractVarArgs,
	#[scope(crate::platform_application_io::try_wgpu_executor::IDENTIFIER)] executor: Option<&'a WgpuExecutor>,
	data: RenderIntermediate,
) -> RenderOutput {
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");
	let mut render_params = render_params.clone();
	render_params.footprint = *footprint;

	let RenderIntermediate { ty, mut metadata } = data;
	metadata.apply_transform(footprint.transform);

	let data = match (render_params.render_output_type, ty) {
		(RenderOutputTypeRequest::Svg, RenderIntermediateType::Svg(data)) => {
			let logical_resolution = render_params.footprint.resolution.as_dvec2() / render_params.scale;

			let mut render = SvgRender::from(data.as_ref());
			render.wrap_with_transform(render_params.footprint.transform, Some(logical_resolution));

			let output = SvgRenderOutput::from(render);
			assert!(output.svg_defs.is_empty());

			RenderOutputType::Svg {
				svg: output.svg,
				image_data: output.image_data.into_iter().map(|(image, id)| (id, image.0)).collect(),
			}
		}
		(RenderOutputTypeRequest::Vello, RenderIntermediateType::Vello(data)) => {
			let (scene, context) = data.as_ref();
			let scale = render_params.scale;
			let physical_resolution = render_params.footprint.resolution;

			let scale_transform = glam::DAffine2::from_scale(glam::DVec2::splat(scale));
			let footprint_transform = scale_transform * render_params.footprint.transform;
			let footprint_transform_vello = vello::kurbo::Affine::new(footprint_transform.to_cols_array());

			let mut transformed_scene = vello::Scene::new();
			transformed_scene.append(scene, Some(footprint_transform_vello));

			// We now replace all transforms which are supposed to be infinite with a transform which covers the entire viewport.
			// See <https://xi.zulipchat.com/#narrow/channel/197075-vello/topic/Full.20screen.20color.2Fgradients/near/538435044> for more detail.
			//
			// `!is_finite()` rather than `== f32::INFINITY`: `scene.append` composes the child's `Affine::scale(INFINITY)` with
			// the viewport rotation, leaving `matrix[0] = cos(θ) * INFINITY`. In the (90°, 270°) tilt range cos is negative so
			// the result is `-INFINITY`, which the old equality check missed; Vello then rasterized a unit rect with non-finite
			// vertices, dropping the gradient and tanking performance. `!is_finite()` also covers NaN as a guard against future
			// code paths where `matrix[0]` could land on `0 * INFINITY`.
			let scaled_infinite_transform = vello::kurbo::Affine::scale_non_uniform(physical_resolution.x as f64, physical_resolution.y as f64);
			for transform in transformed_scene.encoding_mut().transforms.iter_mut() {
				if !transform.matrix[0].is_finite() {
					*transform = vello_encoding::Transform::from_kurbo(&scaled_infinite_transform);
				}
			}

			let texture = executor
				.expect("GPU executor not available")
				.render_vello_scene(&transformed_scene, physical_resolution, context, None)
				.await
				.expect("Failed to render Vello scene");
			RenderOutputType::Texture(texture.into())
		}
		_ => unreachable!("Render node did not receive its requested data type"),
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
