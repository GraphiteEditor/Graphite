use core_types::table::Table;
use core_types::transform::{Footprint, Transform};
use core_types::uuid::generate_uuid;
use core_types::{CloneVarArgs, ExtractAll, ExtractVarArgs};
use core_types::{Color, Context, Ctx, ExtractFootprint, OwnedContextImpl, WasmNotSend};
pub use graph_craft::application_io::*;
use graph_craft::document::value::RenderOutput;
pub use graph_craft::document::value::RenderOutputType;
use graphene_application_io::{ApplicationIo, ExportFormat, RenderConfig};
use graphic_types::raster_types::{CPU, Raster};
use graphic_types::{Graphic, Vector};
use rendering::{Render, RenderMetadata, RenderOutputType as RenderOutputTypeRequest, RenderParams, SvgRender, SvgRenderOutput};
use std::fmt::Write;
use std::sync::Arc;
use vector_types::GradientStops;
use wgpu_executor::RenderContext;

// Re-export render_output_cache from render_cache module
pub use crate::render_cache::render_output_cache;

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
	ctx: impl Ctx + ExtractVarArgs + ExtractAll + CloneVarArgs,
	#[implementations(
		Context -> Table<Table<Graphic>>,
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
				ty: RenderIntermediateType::Svg(Arc::new(render.into())),
				metadata,
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
			let Some(exec) = editor_api.application_io.as_ref().unwrap().gpu_executor() else {
				unreachable!("Attempted to render with Vello when no GPU executor is available");
			};
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

			let texture = exec
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
async fn render_background<'a: 'n>(ctx: impl Ctx + ExtractFootprint + ExtractVarArgs, editor_api: &'a PlatformEditorApi, data: RenderOutput) -> RenderOutput {
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	if !render_params.to_canvas() {
		return data;
	}

	let RenderOutput { data: foreground_data, metadata } = data;
	let mut render_params = render_params.clone();
	render_params.footprint = *footprint;

	let data = match foreground_data {
		RenderOutputType::Texture(foreground_texture) => {
			if let Some(exec) = editor_api.application_io.as_ref().unwrap().gpu_executor() {
				let doc_to_screen = (glam::DAffine2::from_scale(glam::DVec2::splat(render_params.scale)) * render_params.footprint.transform).as_affine2();
				let blended = exec
					.composite_background(foreground_texture.as_ref(), &metadata.backgrounds, doc_to_screen, render_params.viewport_zoom as f32)
					.await;

				RenderOutputType::Texture(blended.into())
			} else {
				RenderOutputType::Texture(foreground_texture)
			}
		}
		RenderOutputType::Svg {
			svg: foreground_svg,
			image_data: foreground_images,
		} => {
			let mut render = SvgRender::new();

			if render_params.viewport_zoom > 0. {
				let draw_checkerboard = |render: &mut SvgRender, rect: vello::kurbo::Rect, pattern_origin: glam::DVec2, checker_id_prefix: &str| {
					let checker_id = format!("{checker_id_prefix}-{}", generate_uuid());
					let cell_size = 8. / render_params.viewport_zoom;
					let pattern_size = cell_size * 2.;

					write!(
						&mut render.svg_defs,
						r##"<pattern id="{checker_id}" x="{}" y="{}" width="{pattern_size}" height="{pattern_size}" patternUnits="userSpaceOnUse"><rect width="{pattern_size}" height="{pattern_size}" fill="#ffffff" /><rect x="{cell_size}" y="0" width="{cell_size}" height="{cell_size}" fill="#cccccc" /><rect x="0" y="{cell_size}" width="{cell_size}" height="{cell_size}" fill="#cccccc" /></pattern>"##,
						pattern_origin.x,
						pattern_origin.y,
					)
					.unwrap();

					render.leaf_tag("rect", |attributes| {
						attributes.push("x", rect.x0.to_string());
						attributes.push("y", rect.y0.to_string());
						attributes.push("width", rect.width().to_string());
						attributes.push("height", rect.height().to_string());
						attributes.push("fill", format!("url(#{checker_id})"));
					});
				};

				if metadata.backgrounds.is_empty() {
					if render_params.scale > 0. {
						let logical_resolution = render_params.footprint.resolution.as_dvec2() / render_params.scale;
						let logical_footprint = Footprint {
							resolution: logical_resolution.round().as_uvec2().max(glam::UVec2::ONE),
							..render_params.footprint
						};
						let bounds = logical_footprint.viewport_bounds_in_local_space();
						let min = bounds.start.floor();
						let max = bounds.end.ceil();

						if min.is_finite() && max.is_finite() {
							let rect = vello::kurbo::Rect::new(min.x, min.y, max.x, max.y);
							draw_checkerboard(&mut render, rect, glam::DVec2::ZERO, "checkered-viewport");
						}
					}
				} else {
					for background in &metadata.backgrounds {
						let [a, b] = [background.location, background.location + background.dimensions];
						let rect = vello::kurbo::Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
						draw_checkerboard(&mut render, rect, glam::DVec2::new(rect.x0, rect.y0), "checkered-artboard");
					}
				}
			}

			let logical_resolution = render_params.footprint.resolution.as_dvec2() / render_params.scale;
			render.wrap_with_transform(render_params.footprint.transform, Some(logical_resolution));

			let background = SvgRenderOutput::from(render);
			assert!(background.svg_defs.is_empty());

			let svg = format!("{}{}", background.svg, foreground_svg);
			let image_data = foreground_images;

			RenderOutputType::Svg { svg, image_data }
		}
		_ => unreachable!("Render background node received unsupported render output type"),
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
