#[cfg(target_family = "wasm")]
use base64::Engine;
#[cfg(target_family = "wasm")]
use canvas_utils::{Canvas, CanvasHandle};
use core_types::color::SRGBA8;
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
	_primary: (),
	/// The web address to send the GET request to.
	#[name("URL")]
	url: String,
	/// Makes the request run in the background without waiting on a response. This is useful for triggering webhooks without blocking the continued execution of the graph.
	discard_result: bool,
	#[widget(ParsedWidgetOverride::Custom = "text_area")] headers: String,
) -> String {
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
		return String::new();
	}

	let Ok(response) = request.send().await else {
		return String::new();
	};
	response.text().await.ok().unwrap_or_default()
}

/// Sends an HTTP POST request to a specified URL with the provided binary data and optionally waits for the response (unless discarded) which is output as a string.
#[node_macro::node(category("Web Request"))]
async fn post_request(
	_: impl Ctx,
	_primary: (),
	/// The web address to send the POST request to.
	#[name("URL")]
	url: String,
	/// The binary data to include in the body of the POST request.
	body: List<u8>,
	/// Makes the request run in the background without waiting on a response. This is useful for triggering webhooks without blocking the continued execution of the graph.
	discard_result: bool,
	#[widget(ParsedWidgetOverride::Custom = "text_area")] headers: String,
) -> String {
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
		return String::new();
	}

	let Ok(response) = request.send().await else {
		return String::new();
	};
	response.text().await.ok().unwrap_or_default()
}

/// Converts a text string to raw binary data. Useful for transmission over HTTP or writing to files.
#[node_macro::node(category("Web Request"), name("String to Bytes"))]
fn string_to_bytes(_: impl Ctx, string: String) -> List<u8> {
	string.into_bytes().into_iter().map(Item::new_from_element).collect()
}

/// Converts extracted raw RGBA pixel data from an input image. Each pixel becomes 4 sequential bytes. Useful for transmission over HTTP or writing to files.
#[node_macro::node(category("Web Request"), name("Image to Bytes"))]
fn image_to_bytes(_: impl Ctx, image: List<Raster<CPU>>) -> List<u8> {
	let Some(image) = image.element(0) else { return List::new() };
	image
		.data
		.iter()
		.flat_map(|color| {
			let SRGBA8 { red, green, blue, alpha } = (*color).into();
			[red, green, blue, alpha]
		})
		.map(Item::new_from_element)
		.collect()
}

/// Loads binary from URLs and local asset paths. Returns a transparent placeholder if the resource fails to load, allowing rendering to continue.
#[node_macro::node(category("Web Request"))]
async fn load_resource<'a: 'n>(_: impl Ctx, _primary: (), #[scope("editor-api")] _editor: &'a PlatformEditorApi, #[name("URL")] url: String) -> Arc<[u8]> {
	let placeholder = || -> Arc<[u8]> { Arc::from(Vec::<u8>::new()) };

	let response = match reqwest::Client::new().get(&url).send().await {
		Ok(response) => response,
		Err(error) => {
			log::error!("HTTP request for `{url}` failed: {error}");
			return placeholder();
		}
	};

	match response.bytes().await {
		Ok(bytes) => Arc::from(bytes.to_vec()),
		Err(error) => {
			log::error!("Failed to read HTTP response for `{url}`: {error}");
			placeholder()
		}
	}
}

/// Converts raw binary data to a raster image.
///
/// Works with standard image format (PNG, JPEG, WebP, etc.). Automatically converts the color space to linear sRGB for accurate compositing.
#[node_macro::node(category("Web Request"))]
fn decode_image(_: impl Ctx, data: Arc<[u8]>) -> List<Raster<CPU>> {
	let Some(image) = ::image::load_from_memory(data.as_ref()).ok() else {
		return List::new();
	};
	let image = image.to_rgba32f();
	let image = Image {
		data: image
			.chunks(4)
			.map(|pixel| {
				// Decoded bytes are unassociated gamma sRGB; premultiply in gamma then lift to linear
				let a = pixel[3];
				Color::from_gamma_srgb_channels(pixel[0] * a, pixel[1] * a, pixel[2] * a, a)
			})
			.collect(),
		width: image.width(),
		height: image.height(),
		..Default::default()
	};

	List::new_from_element(Raster::new_cpu(image))
}

#[cfg(target_family = "wasm")]
#[node_macro::node(category(""))]
async fn create_canvas(_: impl Ctx) -> CanvasHandle {
	CanvasHandle::new()
}

/// Renders a view of the input graphic within an area defined by the *Footprint*.
#[cfg(target_family = "wasm")]
#[node_macro::node(category(""))]
async fn rasterize<T: WasmNotSend + Clone + 'n>(
	_: impl Ctx,
	#[implementations(
		List<Vector>,
		List<Raster<CPU>>,
		List<Graphic>,
		List<Color>,
		List<GradientStops>,
	)]
	mut data: List<T>,
	footprint: Footprint,
	mut canvas: CanvasHandle,
) -> List<Raster<CPU>>
where
	List<T>: Render + Clone + graphic_types::IntoGraphicList,
{
	use glam::{DAffine2, DVec2};

	if footprint.transform.matrix2.determinant() == 0. {
		log::trace!("Invalid footprint received for rasterization");
		return List::new();
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

	let rasterized = context.get_image_data(0, 0, resolution.x as i32, resolution.y as i32).unwrap();

	let image = Image::from_image_data(&rasterized.data().0, resolution.x as u32, resolution.y as u32);
	List::new_from_item(
		Item::new_from_element(Raster::new_cpu(image))
			.with_attribute(ATTR_TRANSFORM, footprint.transform)
			.with_attribute(ATTR_EDITOR_MERGED_LAYERS, upstream_graphic_list),
	)
}

#[node_macro::node(category(""))]
pub async fn resource<'a: 'n>(_: impl Ctx, _primary: (), #[scope("editor-api")] editor_api: &'a PlatformEditorApi, hash: ResourceHash) -> Resource {
	let application_io = editor_api.application_io.as_ref().expect("ApplicationIo must be available when using resources");
	application_io.load_resource(hash).await.unwrap_or_else(|| {
		panic!("Resource {hash} not found");
	})
}

#[node_macro::node(category(""))]
pub fn image<'a: 'n>(_: impl Ctx, resource: Resource) -> List<Raster<CPU>> {
	let image_data = resource.as_ref();

	let Some(image) = ::image::load_from_memory(image_data).ok() else {
		return List::new();
	};
	let image = image.to_rgba32f();
	let image = Image {
		data: image
			.chunks(4)
			.map(|pixel| {
				let alpha = pixel[3];
				Color::from_gamma_srgb_channels(pixel[0] * alpha, pixel[1] * alpha, pixel[2] * alpha, alpha)
			})
			.collect(),
		width: image.width(),
		height: image.height(),
		..Default::default()
	};
	List::new_from_element(Raster::new_cpu(image))
}
