use graph_craft::document::value::RenderOutput;
pub use graph_craft::document::value::RenderOutputType;
pub use graph_craft::wasm_application_io::*;
use graphene_application_io::{ApplicationIo, ExportFormat, RenderConfig};
#[cfg(target_arch = "wasm32")]
use graphene_core::instances::Instances;
#[cfg(target_arch = "wasm32")]
use graphene_core::math::bbox::Bbox;
use graphene_core::raster::image::Image;
use graphene_core::raster_types::{CPU, Raster, RasterDataTable};
use graphene_core::transform::Footprint;
use graphene_core::vector::VectorDataTable;
use graphene_core::{Color, Context, Ctx, ExtractFootprint, GraphicGroupTable, OwnedContextImpl, WasmNotSend};
use graphene_svg_renderer::RenderMetadata;
use graphene_svg_renderer::{GraphicElementRendered, RenderParams, RenderSvgSegmentList, SvgRender, format_transform_matrix};

#[cfg(target_arch = "wasm32")]
use base64::Engine;
#[cfg(target_arch = "wasm32")]
use glam::DAffine2;
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

#[cfg(feature = "wgpu")]
#[node_macro::node(category("Debug: GPU"))]
async fn create_surface<'a: 'n>(_: impl Ctx, editor: &'a WasmEditorApi) -> Arc<WasmSurfaceHandle> {
	Arc::new(editor.application_io.as_ref().unwrap().create_window())
}

// TODO: Fix and reenable in order to get the 'Draw Canvas' node working again.
// #[cfg(target_arch = "wasm32")]
// use wasm_bindgen::Clamped;
//
// #[node_macro::node(category("Debug: GPU"))]
// #[cfg(target_arch = "wasm32")]
// async fn draw_image_frame(
// 	_: impl Ctx,
// 	image: RasterDataTable<graphene_core::raster::SRGBA8>,
// 	surface_handle: Arc<WasmSurfaceHandle>,
// ) -> graphene_core::application_io::SurfaceHandleFrame<HtmlCanvasElement> {
// 	let image = image.instance_ref_iter().next().unwrap().instance;
// 	let image_data = image.image.data;
// 	let array: Clamped<&[u8]> = Clamped(bytemuck::cast_slice(image_data.as_slice()));
// 	if image.image.width > 0 && image.image.height > 0 {
// 		let canvas = &surface_handle.surface;
// 		canvas.set_width(image.image.width);
// 		canvas.set_height(image.image.height);
// 		// TODO: replace "2d" with "bitmaprenderer" once we switch to ImageBitmap (lives on gpu) from RasterData (lives on cpu)
// 		let context = canvas.get_context("2d").unwrap().unwrap().dyn_into::<CanvasRenderingContext2d>().unwrap();
// 		let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(array, image.image.width, image.image.height).expect("Failed to construct RasterData");
// 		context.put_image_data(&image_data, 0., 0.).unwrap();
// 	}
// 	graphene_core::application_io::SurfaceHandleFrame {
// 		surface_handle,
// 		transform: image.transform,
// 	}
// }

#[node_macro::node(category("Web Request"))]
async fn get_request(_: impl Ctx, _primary: (), #[name("URL")] url: String, discard_result: bool) -> String {
	#[cfg(target_arch = "wasm32")]
	{
		if discard_result {
			wasm_bindgen_futures::spawn_local(async move {
				let _ = reqwest::get(url).await;
			});
			return String::new();
		}
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		#[cfg(feature = "tokio")]
		if discard_result {
			tokio::spawn(async move {
				let _ = reqwest::get(url).await;
			});
			return String::new();
		}
		#[cfg(not(feature = "tokio"))]
		if discard_result {
			return String::new();
		}
	}

	let Ok(response) = reqwest::get(url).await else { return String::new() };
	response.text().await.ok().unwrap_or_default()
}

#[node_macro::node(category("Web Request"))]
async fn post_request(_: impl Ctx, _primary: (), #[name("URL")] url: String, body: Vec<u8>, discard_result: bool) -> String {
	#[cfg(target_arch = "wasm32")]
	{
		if discard_result {
			wasm_bindgen_futures::spawn_local(async move {
				let _ = reqwest::Client::new().post(url).body(body).header("Content-Type", "application/octet-stream").send().await;
			});
			return String::new();
		}
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		#[cfg(feature = "tokio")]
		if discard_result {
			let url = url.clone();
			let body = body.clone();
			tokio::spawn(async move {
				let _ = reqwest::Client::new().post(url).body(body).header("Content-Type", "application/octet-stream").send().await;
			});
			return String::new();
		}
		#[cfg(not(feature = "tokio"))]
		if discard_result {
			return String::new();
		}
	}

	let Ok(response) = reqwest::Client::new().post(url).body(body).header("Content-Type", "application/octet-stream").send().await else {
		return String::new();
	};
	response.text().await.ok().unwrap_or_default()
}

#[node_macro::node(category("Web Request"), name("String to Bytes"))]
fn string_to_bytes(_: impl Ctx, string: String) -> Vec<u8> {
	string.into_bytes()
}

#[node_macro::node(category("Web Request"), name("Image to Bytes"))]
fn image_to_bytes(_: impl Ctx, image: RasterDataTable<CPU>) -> Vec<u8> {
	let Some(image) = image.instance_ref_iter().next() else { return vec![] };
	image.instance.data.iter().flat_map(|color| color.to_rgb8_srgb().into_iter()).collect::<Vec<u8>>()
}

#[node_macro::node(category("Web Request"))]
async fn load_resource<'a: 'n>(_: impl Ctx, _primary: (), #[scope("editor-api")] editor: &'a WasmEditorApi, #[name("URL")] url: String) -> Arc<[u8]> {
	let Some(api) = editor.application_io.as_ref() else {
		return Arc::from(include_bytes!("../../graph-craft/src/null.png").to_vec());
	};
	let Ok(data) = api.load_resource(url) else {
		return Arc::from(include_bytes!("../../graph-craft/src/null.png").to_vec());
	};
	let Ok(data) = data.await else {
		return Arc::from(include_bytes!("../../graph-craft/src/null.png").to_vec());
	};

	data
}

#[node_macro::node(category("Web Request"))]
fn decode_image(_: impl Ctx, data: Arc<[u8]>) -> RasterDataTable<CPU> {
	let Some(image) = image::load_from_memory(data.as_ref()).ok() else {
		return RasterDataTable::default();
	};
	let image = image.to_rgba32f();
	let image = Image {
		data: image
			.chunks(4)
			.map(|pixel| Color::from_unassociated_alpha(pixel[0], pixel[1], pixel[2], pixel[3]).to_linear_srgb())
			.collect(),
		width: image.width(),
		height: image.height(),
		..Default::default()
	};

	RasterDataTable::new(Raster::new_cpu(image))
}

fn render_svg(data: impl GraphicElementRendered, mut render: SvgRender, render_params: RenderParams, footprint: Footprint) -> RenderOutputType {
	if !data.contains_artboard() && !render_params.hide_artboards {
		render.leaf_tag("rect", |attributes| {
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

	data.render_svg(&mut render, &render_params);

	render.wrap_with_transform(footprint.transform, Some(footprint.resolution.as_dvec2()));

	RenderOutputType::Svg(render.svg.to_svg_string())
}

#[cfg(feature = "vello")]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
async fn render_canvas(
	render_config: RenderConfig,
	data: impl GraphicElementRendered,
	editor: &WasmEditorApi,
	surface_handle: wgpu_executor::WgpuSurface,
	render_params: RenderParams,
) -> RenderOutputType {
	use graphene_application_io::SurfaceFrame;

	let footprint = render_config.viewport;
	let Some(exec) = editor.application_io.as_ref().unwrap().gpu_executor() else {
		unreachable!("Attempted to render with Vello when no GPU executor is available");
	};
	use vello::*;

	let mut scene = Scene::new();
	let mut child = Scene::new();

	let mut context = wgpu_executor::RenderContext::default();
	data.render_to_vello(&mut child, Default::default(), &mut context, &render_params);

	// TODO: Instead of applying the transform here, pass the transform during the translation to avoid the O(n) cost
	scene.append(&child, Some(kurbo::Affine::new(footprint.transform.to_cols_array())));

	let mut background = Color::from_rgb8_srgb(0x22, 0x22, 0x22);
	if !data.contains_artboard() && !render_config.hide_artboards {
		background = Color::WHITE;
	}
	exec.render_vello_scene(&scene, &surface_handle, footprint.resolution, &context, background)
		.await
		.expect("Failed to render Vello scene");

	let frame = SurfaceFrame {
		surface_id: surface_handle.window_id,
		resolution: render_config.viewport.resolution,
		transform: glam::DAffine2::IDENTITY,
	};

	RenderOutputType::CanvasFrame(frame)
}

#[cfg(target_arch = "wasm32")]
#[node_macro::node(category(""))]
async fn rasterize<T: WasmNotSend + 'n>(
	_: impl Ctx,
	#[implementations(
		VectorDataTable,
		RasterDataTable<CPU>,
		GraphicGroupTable,
	)]
	mut data: Instances<T>,
	footprint: Footprint,
	surface_handle: Arc<graphene_application_io::SurfaceHandle<HtmlCanvasElement>>,
) -> RasterDataTable<CPU>
where
	Instances<T>: GraphicElementRendered,
{
	use graphene_core::instances::Instance;

	if footprint.transform.matrix2.determinant() == 0. {
		log::trace!("Invalid footprint received for rasterization");
		return RasterDataTable::default();
	}

	let mut render = SvgRender::new();
	let aabb = Bbox::from_transform(footprint.transform).to_axis_aligned_bbox();
	let size = aabb.size();
	let resolution = footprint.resolution;
	let render_params = RenderParams {
		culling_bounds: None,
		..Default::default()
	};

	for instance in data.instance_mut_iter() {
		*instance.transform = DAffine2::from_translation(-aabb.start) * *instance.transform;
	}
	data.render_svg(&mut render, &render_params);
	render.format_svg(glam::DVec2::ZERO, size);
	let svg_string = render.svg.to_svg_string();

	let canvas = &surface_handle.surface;
	canvas.set_width(resolution.x);
	canvas.set_height(resolution.y);

	let context = canvas.get_context("2d").unwrap().unwrap().dyn_into::<CanvasRenderingContext2d>().unwrap();

	let preamble = "data:image/svg+xml;base64,";
	let mut base64_string = String::with_capacity(preamble.len() + svg_string.len() * 4);
	base64_string.push_str(preamble);
	base64::engine::general_purpose::STANDARD.encode_string(svg_string, &mut base64_string);

	let image_data = web_sys::HtmlImageElement::new().unwrap();
	image_data.set_src(base64_string.as_str());
	wasm_bindgen_futures::JsFuture::from(image_data.decode()).await.unwrap();
	context
		.draw_image_with_html_image_element_and_dw_and_dh(&image_data, 0., 0., resolution.x as f64, resolution.y as f64)
		.unwrap();

	let rasterized = context.get_image_data(0., 0., resolution.x as f64, resolution.y as f64).unwrap();

	let mut result = RasterDataTable::default();
	let image = Image::from_image_data(&rasterized.data().0, resolution.x as u32, resolution.y as u32);
	result.push(Instance {
		instance: Raster::new_cpu(image),
		transform: footprint.transform,
		..Default::default()
	});

	result
}

#[node_macro::node(category(""))]
async fn render<'a: 'n, T: 'n + GraphicElementRendered + WasmNotSend>(
	render_config: RenderConfig,
	editor_api: impl Node<Context<'static>, Output = &'a WasmEditorApi>,
	#[implementations(
		Context -> VectorDataTable,
		Context -> RasterDataTable<CPU>,
		Context -> GraphicGroupTable,
		Context -> graphene_core::Artboard,
		Context -> graphene_core::ArtboardGroupTable,
		Context -> Option<Color>,
		Context -> Vec<Color>,
		Context -> bool,
		Context -> f32,
		Context -> f64,
		Context -> String,
	)]
	data: impl Node<Context<'static>, Output = T>,
	_surface_handle: impl Node<Context<'static>, Output = Option<wgpu_executor::WgpuSurface>>,
) -> RenderOutput {
	let footprint = render_config.viewport;
	let ctx = OwnedContextImpl::default()
		.with_footprint(footprint)
		.with_real_time(render_config.time.time)
		.with_animation_time(render_config.time.animation_time.as_secs_f64())
		.into_context();
	ctx.footprint();

	let RenderConfig { hide_artboards, for_export, .. } = render_config;
	let render_params = RenderParams {
		view_mode: render_config.view_mode,
		culling_bounds: None,
		thumbnail: false,
		hide_artboards,
		for_export,
		for_mask: false,
		alignment_parent_transform: None,
	};

	let data = data.eval(ctx.clone()).await;
	let editor_api = editor_api.eval(None).await;

	#[cfg(all(feature = "vello", not(test)))]
	let surface_handle = _surface_handle.eval(None).await;

	let use_vello = editor_api.editor_preferences.use_vello();
	#[cfg(all(feature = "vello", not(test)))]
	let use_vello = use_vello && surface_handle.is_some();

	let mut metadata = RenderMetadata::default();
	data.collect_metadata(&mut metadata, footprint, None);

	let output_format = render_config.export_format;
	let data = match output_format {
		ExportFormat::Svg => render_svg(data, SvgRender::new(), render_params, footprint),
		ExportFormat::Canvas => {
			if use_vello && editor_api.application_io.as_ref().unwrap().gpu_executor().is_some() {
				#[cfg(all(feature = "vello", not(test)))]
				return RenderOutput {
					data: render_canvas(render_config, data, editor_api, surface_handle.unwrap(), render_params).await,
					metadata,
				};
				#[cfg(any(not(feature = "vello"), test))]
				render_svg(data, SvgRender::new(), render_params, footprint)
			} else {
				render_svg(data, SvgRender::new(), render_params, footprint)
			}
		}
		_ => todo!("Non-SVG render output for {output_format:?}"),
	};
	RenderOutput { data, metadata }
}
