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
			// When the viewport is tilted, the upscale applies the rotation so individual document pixels
			// appear as tilted squares, while the document-space render itself stays pixel-aligned (no rotation).
			if render_params.render_mode == RenderMode::PixelPreview {
				let logical_zoom = footprint.decompose_scale().x;
				if logical_zoom > 1. {
					let inv = footprint.transform.inverse();

					// Compute the viewport's axis-aligned bounding box in document space.
					// When tilted, the visible area is a rotated rectangle in document space;
					// the AABB of that rectangle determines the document render region.
					let corners = [
						glam::DVec2::ZERO,
						glam::DVec2::new(logical_resolution.x, 0.),
						glam::DVec2::new(logical_resolution.x, logical_resolution.y),
						glam::DVec2::new(0., logical_resolution.y),
					];
					let doc_corners = corners.map(|c| inv.transform_point2(c));
					let doc_min = doc_corners.iter().copied().reduce(|a, b| a.min(b)).unwrap();
					let doc_max = doc_corners.iter().copied().reduce(|a, b| a.max(b)).unwrap();

					// Snap the document render origin to integer document pixels for stable antialiasing.
					// This ensures paths always sit at exact document-pixel-aligned positions in the 100%-scale render,
					// so the antialiasing pattern doesn't flicker as you zoom or pan slightly.
					let snapped_doc_origin = doc_min.floor();
					let doc_size = doc_max - snapped_doc_origin;

					// Document render transform: no zoom, no rotation — only translation at DPI scale.
					// This renders the document pixel-aligned at 100% scale.
					let document_render_transform = glam::DAffine2::from_translation(-snapped_doc_origin);
					let document_transform_vello = vello::kurbo::Affine::new((scale_transform * document_render_transform).to_cols_array());

					let extra = scale.ceil() as u32 + 2;
					let document_resolution = UVec2::new((doc_size.x * scale).ceil() as u32 + extra, (doc_size.y * scale).ceil() as u32 + extra);

					let mut scene = vello::Scene::new();
					scene.append(child, Some(document_transform_vello));

					// Same infinite-transform fix as in the normal render path (see comment there), but sized to
					// the document-resolution render target instead of the full viewport resolution.
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

					// Map output viewport pixels to document texture coordinates.
					// source_transform = footprint.inverse().matrix2 (encodes rotation and 1/zoom).
					// source_offset = DPI * (viewport_origin_in_doc_space - snapped_doc_origin).
					// For each output pixel p: tex_coord = source_transform * p + source_offset.
					let source_transform = glam::Mat2::from_cols(inv.matrix2.x_axis.as_vec2(), inv.matrix2.y_axis.as_vec2());
					let source_offset = ((inv.translation - snapped_doc_origin) * scale).as_vec2();
					let texture = exec
						.resample_texture(&document_texture, physical_resolution, source_transform, source_offset, wgpu::FilterMode::Nearest)
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
