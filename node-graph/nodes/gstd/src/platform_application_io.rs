#[cfg(target_family = "wasm")]
use base64::Engine;
#[cfg(target_family = "wasm")]
use canvas_utils::{Canvas, CanvasHandle};
use core_types::list::{Item, List};
#[cfg(target_family = "wasm")]
use core_types::math::bbox::Bbox;
#[cfg(target_family = "wasm")]
use core_types::transform::Footprint;
#[cfg(target_family = "wasm")]
use core_types::{ATTR_EDITOR_MERGED_LAYERS, ATTR_TRANSFORM, WasmNotSend};
use core_types::{Color, Ctx};
pub use graph_craft::application_io::*;
pub use graph_craft::document::value::RenderOutputType;
use graphene_application_io::ApplicationIo;
#[cfg(target_family = "wasm")]
pub use graphene_canvas_utils as canvas_utils;
#[cfg(target_family = "wasm")]
use graphic_types::Graphic;
#[cfg(target_family = "wasm")]
use graphic_types::IntoGraphicList;
#[cfg(target_family = "wasm")]
use graphic_types::Vector;
use graphic_types::raster_types::Image;
use graphic_types::raster_types::{CPU, Raster};
#[cfg(target_family = "wasm")]
use graphic_types::vector_types::gradient::GradientStops;
#[cfg(target_family = "wasm")]
use rendering::{Render, RenderParams, RenderSvgSegmentList, SvgRender};
use std::sync::Arc;

fn parse_headers(headers: &str) -> reqwest::header::HeaderMap {
	use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

	let mut header_map = HeaderMap::new();
	for line in headers.lines() {
		if let Some((key, value)) = line.split_once(':') {
			let Ok(header_name) = HeaderName::from_bytes(key.trim().as_bytes()) else { continue };
			let Ok(header_value) = HeaderValue::from_str(value.trim()) else { continue };
			header_map.insert(header_name, header_value);
		}
	}
	header_map
}

/// Sends an HTTP GET request to a specified URL and optionally waits for the response (unless discarded) which is output as a string.
#[node_macro::node(category("Web Request"))]
async fn get_request(
	_: impl Ctx,
	_primary: Item<()>,
	/// The web address to send the GET request to.
	#[name("URL")]
	url: Item<String>,
	/// Makes the request run in the background without waiting on a response. This is useful for triggering webhooks without blocking the continued execution of the graph.
	discard_result: Item<bool>,
	#[widget(ParsedWidgetOverride::Custom = "text_area")] headers: Item<String>,
) -> Item<String> {
	let _primary = _primary.into_element();
	let url = url.into_element();
	let discard_result = discard_result.into_element();
	let headers = headers.into_element();
	let header_map = parse_headers(&headers);
	let request = reqwest::Client::new().get(url).headers(header_map);

	if discard_result {
		#[cfg(target_family = "wasm")]
		wasm_bindgen_futures::spawn_local(async move {
			let _ = request.send().await;
		});
		#[cfg(all(not(target_family = "wasm"), feature = "tokio"))]
		tokio::spawn(async move {
			let _ = request.send().await;
		});
		return Item::new_from_element(String::new());
	}

	let Ok(response) = request.send().await else {
		return Item::new_from_element(String::new());
	};
	Item::new_from_element(response.text().await.ok().unwrap_or_default())
}

/// Sends an HTTP POST request to a specified URL with the provided binary data and optionally waits for the response (unless discarded) which is output as a string.
#[node_macro::node(category("Web Request"))]
async fn post_request(
	_: impl Ctx,
	_primary: Item<()>,
	/// The web address to send the POST request to.
	#[name("URL")]
	url: Item<String>,
	/// The binary data to include in the body of the POST request.
	body: Item<List<u8>>,
	/// Makes the request run in the background without waiting on a response. This is useful for triggering webhooks without blocking the continued execution of the graph.
	discard_result: Item<bool>,
	#[widget(ParsedWidgetOverride::Custom = "text_area")] headers: Item<String>,
) -> Item<String> {
	let _primary = _primary.into_element();
	let url = url.into_element();
	let body = body.into_element();
	let discard_result = discard_result.into_element();
	let headers = headers.into_element();
	let mut header_map = parse_headers(&headers);
	header_map.insert("Content-Type", "application/octet-stream".parse().unwrap());
	let body_bytes: Vec<u8> = body.iter_element_values().copied().collect();
	let request = reqwest::Client::new().post(url).body(body_bytes).headers(header_map);

	if discard_result {
		#[cfg(target_family = "wasm")]
		wasm_bindgen_futures::spawn_local(async move {
			let _ = request.send().await;
		});
		#[cfg(all(not(target_family = "wasm"), feature = "tokio"))]
		tokio::spawn(async move {
			let _ = request.send().await;
		});
		return Item::new_from_element(String::new());
	}

	let Ok(response) = request.send().await else {
		return Item::new_from_element(String::new());
	};
	Item::new_from_element(response.text().await.ok().unwrap_or_default())
}

/// Converts a text string to raw binary data. Useful for transmission over HTTP or writing to files.
#[node_macro::node(category("Web Request"), name("String to Bytes"))]
fn string_to_bytes(_: impl Ctx, string: Item<String>) -> Item<List<u8>> {
	let string = string.into_element();
	Item::new_from_element(string.into_bytes().into_iter().map(Item::new_from_element).collect())
}

/// Converts extracted raw RGBA pixel data from an input image. Each pixel becomes 4 sequential bytes. Useful for transmission over HTTP or writing to files.
#[node_macro::node(category("Web Request"), name("Image to Bytes"))]
fn image_to_bytes(_: impl Ctx, image: Item<List<Raster<CPU>>>) -> Item<List<u8>> {
	let image = image.into_element();
	let Some(image) = image.element(0) else { return Item::new_from_element(List::new()) };
	Item::new_from_element(image.data.iter().flat_map(|color| color.to_rgba8_srgb()).map(Item::new_from_element).collect())
}

/// Loads binary from URLs and local asset paths. Returns a transparent placeholder if the resource fails to load, allowing rendering to continue.
#[node_macro::node(category("Web Request"))]
async fn load_resource<'a: 'n>(_: impl Ctx, _primary: Item<()>, #[scope("editor-api")] editor_resources: Item<&'a PlatformEditorApi>, #[name("URL")] url: Item<String>) -> Item<Arc<[u8]>> {
	let _primary = _primary.into_element();
	let editor_resources = editor_resources.into_element();
	let url = url.into_element();
	let Some(api) = editor_resources.application_io.as_ref() else {
		return Item::new_from_element(Arc::from(include_bytes!("../../../graph-craft/src/null.png").to_vec()));
	};
	let Ok(data) = api.load_resource(url) else {
		return Item::new_from_element(Arc::from(include_bytes!("../../../graph-craft/src/null.png").to_vec()));
	};
	let Ok(data) = data.await else {
		return Item::new_from_element(Arc::from(include_bytes!("../../../graph-craft/src/null.png").to_vec()));
	};

	Item::new_from_element(data)
}

/// Converts raw binary data to a raster image.
///
/// Works with standard image format (PNG, JPEG, WebP, etc.). Automatically converts the color space to linear sRGB for accurate compositing.
#[node_macro::node(category("Web Request"))]
fn decode_image(_: impl Ctx, data: Item<Arc<[u8]>>) -> Item<List<Raster<CPU>>> {
	let data = data.into_element();
	let Some(image) = image::load_from_memory(data.as_ref()).ok() else {
		return Item::new_from_element(List::new());
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

	Item::new_from_element(List::new_from_element(Raster::new_cpu(image)))
}

#[cfg(target_family = "wasm")]
#[node_macro::node(category(""))]
async fn create_canvas(_: impl Ctx) -> Item<CanvasHandle> {
	Item::new_from_element(CanvasHandle::new())
}

/// Renders a view of the input graphic within an area defined by the *Footprint*.
#[cfg(target_family = "wasm")]
#[node_macro::node(category(""))]
async fn rasterize<T: WasmNotSend + Clone + 'n>(
	_: impl Ctx,
	#[implementations(
		Item<List<Vector>>,
		Item<List<Raster<CPU>>>,
		Item<List<Graphic>>,
		Item<List<Color>>,
		Item<List<GradientStops>>,
	)]
	data: Item<List<T>>,
	footprint: Item<Footprint>,
	canvas: Item<CanvasHandle>,
) -> Item<List<Raster<CPU>>>
where
	List<T>: Render + Clone + graphic_types::IntoGraphicList,
{
	let mut data = data.into_element();
	let footprint = footprint.into_element();
	let mut canvas = canvas.into_element();
	use glam::{DAffine2, DVec2};

	if footprint.transform.matrix2.determinant() == 0. {
		log::trace!("Invalid footprint received for rasterization");
		return Item::new_from_element(List::new());
	}

	// Snapshot the input as a List<Graphic> so the renderer can recurse into the original child layers
	// when collecting metadata, exposing their click targets to editor tools (same mechanism as Boolean Operation).
	let upstream_graphic_list = data.clone().into_graphic_list();

	let mut render = SvgRender::new();
	let aabb = Bbox::from_transform(footprint.transform).to_axis_aligned_bbox();
	let size = aabb.size();
	let resolution = footprint.resolution;
	let render_params = RenderParams {
		footprint,
		for_export: true,
		..Default::default()
	};

	for transform in data.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
		*transform = DAffine2::from_translation(-aabb.start) * *transform;
	}
	data.render_svg(&mut render, &render_params);
	render.format_svg(DVec2::ZERO, size);
	let svg_string = render.svg.to_svg_string();

	canvas.set_resolution(resolution);
	let context = canvas.context();

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
	Item::new_from_element(List::new_from_item(
		Item::new_from_element(Raster::new_cpu(image))
			.with_attribute(ATTR_TRANSFORM, footprint.transform)
			.with_attribute(ATTR_EDITOR_MERGED_LAYERS, upstream_graphic_list),
	))
}
