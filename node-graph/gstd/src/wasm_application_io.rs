#[cfg(target_family = "wasm")]
use base64::Engine;
pub use graph_craft::document::value::RenderOutputType;
pub use graph_craft::wasm_application_io::*;
use graphene_application_io::ApplicationIo;
#[cfg(target_family = "wasm")]
use graphene_core::gradient::GradientStops;
#[cfg(target_family = "wasm")]
use graphene_core::math::bbox::Bbox;
use graphene_core::raster::image::Image;
use graphene_core::raster_types::{CPU, Raster};
use graphene_core::table::Table;
#[cfg(target_family = "wasm")]
use graphene_core::transform::Footprint;
#[cfg(target_family = "wasm")]
use graphene_core::vector::Vector;
use graphene_core::{Color, Ctx};
#[cfg(target_family = "wasm")]
use graphene_core::{Graphic, WasmNotSend};
#[cfg(target_family = "wasm")]
use graphene_svg_renderer::{Render, RenderParams, RenderSvgSegmentList, SvgRender};
use std::sync::Arc;
#[cfg(target_family = "wasm")]
use wasm_bindgen::JsCast;
#[cfg(target_family = "wasm")]
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

#[cfg(feature = "wgpu")]
#[node_macro::node(category("Debug: GPU"))]
async fn create_surface<'a: 'n>(_: impl Ctx, editor: &'a WasmEditorApi) -> Arc<WasmSurfaceHandle> {
	Arc::new(editor.application_io.as_ref().unwrap().create_window())
}

#[node_macro::node(category("Web Request"))]
async fn get_request(_: impl Ctx, _primary: (), #[name("URL")] url: String, discard_result: bool) -> String {
	#[cfg(target_family = "wasm")]
	{
		if discard_result {
			wasm_bindgen_futures::spawn_local(async move {
				let _ = reqwest::get(url).await;
			});
			return String::new();
		}
	}
	#[cfg(not(target_family = "wasm"))]
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
	#[cfg(target_family = "wasm")]
	{
		if discard_result {
			wasm_bindgen_futures::spawn_local(async move {
				let _ = reqwest::Client::new().post(url).body(body).header("Content-Type", "application/octet-stream").send().await;
			});
			return String::new();
		}
	}
	#[cfg(not(target_family = "wasm"))]
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
fn image_to_bytes(_: impl Ctx, image: Table<Raster<CPU>>) -> Vec<u8> {
	let Some(image) = image.iter().next() else { return vec![] };
	image.element.data.iter().flat_map(|color| color.to_rgb8_srgb().into_iter()).collect::<Vec<u8>>()
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
fn decode_image(_: impl Ctx, data: Arc<[u8]>) -> Table<Raster<CPU>> {
	let Some(image) = image::load_from_memory(data.as_ref()).ok() else {
		return Table::new();
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

	Table::new_from_element(Raster::new_cpu(image))
}

#[cfg(target_family = "wasm")]
#[node_macro::node(category(""))]
async fn rasterize<T: WasmNotSend + 'n>(
	_: impl Ctx,
	#[implementations(
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Graphic>,
		Table<Color>,
		Table<GradientStops>,
	)]
	mut data: Table<T>,
	footprint: Footprint,
	surface_handle: Arc<graphene_application_io::SurfaceHandle<HtmlCanvasElement>>,
) -> Table<Raster<CPU>>
where
	Table<T>: Render,
{
	use graphene_core::table::TableRow;

	if footprint.transform.matrix2.determinant() == 0. {
		log::trace!("Invalid footprint received for rasterization");
		return Table::new();
	}

	let mut render = SvgRender::new();
	let aabb = Bbox::from_transform(footprint.transform).to_axis_aligned_bbox();
	let size = aabb.size();
	let resolution = footprint.resolution;
	let render_params = RenderParams {
		footprint,
		for_export: true,
		..Default::default()
	};

	for row in data.iter_mut() {
		*row.transform = glam::DAffine2::from_translation(-aabb.start) * *row.transform;
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

	let image = Image::from_image_data(&rasterized.data().0, resolution.x as u32, resolution.y as u32);
	Table::new_from_row(TableRow {
		element: Raster::new_cpu(image),
		transform: footprint.transform,
		..Default::default()
	})
}
