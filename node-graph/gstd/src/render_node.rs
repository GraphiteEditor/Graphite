use graph_craft::document::value::RenderOutput;
pub use graph_craft::document::value::RenderOutputType;
pub use graph_craft::wasm_application_io::*;
use graphene_application_io::{ApplicationIo, ExportFormat, ImageTexture, RenderConfig, SurfaceFrame};
use graphene_core::gradient::GradientStops;
use graphene_core::raster::image::Image;
use graphene_core::raster_types::{CPU, Raster};
use graphene_core::table::Table;
use graphene_core::transform::Footprint;
use graphene_core::vector::Vector;
use graphene_core::{Artboard, CloneVarArgs, ExtractAll, ExtractVarArgs};
use graphene_core::{Color, Context, Ctx, ExtractFootprint, Graphic, OwnedContextImpl, WasmNotSend};
use graphene_svg_renderer::{Render, RenderOutputType as RenderOutputTypeRequest, RenderParams, RenderSvgSegmentList, SvgRender, format_transform_matrix};
use graphene_svg_renderer::{RenderMetadata, SvgSegment};

use std::sync::Arc;

#[cfg(feature = "wgpu")]
#[node_macro::node(category("Debug: GPU"))]
async fn create_surface<'a: 'n>(_: impl Ctx, editor: &'a WasmEditorApi) -> Arc<WasmSurfaceHandle> {
	Arc::new(editor.application_io.as_ref().unwrap().create_window())
}

#[derive(Clone, dyn_any::DynAny)]
pub enum RenderIntermediateType {
	Vello(Arc<vello::Scene>),
	Svg(Arc<(String, Vec<(u64, Image<Color>)>)>),
	Data(Arc<dyn Render + Send + Sync + 'static>),
}
#[derive(Clone, dyn_any::DynAny)]
pub struct RenderIntermediate {
	ty: RenderIntermediateType,
	metadata: RenderMetadata,
	contains_artboard: bool,
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
	editor_api: impl Node<Context<'static>, Output = &'a WasmEditorApi>,
) -> RenderIntermediate {
	let mut render = SvgRender::new();
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

	if data.contains_color_or_gradient() {
		return RenderIntermediate {
			ty: RenderIntermediateType::Data(Arc::new(data)),
			metadata,
			contains_artboard,
		};
	}
	let editor_api = editor_api.eval(None).await;

	if !render_params.for_export
		&& editor_api.editor_preferences.use_vello()
		&& matches!(
			render_params.render_output_type,
			graphene_svg_renderer::RenderOutputType::Canvas | graphene_svg_renderer::RenderOutputType::Texture
		) {
		let mut scene = vello::Scene::new();

		let mut context = wgpu_executor::RenderContext::default();
		data.render_to_vello(&mut scene, Default::default(), &mut context, render_params);

		RenderIntermediate {
			ty: RenderIntermediateType::Vello(Arc::new(scene)),
			metadata,
			contains_artboard,
		}
	} else {
		data.render_svg(&mut render, render_params);

		RenderIntermediate {
			ty: RenderIntermediateType::Svg(Arc::new((render.svg.to_svg_string(), render.image_data))),
			metadata,
			contains_artboard,
		}
	}
}
// #[node_macro::node(category(""))]
// async fn render_to_vello_scene<'a: 'n, T: 'static + Render + WasmNotSend + Sync>(
// 	ctx: impl Ctx + ExtractVarArgs + ExtractAll + CloneVarArgs,
// 	#[implementations(
// 		Context -> Table<Artboard>,
// 		Context -> Table<Graphic>,
// 		Context -> Table<Vector>,
// 		Context -> Table<Raster<CPU>>,
// 		Context -> Table<Color>,
// 		Context -> Table<GradientStops>,
// 	)]
// 	data: impl Node<Context<'static>, Output = T>,
// ) -> RenderIntermediate {
// 	let render_params = ctx
// 		.vararg(0)
// 		.expect("Did not find var args")
// 		.downcast_ref::<graphene_svg_renderer::RenderParams>()
// 		.expect("Downcasting render params yielded invalid type");

// 	let ctx = OwnedContextImpl::from(ctx.clone()).into_context();
// 	let data = data.eval(ctx).await;

// 	let footprint = Footprint::default();
// 	let mut metadata = RenderMetadata::default();
// 	data.collect_metadata(&mut metadata, footprint, None);

// 	if data.contains_color_or_gradient() {
// 		return RenderIntermediate {
// 			ty: RenderIntermediateType::Data(Arc::new(data)),
// 			metadata,
// 		};
// 	}

// 	let mut scene = vello::Scene::new();

// 	let mut context = wgpu_executor::RenderContext::default();
// 	data.render_to_vello(&mut scene, Default::default(), &mut context, render_params);

// 	RenderIntermediate {
// 		ty: RenderIntermediateType::Vello(Arc::new(scene)),
// 		metadata,
// 	}
// }

// #[cfg(feature = "vello")]
// #[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
// async fn render_canvas(render_config: RenderConfig, data: impl Render, editor: &WasmEditorApi, surface_handle: Option<wgpu_executor::WgpuSurface>, render_params: RenderParams) -> RenderOutputType {
// 	use graphene_application_io::{ImageTexture, SurfaceFrame};

// 	let mut footprint = render_config.viewport;
// 	footprint.resolution = footprint.resolution.max(glam::UVec2::splat(1));
// 	let Some(exec) = editor.application_io.as_ref().unwrap().gpu_executor() else {
// 		unreachable!("Attempted to render with Vello when no GPU executor is available");
// 	};
// 	use vello::*;

// 	let mut scene = Scene::new();
// 	let mut child = Scene::new();

// 	let mut context = wgpu_executor::RenderContext::default();
// 	data.render_to_vello(&mut child, Default::default(), &mut context, &render_params);

// 	// TODO: Instead of applying the transform here, pass the transform during the translation to avoid the O(n) cost
// 	scene.append(&child, Some(kurbo::Affine::new(footprint.transform.to_cols_array())));

// 	let mut background = Color::from_rgb8_srgb(0x22, 0x22, 0x22);
// 	if !data.contains_artboard() && !render_config.hide_artboards {
// 		background = Color::WHITE;
// 	}
// 	if let Some(surface_handle) = surface_handle {
// 		exec.render_vello_scene(&scene, &surface_handle, footprint.resolution, &context, background)
// 			.await
// 			.expect("Failed to render Vello scene");

// 		let frame = SurfaceFrame {
// 			surface_id: surface_handle.window_id,
// 			resolution: render_config.viewport.resolution,
// 			transform: glam::DAffine2::IDENTITY,
// 		};

// 		RenderOutputType::CanvasFrame(frame)
// 	} else {
// 		let texture = exec
// 			.render_vello_scene_to_texture(&scene, footprint.resolution, &context, background)
// 			.await
// 			.expect("Failed to render Vello scene");

// 		RenderOutputType::Texture(ImageTexture { texture })
// 	}
// }
#[node_macro::node(category(""))]
async fn create_context<'a: 'n>(
	#[extra_injections(InjectFootprint, InjectRealTime, InjectAnimationTime, InjectVarArgs)] render_config: RenderConfig,
	data: impl Node<Context<'static>, Output = RenderOutput>,
) -> RenderOutput {
	let footprint = render_config.viewport;

	let render_output_type = match render_config.export_format {
		ExportFormat::Svg => RenderOutputTypeRequest::Svg,
		ExportFormat::Png { .. } => todo!(),
		ExportFormat::Jpeg => todo!(),
		ExportFormat::Canvas => RenderOutputTypeRequest::Canvas,
		ExportFormat::Texture => RenderOutputTypeRequest::Texture,
	};
	let render_params = RenderParams {
		view_mode: render_config.view_mode,
		hide_artboards: render_config.hide_artboards,
		for_export: render_config.for_export,
		render_output_type,
		footprint: Footprint::default(),
		..Default::default()
	};
	let ctx = OwnedContextImpl::default()
		.with_footprint(footprint)
		.with_real_time(render_config.time.time)
		.with_animation_time(render_config.time.animation_time.as_secs_f64())
		.with_vararg(Box::new(render_params))
		.into_context();

	data.eval(ctx).await
}

#[node_macro::node(category(""))]
async fn render<'a: 'n>(
	ctx: impl Ctx + ExtractFootprint + ExtractVarArgs,
	editor_api: &'a WasmEditorApi,
	data: RenderIntermediate,
	_surface_handle: impl Node<Context<'static>, Output = Option<wgpu_executor::WgpuSurface>>,
) -> RenderOutput {
	// let data = data.eval(ctx.clone()).await;
	// let editor_api = editor_api.eval(None).await;
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");
	let mut render_params = render_params.clone();
	render_params.footprint = *footprint;
	let render_params = &render_params;

	let RenderIntermediate { ty, mut metadata, contains_artboard } = data;
	metadata.apply_transform(footprint.transform);

	let surface_handle = if cfg!(all(feature = "vello", target_family = "wasm")) {
		_surface_handle.eval(None).await
	} else {
		None
	};

	let mut output_format = render_params.render_output_type;
	// TODO: Actually request the right thing upfront
	if let RenderIntermediateType::Svg(_) = ty {
		output_format = RenderOutputTypeRequest::Svg;
	}
	let data = match output_format {
		RenderOutputTypeRequest::Svg => {
			let mut svg_renderer = SvgRender::new();
			if !contains_artboard && !render_params.hide_artboards {
				svg_renderer.leaf_tag("rect", |attributes| {
					attributes.push("x", "0");
					attributes.push("y", "0");
					attributes.push("width", footprint.resolution.x.to_string());
					attributes.push("height", footprint.resolution.y.to_string());
					let matrix = format_transform_matrix(footprint.transform.inverse());
					if !matrix.is_empty() {
						attributes.push("transform", matrix);
					}
					attributes.push("fill", "white");
				});
			}
			match &ty {
				RenderIntermediateType::Svg(svg_data) => {
					svg_renderer.svg.push(SvgSegment::from(svg_data.0.clone()));
					svg_renderer.image_data = svg_data.1.clone();
				}
				RenderIntermediateType::Data(data) => {
					data.render_svg(&mut svg_renderer, render_params);
				}
				_ => unreachable!(),
			};

			svg_renderer.wrap_with_transform(footprint.transform, Some(footprint.resolution.as_dvec2()));
			RenderOutputType::Svg {
				svg: svg_renderer.svg.to_svg_string(),
				image_data: svg_renderer.image_data,
			}
		}
		_ => {
			let mut context = wgpu_executor::RenderContext::default();
			let Some(exec) = editor_api.application_io.as_ref().unwrap().gpu_executor() else {
				unreachable!("Attempted to render with Vello when no GPU executor is available");
			};
			let scene = match ty {
				RenderIntermediateType::Vello(child) => {
					let mut scene = vello::Scene::new();
					scene.append(Arc::as_ref(&child), Some(vello::kurbo::Affine::new(footprint.transform.to_cols_array())));
					scene
				}
				RenderIntermediateType::Data(data) => {
					let mut scene = vello::Scene::new();
					data.render_to_vello(&mut scene, footprint.transform, &mut context, render_params);
					scene
				}
				_ => unreachable!(),
			};

			let mut background = Color::from_rgb8_srgb(0x22, 0x22, 0x22);
			if !contains_artboard && !render_params.hide_artboards {
				background = Color::WHITE;
			}
			if let Some(surface_handle) = surface_handle {
				exec.render_vello_scene(&scene, &surface_handle, footprint.resolution, &context, background)
					.await
					.expect("Failed to render Vello scene");

				let frame = SurfaceFrame {
					surface_id: surface_handle.window_id,
					resolution: footprint.resolution,
					transform: glam::DAffine2::IDENTITY,
				};

				RenderOutputType::CanvasFrame(frame)
			} else {
				let texture = exec
					.render_vello_scene_to_texture(&scene, footprint.resolution, &context, background)
					.await
					.expect("Failed to render Vello scene");

				RenderOutputType::Texture(ImageTexture { texture })
			}
		}
	};
	RenderOutput { data, metadata }
}
