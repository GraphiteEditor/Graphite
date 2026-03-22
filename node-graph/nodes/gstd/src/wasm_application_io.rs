use core_types::table::Table;
use core_types::{Color, Ctx};
pub use graph_craft::document::value::RenderOutputType;
pub use graph_craft::wasm_application_io::*;
use graphene_application_io::ApplicationIo;
use graphic_types::raster_types::Image;
use graphic_types::raster_types::{CPU, Raster};
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
	body: Vec<u8>,
	/// Makes the request run in the background without waiting on a response. This is useful for triggering webhooks without blocking the continued execution of the graph.
	discard_result: bool,
	#[widget(ParsedWidgetOverride::Custom = "text_area")] headers: String,
) -> String {
	let mut header_map = parse_headers(&headers);
	header_map.insert("Content-Type", "application/octet-stream".parse().unwrap());
	let request = reqwest::Client::new().post(url).body(body).headers(header_map);

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
fn string_to_bytes(_: impl Ctx, string: String) -> Vec<u8> {
	string.into_bytes()
}

/// Converts extracted raw RGBA pixel data from an input image. Each pixel becomes 4 sequential bytes. Useful for transmission over HTTP or writing to files.
#[node_macro::node(category("Web Request"), name("Image to Bytes"))]
fn image_to_bytes(_: impl Ctx, image: Table<Raster<CPU>>) -> Vec<u8> {
	let Some(image) = image.iter().next() else { return vec![] };
	image.element.data.iter().flat_map(|color| color.to_rgb8_srgb().into_iter()).collect::<Vec<u8>>()
}

/// Loads binary from URLs and local asset paths. Returns a transparent placeholder if the resource fails to load, allowing rendering to continue.
#[node_macro::node(category("Web Request"))]
async fn load_resource<'a: 'n>(_: impl Ctx, _primary: (), #[scope("editor-api")] editor_resources: &'a WasmEditorApi, #[name("URL")] url: String) -> Arc<[u8]> {
	let Some(api) = editor_resources.application_io.as_ref() else {
		return Arc::from(include_bytes!("../../../graph-craft/src/null.png").to_vec());
	};
	let Ok(data) = api.load_resource(url) else {
		return Arc::from(include_bytes!("../../../graph-craft/src/null.png").to_vec());
	};
	let Ok(data) = data.await else {
		return Arc::from(include_bytes!("../../../graph-craft/src/null.png").to_vec());
	};

	data
}

/// Converts raw binary data to a raster image.
///
/// Works with standard image format (PNG, JPEG, WebP, etc.). Automatically converts the color space to linear sRGB for accurate compositing.
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
