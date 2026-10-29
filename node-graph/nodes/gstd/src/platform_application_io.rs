use core_types::Ctx;
use core_types::color::SRGBA8;
use core_types::list::{Item, List};
use core_types::math::bbox::Bbox;
use core_types::ops::{Convert, ConvertAsync};
use core_types::transform::Footprint;
use core_types::{ATTR_EDITOR_MERGED_LAYERS, ATTR_TRANSFORM};
use glam::{DAffine2, UVec2};
pub use graph_craft::application_io::resource::{Resource, ResourceHash};
pub use graph_craft::application_io::*;
pub use graph_craft::document::value::RenderOutputType;
#[cfg(target_family = "wasm")]
pub use graphene_canvas_utils as canvas_utils;
use graphic_types::Graphic;
use graphic_types::raster_types::Image;
use graphic_types::raster_types::{CPU, GPU, Raster};
use rendering::{Render, RenderParams};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum HttpRequestError {
	#[error("`{url}` is not a valid URL: {source}")]
	InvalidUrl { url: String, source: url::ParseError },
	#[error("The header line `{0}` is not a valid `Name: Value` pair")]
	InvalidHeader(String),
	#[error("Request to `{url}` failed: {}", error_chain(.source))]
	Failed { url: String, source: reqwest::Error },
	#[error(transparent)]
	Unreachable(http::Error),
	#[error("Request to `{url}` was answered with {status}: {body}")]
	Status { url: String, status: reqwest::StatusCode, body: String },
}

impl HttpRequestError {
	fn failed(url: &str, error: impl Into<http::Error>) -> Self {
		match error.into() {
			http::Error::Request(error) => Self::Failed {
				url: url.to_string(),
				source: error.without_url(),
			},
			#[cfg(not(target_family = "wasm"))]
			error => Self::Unreachable(error),
		}
	}
}

fn error_chain(error: &(dyn std::error::Error + 'static)) -> String {
	std::iter::successors(Some(error), |error| error.source()).map(ToString::to_string).collect::<Vec<_>>().join(": ")
}

async fn send(url: &str, request: reqwest::Request) -> Result<reqwest::Response, HttpRequestError> {
	let response = http::send(request).await.map_err(|error| HttpRequestError::failed(url, error))?;

	let status = response.status();
	if status.is_client_error() || status.is_server_error() {
		let body = response.text().await.unwrap_or_default();
		return Err(HttpRequestError::Status {
			url: url.to_string(),
			status,
			body: body.trim().to_string(),
		});
	}

	Ok(response)
}

fn new_request(method: reqwest::Method, url: &str) -> Result<reqwest::Request, HttpRequestError> {
	let parsed_url = url.parse().map_err(|source| HttpRequestError::InvalidUrl { url: url.to_string(), source })?;
	Ok(reqwest::Request::new(method, parsed_url))
}

fn parse_headers(headers: &str) -> Result<reqwest::header::HeaderMap, HttpRequestError> {
	use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

	let mut header_map = HeaderMap::new();
	for line in headers.lines().map(str::trim).filter(|line| !line.is_empty()) {
		let header = line
			.split_once(':')
			.and_then(|(key, value)| Some((HeaderName::from_bytes(key.trim().as_bytes()).ok()?, HeaderValue::from_str(value.trim()).ok()?)));
		let Some((header_name, header_value)) = header else {
			return Err(HttpRequestError::InvalidHeader(line.to_string()));
		};
		header_map.insert(header_name, header_value);
	}
	Ok(header_map)
}

/// Sends an HTTP GET request to a specified URL and optionally waits for the response (unless discarded) which is output as a string.
#[node_macro::node(category("Data: I/O"), icon("NodeGetRequest"), placeholder = pending_text)]
async fn get_request(
	_: impl Ctx,
	_primary: (),
	/// The web address to send the GET request to.
	#[name("URL")]
	url: Item<String>,
	/// Makes the request run in the background without waiting on a response. This is useful for triggering webhooks without blocking the continued execution of the graph.
	discard_result: Item<bool>,
	#[widget(ParsedWidgetOverride::Custom = "text_area")] headers: Item<String>,
) -> Result<Item<String>, HttpRequestError> {
	let (url, headers) = (url.into_element(), headers.into_element());
	let discard_result = *discard_result.element();

	if url.trim().is_empty() {
		return Ok(Item::default());
	}

	let header_map = parse_headers(&headers)?;
	let mut request = new_request(reqwest::Method::GET, &url)?;
	*request.headers_mut() = header_map;

	let response = send(&url, request).await?;
	if discard_result {
		return Ok(Item::default());
	}

	let text = response.text().await.map_err(|error| HttpRequestError::failed(&url, error))?;
	Ok(Item::new_from_element(text))
}

fn pending_text(_primary: &(), _url: &Item<String>, _discard_result: &Item<bool>, _headers: &Item<String>) -> Item<String> {
	Item::default()
}

/// Sends an HTTP POST request to a specified URL with the provided binary data and optionally waits for the response (unless discarded) which is output as a string.
#[node_macro::node(category("Data: I/O"), icon("NodePostRequest"), placeholder = pending_post_text)]
async fn post_request(
	_: impl Ctx,
	_primary: (),
	/// The web address to send the POST request to.
	#[name("URL")]
	url: Item<String>,
	/// The binary data to include in the body of the POST request.
	body: Item<Resource>,
	/// Makes the request run in the background without waiting on a response. This is useful for triggering webhooks without blocking the continued execution of the graph.
	discard_result: Item<bool>,
	#[widget(ParsedWidgetOverride::Custom = "text_area")] headers: Item<String>,
) -> Result<Item<String>, HttpRequestError> {
	let (url, headers) = (url.into_element(), headers.into_element());
	let discard_result = *discard_result.element();

	if url.trim().is_empty() {
		return Ok(Item::default());
	}

	let mut header_map = parse_headers(&headers)?;
	header_map
		.entry(reqwest::header::CONTENT_TYPE)
		.or_insert(reqwest::header::HeaderValue::from_static("application/octet-stream"));
	let body_bytes: Vec<u8> = body.element().as_ref().to_vec();
	let mut request = new_request(reqwest::Method::POST, &url)?;
	*request.headers_mut() = header_map;
	*request.body_mut() = Some(body_bytes.into());

	let response = send(&url, request).await?;
	if discard_result {
		return Ok(Item::default());
	}

	let text = response.text().await.map_err(|error| HttpRequestError::failed(&url, error))?;
	Ok(Item::new_from_element(text))
}

fn pending_post_text(_primary: &(), _url: &Item<String>, _body: &Item<Resource>, _discard_result: &Item<bool>, _headers: &Item<String>) -> Item<String> {
	Item::default()
}

/// Converts a text string to raw binary data. Useful for transmission over HTTP or writing to files.
#[node_macro::node(category("Data: I/O"), icon("NodeStringToBytes"), name("String to Bytes"))]
fn string_to_bytes(_: impl Ctx, string: Item<String>) -> Item<Resource> {
	Item::new_from_element(Resource::new(string.into_element().into_bytes()))
}

/// Converts extracted raw RGBA pixel data from an input image. Each pixel becomes 4 sequential bytes. Useful for transmission over HTTP or writing to files.
#[node_macro::node(category("Data: I/O"), icon("NodeImageToBytes"), name("Image to Bytes"))]
fn image_to_bytes(_: impl Ctx, image: Item<Raster<CPU>>) -> Item<Resource> {
	let bytes: Vec<u8> = image
		.element()
		.data
		.iter()
		.flat_map(|color| {
			let SRGBA8 { red, green, blue, alpha } = (*color).into();
			[red, green, blue, alpha]
		})
		.collect();

	Item::new_from_element(Resource::new(bytes))
}

/// Loads binary from URLs and local asset paths.
#[node_macro::node(category("Data: I/O"), icon("NodeLoadResource"), placeholder = pending_resource)]
async fn load_resource(_: impl Ctx, _primary: (), #[name("URL")] url: Item<String>) -> Result<Item<Resource>, HttpRequestError> {
	let url = url.into_element();

	if url.trim().is_empty() {
		return Ok(Item::new_from_element(Resource::empty()));
	}

	let response = send(&url, new_request(reqwest::Method::GET, &url)?).await?;
	let bytes = response.bytes().await.map_err(|error| HttpRequestError::failed(&url, error))?;
	Ok(Item::new_from_element(Resource::new(bytes)))
}

fn pending_resource(_primary: &(), _url: &Item<String>) -> Item<Resource> {
	Item::new_from_element(Resource::empty())
}

/// Converts the raw byte data of an image file to a raster image.
///
/// Supports the formats: PNG, JPG, GIF, WEBP, TIFF, BMP, TGA, ICO, HDR, EXR.
#[node_macro::node(category("Data: I/O"), icon("NodeBytesToImage"), name("Bytes to Image"))]
fn bytes_to_image(_: impl Ctx, data: Item<Resource>) -> Item<Raster<CPU>> {
	let data = data.into_element();
	let Some(image) = Image::from_encoded(data.as_ref()) else { return Item::default() };

	Item::new_from_element(Raster::new_cpu(image))
}

/// Renders a view of the input graphic within an area defined by the *Footprint*.
#[node_macro::node(category(""), placeholder = pending_raster)]
async fn rasterize(_: impl Ctx, data: List<Graphic>, footprint: Item<Footprint>, #[scope(wgpu_executor::IDENTIFIER)] executor: Item<::wgpu_executor::WgpuExecutor>) -> List<Raster<CPU>> {
	rasterize_with_vello(data, footprint.into_element(), executor.into_element()).await
}

async fn rasterize_with_vello(data: List<Graphic>, footprint: Footprint, executor: ::wgpu_executor::WgpuExecutor) -> List<Raster<CPU>> {
	let resolution = footprint.resolution;
	if footprint.transform.matrix2.determinant() == 0. || resolution.cmpeq(UVec2::ZERO).any() {
		log::trace!("Invalid footprint received for rasterization");
		return List::new();
	}

	let aabb = Bbox::from_transform(footprint.transform).to_axis_aligned_bbox();
	let view = DAffine2::from_scale(resolution.as_dvec2() / aabb.size()) * DAffine2::from_translation(-aabb.start);
	let render_params = RenderParams {
		footprint,
		for_export: true,
		..Default::default()
	};
	let texture = match executor.render_vello(resolution, view, &mut |scene| data.render_to_vello(scene, DAffine2::IDENTITY, &render_params)) {
		Ok(texture) => texture,
		Err(error) => {
			log::error!("Failed to rasterize: {error}");
			return List::new();
		}
	};
	let raster = ConvertAsync::<Raster<CPU>, _>::convert(Raster::new_gpu(texture), Footprint::DEFAULT, executor).await;

	// The input's layers stay reachable so the renderer can expose their click targets to the editor's tools
	List::new_from_item(
		Item::new_from_element(raster)
			.with_attribute(ATTR_TRANSFORM, footprint.transform)
			.with_attribute(ATTR_EDITOR_MERGED_LAYERS, data),
	)
}

fn pending_raster(_data: &List<Graphic>, _footprint: &Item<Footprint>, _executor: &Item<::wgpu_executor::WgpuExecutor>) -> List<Raster<CPU>> {
	List::new()
}

#[node_macro::node(category(""), inject_scope)]
fn editor_api(_: impl Ctx, #[scope("editor-api")] editor_api: Item<Arc<PlatformEditorApi>>) -> Item<Arc<PlatformEditorApi>> {
	editor_api
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Application IO is unavailable")]
pub struct ApplicationIoError;

#[node_macro::node(category(""), inject_scope)]
fn application_io(_: impl Ctx, #[scope(editor_api::IDENTIFIER)] editor_api: Item<Arc<PlatformEditorApi>>) -> Result<Item<Arc<PlatformApplicationIo>>, ApplicationIoError> {
	editor_api.into_element().application_io.clone().map(Item::new_from_element).ok_or(ApplicationIoError)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Resource {0} was not found")]
pub struct ResourceError(ResourceHash);

#[node_macro::node(category(""), placeholder = pending_stored_resource)]
async fn resource(
	_: impl Ctx,
	/// The scope-provided application IO giving access to the platform's resource storage.
	#[scope(application_io::IDENTIFIER)]
	application_io: Item<Arc<PlatformApplicationIo>>,
	/// The content hash identifying which stored resource to load.
	hash: Item<ResourceHash>,
) -> Result<Item<Resource>, ResourceError> {
	let hash = hash.into_element();
	let resource = application_io.element().load_resource(hash).await.ok_or(ResourceError(hash))?;
	Ok(Item::new_from_element(resource))
}

fn pending_stored_resource(_application_io: &Item<Arc<PlatformApplicationIo>>, _hash: &Item<ResourceHash>) -> Item<Resource> {
	Item::new_from_element(Resource::empty())
}

#[node_macro::node(category(""), inject_scope)]
fn wgpu_executor(_: impl Ctx, #[scope(application_io::IDENTIFIER)] application_io: Item<Arc<PlatformApplicationIo>>) -> Result<Item<::wgpu_executor::WgpuExecutor>, ::wgpu_executor::NoGpuError> {
	application_io.element().gpu_executor().cloned().map(Item::new_from_element).ok_or(::wgpu_executor::NoGpuError)
}

/// Uploads image data from CPU memory into a GPU texture so that GPU-based nodes can process it.
#[node_macro::node(category("Raster"), icon("NodeUploadTexture"), memoize)]
fn upload_texture(_: impl Ctx, content: Item<Raster<CPU>>, #[scope(wgpu_executor::IDENTIFIER)] executor: Item<::wgpu_executor::WgpuExecutor>) -> Item<Raster<GPU>> {
	let executor = executor.into_element();
	let (raster, attributes) = content.into_parts();

	Item::from_parts(raster.convert(Footprint::DEFAULT, executor), attributes)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::Color;
	use glam::DVec2;

	#[tokio::test]
	async fn rasterize_renders_the_footprint_area_at_its_resolution() {
		let Some(executor) = ::wgpu_executor::WgpuExecutor::new().await else { return };

		let red = Item::new_from_element(Raster::new_cpu(Image::new(1, 1, Color::RED))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::new(500., 1000.)));
		let data = List::new_from_item(Item::new_from_element(Graphic::RasterCPU(Box::new(red))));
		let footprint = Footprint {
			transform: DAffine2::from_scale(DVec2::splat(1000.)),
			resolution: UVec2::splat(4),
			..Default::default()
		};

		let output = rasterize_with_vello(data, footprint, executor).await;

		let image = output.iter_element_values().next().expect("Rasterize should output one image").data();
		assert_eq!((image.width, image.height), (4, 4));
		for x in 0..4 {
			let pixel = image.data[(4 + x) as usize];
			let expected_red = x < 2;
			assert_eq!(pixel.a() > 0.5, expected_red, "pixel {x} should be {}", if expected_red { "red" } else { "transparent" });
		}
	}
}
