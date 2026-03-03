use core_types::table::Table;
use core_types::transform::{Footprint, Transform};
use core_types::{CloneVarArgs, ExtractAll, ExtractVarArgs};
use core_types::{Color, Context, Ctx, ExtractFootprint, OwnedContextImpl, WasmNotSend};
use glam::UVec2;
use graph_craft::document::value::RenderOutput;
pub use graph_craft::document::value::RenderOutputType;
pub use graph_craft::wasm_application_io::*;
use graphene_application_io::{ApplicationIo, ExportFormat, ImageTexture, RenderConfig};
use graphic_types::Artboard;
use graphic_types::Graphic;
use graphic_types::Vector;
use graphic_types::raster_types::Image;
use graphic_types::raster_types::{CPU, Raster};
use rendering::{Render, RenderOutputType as RenderOutputTypeRequest, RenderParams, RenderSvgSegmentList, SvgRender, format_transform_matrix};
use rendering::{RenderMetadata, SvgSegment};
use std::collections::HashMap;
use std::sync::Arc;
use vector_types::GradientStops;
use vector_types::vector::style::RenderMode;
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
		hide_artboards: render_config.hide_artboards,
		for_export: render_config.for_export,
		render_output_type,
		footprint: Footprint::default(),
		scale: render_config.scale,
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
async fn render<'a: 'n>(ctx: impl Ctx + ExtractFootprint + ExtractVarArgs, editor_api: &'a WasmEditorApi, data: RenderIntermediate) -> RenderOutput {
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
			if !contains_artboard && !render_params.hide_artboards {
				rendering.leaf_tag("rect", |attributes| {
					attributes.push("x", "0");
					attributes.push("y", "0");
					attributes.push("width", logical_resolution.x.to_string());
					attributes.push("height", logical_resolution.y.to_string());
					let matrix = format_transform_matrix(footprint.transform.inverse());
					if !matrix.is_empty() {
						attributes.push("transform", matrix);
					}
					attributes.push("fill", "white");
				});
			}
			rendering.svg.push(SvgSegment::from(svg_data.0.clone()));
			rendering.image_data = svg_data.1.clone();
			rendering.svg_defs = svg_data.2.clone();

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
			scene.append(child, Some(footprint_transform_vello));

			// We now replace all transforms which are supposed to be infinite with a transform which covers the entire viewport
			// See <https://xi.zulipchat.com/#narrow/channel/197075-vello/topic/Full.20screen.20color.2Fgradients/near/538435044> for more detail
			let scaled_infinite_transform = vello::kurbo::Affine::scale_non_uniform(physical_resolution.x as f64, physical_resolution.y as f64);
			for transform in scene.encoding_mut().transforms.iter_mut() {
				if transform.matrix[0] == f32::INFINITY {
					*transform = vello_encoding::Transform::from_kurbo(&scaled_infinite_transform);
				}
			}

			let background = if !render_params.for_export && !contains_artboard && !render_params.hide_artboards {
				Some(Color::WHITE)
			} else {
				None
			};

			// Pixel Preview: render at the 100%-scale in document space, then upscale with nearest-neighbor filtering.
			// This shows the actual pixels of the exported image at viewport zoom levels above 100%.
			if render_params.render_mode == RenderMode::PixelPreview {
				let logical_zoom = footprint.decompose_scale().x;
				if logical_zoom > 1. {
					// Apply an inverse zoom to return scale to the identity, allowing content to render at 100% scale in document space.
					let unzoomed_transform = glam::DAffine2::from_scale(glam::DVec2::splat(1. / logical_zoom)) * footprint.transform;

					// The translation of unzoomed_transform is -pan_document_space (the pan in document units, negated).
					// Snap the pan to integer document pixels so paths always sit at exact document-pixel-aligned positions in the render.
					// Without this, a fractional pan shifts which edges are at sub-pixel boundaries in the 100%-scale render,
					// causing the antialiasing pattern to change as you zoom or pan slightly, making the view visually unstable.
					let pan_document_space = -unzoomed_transform.translation;
					let snapped_pan_document_space = pan_document_space.floor();
					// Remaining fractional document-pixel offset in [0, 1) per axis.
					let fractional_pan = pan_document_space - snapped_pan_document_space;
					let snapped_unzoomed = glam::DAffine2::from_cols(unzoomed_transform.matrix2.x_axis, unzoomed_transform.matrix2.y_axis, -snapped_pan_document_space);
					let document_transform_vello = vello::kurbo::Affine::new((scale_transform * snapped_unzoomed).to_cols_array());

					// Expand by ceil(scale) + 2 extra pixels:
					// scale covers the fractional_pan offset (up to 1 document pixel at DPI scale, i.e. scale texture pixels) and 2 is a safety margin for rounding.
					let extra = scale.ceil() as u32 + 2;
					let document_resolution = UVec2::new(
						(physical_resolution.x as f64 / logical_zoom).ceil() as u32 + extra,
						(physical_resolution.y as f64 / logical_zoom).ceil() as u32 + extra,
					);

					let mut scene = vello::Scene::new();
					scene.append(child, Some(document_transform_vello));

					let scaled_infinite_transform = vello::kurbo::Affine::scale_non_uniform(document_resolution.x as f64, document_resolution.y as f64);
					for transform in scene.encoding_mut().transforms.iter_mut() {
						if transform.matrix[0] == f32::INFINITY {
							*transform = vello_encoding::Transform::from_kurbo(&scaled_infinite_transform);
						}
					}

					let document_texture = exec
						.render_vello_scene_to_texture(&scene, document_resolution, context, background)
						.await
						.expect("Failed to render Vello scene for pixel preview");

					// Use the exact zoom reciprocal as scale rather than the texture-size ratio, which would
					// accumulate rounding error across the viewport width and misplace document pixel boundaries.
					// The source offset accounts for the fractional document-pixel snap so that the integer document-pixel
					// grid aligns correctly in the upscaled viewport output.
					let source_scale = glam::Vec2::splat(1. / logical_zoom as f32);
					let source_offset = (fractional_pan * scale).as_vec2();
					let texture = exec
						.upscale_texture_nearest_neighbor(&document_texture, physical_resolution, source_scale, source_offset)
						.expect("Failed to upscale pixel preview texture");

					return RenderOutput {
						data: RenderOutputType::Texture(ImageTexture { texture }),
						metadata,
					};
				}
			}

			let texture = exec
				.render_vello_scene_to_texture(&scene, physical_resolution, context, background)
				.await
				.expect("Failed to render Vello scene");

			RenderOutputType::Texture(ImageTexture { texture })
		}
		_ => unreachable!("Render node did not receive its requested data type"),
	};
	RenderOutput { data, metadata }
}
