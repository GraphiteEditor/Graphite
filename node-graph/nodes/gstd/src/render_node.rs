use core_types::table::Table;
use core_types::transform::{Footprint, Transform};
use core_types::uuid::generate_uuid;
use core_types::{CloneVarArgs, ExtractAll, ExtractVarArgs};
use core_types::{Color, Context, Ctx, ExtractFootprint, OwnedContextImpl, WasmNotSend};
pub use graph_craft::application_io::*;
use graph_craft::document::value::RenderOutput;
pub use graph_craft::document::value::RenderOutputType;
use graphene_application_io::{ApplicationIo, ExportFormat, RenderConfig};
use graphic_types::raster_types::Image;
use graphic_types::raster_types::{CPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use rendering::{Render, RenderOutputType as RenderOutputTypeRequest, RenderParams, RenderSvgSegmentList, SvgRender, checkerboard_brush};
use rendering::{RenderMetadata, SvgSegment};
use std::collections::HashMap;
use std::sync::Arc;
use vector_types::GradientStops;
use wgpu_executor::RenderContext;

// Re-export render_output_cache from render_cache module
pub use crate::render_cache::render_output_cache;

/// List of (canvas id, image data) pairs for embedding images as canvases in the final SVG string.
type ImageData = HashMap<Image<Color>, u64>;

#[derive(Clone, dyn_any::DynAny)]
pub enum RenderIntermediateType {
	Vello(Arc<(vello::Scene, RenderContext)>),
	Svg(Arc<(String, ImageData, String)>),
}
#[derive(Clone, dyn_any::DynAny)]
pub struct RenderIntermediate {
	pub(crate) ty: RenderIntermediateType,
	pub(crate) metadata: RenderMetadata,
	pub(crate) contains_artboard: bool,
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
	let contains_artboard = data.contains_artboard();

	match &render_params.render_output_type {
		RenderOutputTypeRequest::Vello => {
			let mut scene = vello::Scene::new();

			let mut context = wgpu_executor::RenderContext::default();
			data.render_to_vello(&mut scene, Default::default(), &mut context, render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Vello(Arc::new((scene, context))),
				metadata,
				contains_artboard,
			}
		}
		RenderOutputTypeRequest::Svg => {
			let mut render = SvgRender::new();

			data.render_svg(&mut render, render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Svg(Arc::new((render.svg.to_svg_string(), render.image_data, render.svg_defs.clone()))),
				metadata,
				contains_artboard,
			}
		}
	}
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
		hide_artboards: false,
		for_export: render_config.for_export,
		render_output_type,
		footprint: Footprint::default(),
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

	let RenderIntermediate { ty, mut metadata, contains_artboard } = data;
	metadata.apply_transform(footprint.transform);

	let data = match (render_params.render_output_type, &ty) {
		(RenderOutputTypeRequest::Svg, RenderIntermediateType::Svg(svg_data)) => {
			let mut rendering = SvgRender::new();

			// Infinite canvas background (no artboards)
			if !contains_artboard && !render_params.hide_artboards {
				let show_checkerboard = render_params.to_canvas();
				if show_checkerboard && render_params.viewport_zoom > 0. {
					// Checkerboard pattern anchored at the document origin, tiling at 8x8 viewport pixels
					let checker_id = format!("checkered-canvas-{}", generate_uuid());
					let cell_size = 8. / render_params.viewport_zoom;
					let pattern_size = cell_size * 2.;

					// Compute the axis-aligned bounding box of all four viewport corners in document space,
					// which is necessary when the view is rotated so the rect fully covers the visible area
					let inverse_transform = footprint.transform.inverse();
					let corners = [
						inverse_transform.transform_point2(glam::DVec2::ZERO),
						inverse_transform.transform_point2(glam::DVec2::new(logical_resolution.x, 0.)),
						inverse_transform.transform_point2(glam::DVec2::new(0., logical_resolution.y)),
						inverse_transform.transform_point2(logical_resolution),
					];
					let bb_min = corners.iter().fold(glam::DVec2::MAX, |acc, &c| acc.min(c));
					let bb_max = corners.iter().fold(glam::DVec2::MIN, |acc, &c| acc.max(c));

					rendering.leaf_tag("rect", |attributes| {
						attributes.push("x", bb_min.x.to_string());
						attributes.push("y", bb_min.y.to_string());
						attributes.push("width", (bb_max.x - bb_min.x).to_string());
						attributes.push("height", (bb_max.y - bb_min.y).to_string());
						attributes.push("fill", format!("url(#{checker_id})"));
					});

					// Pattern defs will be appended after the intermediate defs are copied below
					rendering.svg_defs = format!(
						r##"<pattern id="{checker_id}" x="0" y="0" width="{pattern_size}" height="{pattern_size}" patternUnits="userSpaceOnUse"><rect width="{pattern_size}" height="{pattern_size}" fill="#fff"/><rect x="{cell_size}" y="0" width="{cell_size}" height="{cell_size}" fill="#ccc"/><rect x="0" y="{cell_size}" width="{cell_size}" height="{cell_size}" fill="#ccc"/></pattern>"##,
					);
				}
			}

			let existing_defs = rendering.svg_defs.clone();
			rendering.svg.push(SvgSegment::from(svg_data.0.clone()));
			rendering.image_data = svg_data.1.clone();
			rendering.svg_defs = format!("{existing_defs}{}", svg_data.2);

			rendering.wrap_with_transform(footprint.transform, Some(logical_resolution));
			RenderOutputType::Svg {
				svg: rendering.svg.to_svg_string(),
				image_data: rendering.image_data.into_iter().map(|(image, id)| (id, image)).collect(),
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

			// Infinite canvas checkerboard (when no artboards are present)
			let show_checkerboard = !render_params.for_export && !contains_artboard && !render_params.hide_artboards;
			if show_checkerboard && scale > 0. && render_params.viewport_zoom > 0. {
				// Compute the axis-aligned bounding box of all four viewport corners in document space,
				// which is necessary so the rect fully covers the visible area when the canvas is tilted
				let inverse_footprint = footprint_transform.inverse();
				let corners = [
					inverse_footprint.transform_point2(glam::DVec2::ZERO),
					inverse_footprint.transform_point2(glam::DVec2::new(physical_resolution.x as f64, 0.)),
					inverse_footprint.transform_point2(glam::DVec2::new(0., physical_resolution.y as f64)),
					inverse_footprint.transform_point2(physical_resolution.as_dvec2()),
				];
				let bb_min = corners.iter().fold(glam::DVec2::MAX, |acc, &c| acc.min(c));
				let bb_max = corners.iter().fold(glam::DVec2::MIN, |acc, &c| acc.max(c));
				let doc_rect = vello::kurbo::Rect::new(bb_min.x, bb_min.y, bb_max.x, bb_max.y);

				// Draw in document space, transformed to screen by footprint_transform (includes rotation)
				// Brush maps each pixel to 1/viewport_zoom document units, giving constant 8px cells
				let brush_transform = vello::kurbo::Affine::scale(1. / render_params.viewport_zoom);
				scene.fill(vello::peniko::Fill::NonZero, footprint_transform_vello, &checkerboard_brush(), Some(brush_transform), &doc_rect);
			}

			scene.append(child, Some(footprint_transform_vello));

			// We now replace all transforms which are supposed to be infinite with a transform which covers the entire viewport
			// See <https://xi.zulipchat.com/#narrow/channel/197075-vello/topic/Full.20screen.20color.2Fgradients/near/538435044> for more detail
			let scaled_infinite_transform = vello::kurbo::Affine::scale_non_uniform(physical_resolution.x as f64, physical_resolution.y as f64);
			for transform in scene.encoding_mut().transforms.iter_mut() {
				if transform.matrix[0] == f32::INFINITY {
					*transform = vello_encoding::Transform::from_kurbo(&scaled_infinite_transform);
				}
			}

			let texture = Arc::new(exec.render_vello_scene_to_texture(&scene, physical_resolution, context).await.expect("Failed to render Vello scene"));

			RenderOutputType::Texture(texture.into())
		}
		_ => unreachable!("Render node did not receive its requested data type"),
	};
	RenderOutput { data, metadata }
}
