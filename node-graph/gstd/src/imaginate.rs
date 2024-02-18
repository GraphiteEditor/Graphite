use crate::wasm_application_io::WasmEditorApi;
use core::any::TypeId;
use core::future::Future;
use futures::{future::Either, TryFutureExt};
use glam::{DVec2, U64Vec2};
use graph_craft::imaginate_input::{ImaginateController, ImaginateMaskStartingFill, ImaginatePreferences, ImaginateSamplingMethod, ImaginateServerStatus, ImaginateStatus, ImaginateTerminationHandle};
use graphene_core::application_io::NodeGraphUpdateMessage;
use graphene_core::raster::{Color, Image, Luma, Pixel};
use image::{DynamicImage, ImageBuffer, ImageOutputFormat};
use reqwest::Url;

const PROGRESS_EVERY_N_STEPS: u32 = 5;
const SDAPI_TEXT_TO_IMAGE: &str = "sdapi/v1/txt2img";
const SDAPI_IMAGE_TO_IMAGE: &str = "sdapi/v1/img2img";
const SDAPI_PROGRESS: &str = "sdapi/v1/progress?skip_current_image=true";
const SDAPI_TERMINATE: &str = "sdapi/v1/interrupt";

fn new_client() -> Result<reqwest::Client, Error> {
	reqwest::ClientBuilder::new().build().map_err(Error::ClientBuild)
}

fn parse_url(url: &str) -> Result<Url, Error> {
	url.try_into().map_err(|err| Error::UrlParse { text: url.into(), err })
}

fn join_url(base_url: &Url, path: &str) -> Result<Url, Error> {
	base_url.join(path).map_err(|err| Error::UrlParse { text: base_url.to_string(), err })
}

fn new_get_request<U: reqwest::IntoUrl>(client: &reqwest::Client, url: U) -> Result<reqwest::Request, Error> {
	client.get(url).header("Accept", "*/*").build().map_err(Error::RequestBuild)
}

pub struct ImaginatePersistentData {
	pending_server_check: Option<futures::channel::oneshot::Receiver<reqwest::Result<reqwest::Response>>>,
	host_name: Url,
	client: Option<reqwest::Client>,
	server_status: ImaginateServerStatus,
}

impl core::fmt::Debug for ImaginatePersistentData {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		f.debug_struct(core::any::type_name::<Self>())
			.field("pending_server_check", &self.pending_server_check.is_some())
			.field("host_name", &self.host_name)
			.field("status", &self.server_status)
			.finish()
	}
}

impl Default for ImaginatePersistentData {
	fn default() -> Self {
		let mut status = ImaginateServerStatus::default();
		let client = new_client().map_err(|err| status = ImaginateServerStatus::Failed(err.to_string())).ok();
		let ImaginatePreferences { host_name } = Default::default();
		Self {
			pending_server_check: None,
			host_name: parse_url(&host_name).unwrap(),
			client,
			server_status: status,
		}
	}
}

type ImaginateFuture = core::pin::Pin<Box<dyn Future<Output = ()> + 'static>>;

impl ImaginatePersistentData {
	pub fn set_host_name(&mut self, name: &str) {
		match parse_url(name) {
			Ok(url) => self.host_name = url,
			Err(err) => self.server_status = ImaginateServerStatus::Failed(err.to_string()),
		}
	}

	fn initiate_server_check_maybe_fail(&mut self) -> Result<Option<ImaginateFuture>, Error> {
		use futures::future::FutureExt;
		let Some(client) = &self.client else {
			return Ok(None);
		};
		if self.pending_server_check.is_some() {
			return Ok(None);
		}
		self.server_status = ImaginateServerStatus::Checking;
		let url = join_url(&self.host_name, SDAPI_PROGRESS)?;
		let request = new_get_request(client, url)?;
		let (send, recv) = futures::channel::oneshot::channel();
		let response_future = client.execute(request).map(move |r| {
			let _ = send.send(r);
		});
		self.pending_server_check = Some(recv);
		Ok(Some(Box::pin(response_future)))
	}

	pub fn initiate_server_check(&mut self) -> Option<ImaginateFuture> {
		match self.initiate_server_check_maybe_fail() {
			Ok(f) => f,
			Err(err) => {
				self.server_status = ImaginateServerStatus::Failed(err.to_string());
				None
			}
		}
	}

	pub fn poll_server_check(&mut self) {
		if let Some(mut check) = self.pending_server_check.take() {
			self.server_status = match check.try_recv().map(|r| r.map(|r| r.and_then(reqwest::Response::error_for_status))) {
				Ok(Some(Ok(_response))) => ImaginateServerStatus::Connected,
				Ok(Some(Err(_))) | Err(_) => ImaginateServerStatus::Unavailable,
				Ok(None) => {
					self.pending_server_check = Some(check);
					ImaginateServerStatus::Checking
				}
			}
		}
	}

	pub fn server_status(&self) -> &ImaginateServerStatus {
		&self.server_status
	}

	pub fn is_checking(&self) -> bool {
		matches!(self.server_status, ImaginateServerStatus::Checking)
	}
}

#[derive(Debug)]
struct ImaginateFutureAbortHandle(futures::future::AbortHandle);

impl ImaginateTerminationHandle for ImaginateFutureAbortHandle {
	fn terminate(&self) {
		self.0.abort()
	}
}

#[derive(Debug)]
enum Error {
	UrlParse { text: String, err: <&'static str as TryInto<Url>>::Error },
	ClientBuild(reqwest::Error),
	RequestBuild(reqwest::Error),
	Request(reqwest::Error),
	ResponseFormat(reqwest::Error),
	NoImage,
	Base64Decode(base64::DecodeError),
	ImageDecode(image::error::ImageError),
	ImageEncode(image::error::ImageError),
	UnsupportedPixelType(&'static str),
	InconsistentImageSize,
	Terminated,
	TerminationFailed(reqwest::Error),
}

impl core::fmt::Display for Error {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		match self {
			Self::UrlParse { text, err } => write!(f, "invalid url '{text}' ({err})"),
			Self::ClientBuild(err) => write!(f, "failed to create a reqwest client ({err})"),
			Self::RequestBuild(err) => write!(f, "failed to create a reqwest request ({err})"),
			Self::Request(err) => write!(f, "request failed ({err})"),
			Self::ResponseFormat(err) => write!(f, "got an invalid API response ({err})"),
			Self::NoImage => write!(f, "got an empty API response"),
			Self::Base64Decode(err) => write!(f, "failed to decode base64 encoded image ({err})"),
			Self::ImageDecode(err) => write!(f, "failed to decode png image ({err})"),
			Self::ImageEncode(err) => write!(f, "failed to encode png image ({err})"),
			Self::UnsupportedPixelType(ty) => write!(f, "pixel type `{ty}` not supported for imaginate images"),
			Self::InconsistentImageSize => write!(f, "image width and height do not match the image byte size"),
			Self::Terminated => write!(f, "imaginate request was terminated by the user"),
			Self::TerminationFailed(err) => write!(f, "termination failed ({err})"),
		}
	}
}

impl std::error::Error for Error {}

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
struct ImageResponse {
	images: Vec<String>,
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
struct ProgressResponse {
	progress: f64,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct ImaginateTextToImageRequestOverrideSettings {
	show_progress_every_n_steps: u32,
}

impl Default for ImaginateTextToImageRequestOverrideSettings {
	fn default() -> Self {
		Self {
			show_progress_every_n_steps: PROGRESS_EVERY_N_STEPS,
		}
	}
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct ImaginateImageToImageRequestOverrideSettings {
	show_progress_every_n_steps: u32,
	img2img_fix_steps: bool,
}

impl Default for ImaginateImageToImageRequestOverrideSettings {
	fn default() -> Self {
		Self {
			show_progress_every_n_steps: PROGRESS_EVERY_N_STEPS,
			img2img_fix_steps: true,
		}
	}
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct ImaginateTextToImageRequest<'a> {
	#[serde(flatten)]
	common: ImaginateCommonImageRequest<'a>,
	override_settings: ImaginateTextToImageRequestOverrideSettings,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct ImaginateMask {
	mask: String,
	mask_blur: String,
	inpainting_fill: u32,
	inpaint_full_res: bool,
	inpainting_mask_invert: u32,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct ImaginateImageToImageRequest<'a> {
	#[serde(flatten)]
	common: ImaginateCommonImageRequest<'a>,
	override_settings: ImaginateImageToImageRequestOverrideSettings,

	init_images: Vec<String>,
	denoising_strength: f64,
	#[serde(flatten)]
	mask: Option<ImaginateMask>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct ImaginateCommonImageRequest<'a> {
	prompt: String,
	seed: f64,
	steps: u32,
	cfg_scale: f64,
	width: f64,
	height: f64,
	restore_faces: bool,
	tiling: bool,
	negative_prompt: String,
	sampler_index: &'a str,
}

#[cfg(feature = "imaginate")]
#[allow(clippy::too_many_arguments)]
pub async fn imaginate<'a, P: Pixel>(
	image: Image<P>,
	editor_api: impl Future<Output = WasmEditorApi<'a>>,
	controller: ImaginateController,
	seed: impl Future<Output = f64>,
	res: impl Future<Output = Option<DVec2>>,
	samples: impl Future<Output = u32>,
	sampling_method: impl Future<Output = ImaginateSamplingMethod>,
	prompt_guidance: impl Future<Output = f64>,
	prompt: impl Future<Output = String>,
	negative_prompt: impl Future<Output = String>,
	adapt_input_image: impl Future<Output = bool>,
	image_creativity: impl Future<Output = f64>,
	inpaint: impl Future<Output = bool>,
	mask_blur: impl Future<Output = f64>,
	mask_starting_fill: impl Future<Output = ImaginateMaskStartingFill>,
	improve_faces: impl Future<Output = bool>,
	tiling: impl Future<Output = bool>,
) -> Image<P> {
	let WasmEditorApi {
		node_graph_message_sender,
		imaginate_preferences,
		..
	} = editor_api.await;
	let set_progress = |progress: ImaginateStatus| {
		controller.set_status(progress);
		node_graph_message_sender.send(NodeGraphUpdateMessage::ImaginateStatusUpdate);
	};
	let host_name = imaginate_preferences.get_host_name();
	imaginate_maybe_fail(
		image,
		host_name,
		set_progress,
		&controller,
		seed,
		res,
		samples,
		sampling_method,
		prompt_guidance,
		prompt,
		negative_prompt,
		adapt_input_image,
		image_creativity,
		inpaint,
		mask_blur,
		mask_starting_fill,
		improve_faces,
		tiling,
	)
	.await
	.unwrap_or_else(|err| {
		match err {
			Error::Terminated => {
				set_progress(ImaginateStatus::Terminated);
			}
			err => {
				error!("{err}");
				set_progress(ImaginateStatus::Failed(err.to_string()));
			}
		};
		Image::empty()
	})
}

#[cfg(feature = "imaginate")]
#[allow(clippy::too_many_arguments)]
async fn imaginate_maybe_fail<'a, P: Pixel, F: Fn(ImaginateStatus)>(
	image: Image<P>,
	host_name: &str,
	set_progress: F,
	controller: &ImaginateController,
	seed: impl Future<Output = f64>,
	res: impl Future<Output = Option<DVec2>>,
	samples: impl Future<Output = u32>,
	sampling_method: impl Future<Output = ImaginateSamplingMethod>,
	prompt_guidance: impl Future<Output = f64>,
	prompt: impl Future<Output = String>,
	negative_prompt: impl Future<Output = String>,
	adapt_input_image: impl Future<Output = bool>,
	image_creativity: impl Future<Output = f64>,
	_inpaint: impl Future<Output = bool>,
	_mask_blur: impl Future<Output = f64>,
	_mask_starting_fill: impl Future<Output = ImaginateMaskStartingFill>,
	improve_faces: impl Future<Output = bool>,
	tiling: impl Future<Output = bool>,
) -> Result<Image<P>, Error> {
	set_progress(ImaginateStatus::Beginning);

	let base_url: Url = parse_url(host_name)?;

	let client = new_client()?;

	let sampler_index = sampling_method.await;
	let sampler_index = sampler_index.api_value();

	let res = res.await.unwrap_or_else(|| {
		let (width, height) = pick_safe_imaginate_resolution((image.width as _, image.height as _));
		DVec2::new(width as _, height as _)
	});
	let common_request_data = ImaginateCommonImageRequest {
		prompt: prompt.await,
		seed: seed.await,
		steps: samples.await,
		cfg_scale: prompt_guidance.await as f64,
		width: res.x,
		height: res.y,
		restore_faces: improve_faces.await,
		tiling: tiling.await,
		negative_prompt: negative_prompt.await,
		sampler_index,
	};
	let request_builder = if adapt_input_image.await {
		let base64_data = image_to_base64(image)?;
		let request_data = ImaginateImageToImageRequest {
			common: common_request_data,
			override_settings: Default::default(),

			init_images: vec![base64_data],
			denoising_strength: image_creativity.await as f64 * 0.01,
			mask: None,
		};
		let url = join_url(&base_url, SDAPI_IMAGE_TO_IMAGE)?;
		client.post(url).json(&request_data)
	} else {
		let request_data = ImaginateTextToImageRequest {
			common: common_request_data,
			override_settings: Default::default(),
		};
		let url = join_url(&base_url, SDAPI_TEXT_TO_IMAGE)?;
		client.post(url).json(&request_data)
	};

	let request = request_builder.header("Accept", "*/*").build().map_err(Error::RequestBuild)?;

	let (response_future, abort_handle) = futures::future::abortable(client.execute(request));
	controller.set_termination_handle(Box::new(ImaginateFutureAbortHandle(abort_handle)));

	let progress_url = join_url(&base_url, SDAPI_PROGRESS)?;

	futures::pin_mut!(response_future);

	let response = loop {
		let progress_request = new_get_request(&client, progress_url.clone())?;
		let progress_response_future = client.execute(progress_request).and_then(|response| response.json());

		futures::pin_mut!(progress_response_future);

		response_future = match futures::future::select(response_future, progress_response_future).await {
			Either::Left((response, _)) => break response,
			Either::Right((progress, response_future)) => {
				if let Ok(ProgressResponse { progress }) = progress {
					set_progress(ImaginateStatus::Generating(progress * 100.));
				}
				response_future
			}
		};
	};

	let response = match response {
		Ok(response) => response.and_then(reqwest::Response::error_for_status).map_err(Error::Request)?,
		Err(_aborted) => {
			set_progress(ImaginateStatus::Terminating);
			let url = join_url(&base_url, SDAPI_TERMINATE)?;
			let request = client.post(url).build().map_err(Error::RequestBuild)?;
			// The user probably doesn't really care if the server side was really aborted or if there was an network error.
			// So we fool them that the request was terminated if the termination request in reality failed.
			let _ = client.execute(request).await.and_then(reqwest::Response::error_for_status).map_err(Error::TerminationFailed)?;
			return Err(Error::Terminated);
		}
	};

	set_progress(ImaginateStatus::Uploading);

	let ImageResponse { images } = response.json().await.map_err(Error::ResponseFormat)?;

	let result = images.into_iter().next().ok_or(Error::NoImage).and_then(base64_to_image)?;

	set_progress(ImaginateStatus::ReadyDone);

	Ok(result)
}

fn image_to_base64<P: Pixel>(image: Image<P>) -> Result<String, Error> {
	use base64::prelude::*;

	let Image { width, height, data, .. } = image;

	fn cast_with_f32<S: Pixel, D: image::Pixel<Subpixel = f32>>(data: Vec<S>, width: u32, height: u32) -> Result<DynamicImage, Error>
	where
		DynamicImage: From<ImageBuffer<D, Vec<f32>>>,
	{
		ImageBuffer::<D, Vec<f32>>::from_raw(width, height, bytemuck::cast_vec(data))
			.ok_or(Error::InconsistentImageSize)
			.map(Into::into)
	}

	let image: DynamicImage = match TypeId::of::<P>() {
		id if id == TypeId::of::<Color>() => cast_with_f32::<_, image::Rgba<f32>>(data, width, height)?
			// we need to do this cast, because png does not support rgba32f
			.to_rgba16().into(),
		id if id == TypeId::of::<Luma>() => cast_with_f32::<_, image::Luma<f32>>(data, width, height)?
			// we need to do this cast, because png does not support luma32f
			.to_luma16().into(),
		_ => return Err(Error::UnsupportedPixelType(core::any::type_name::<P>())),
	};

	let mut png_data = std::io::Cursor::new(vec![]);
	image.write_to(&mut png_data, ImageOutputFormat::Png).map_err(Error::ImageEncode)?;
	Ok(BASE64_STANDARD.encode(png_data.into_inner()))
}

fn base64_to_image<D: AsRef<[u8]>, P: Pixel>(base64_data: D) -> Result<Image<P>, Error> {
	use base64::prelude::*;

	let png_data = BASE64_STANDARD.decode(base64_data).map_err(Error::Base64Decode)?;
	let dyn_image = image::load_from_memory_with_format(&png_data, image::ImageFormat::Png).map_err(Error::ImageDecode)?;
	let (width, height) = (dyn_image.width(), dyn_image.height());

	let result_data: Vec<P> = match TypeId::of::<P>() {
		id if id == TypeId::of::<Color>() => bytemuck::cast_vec(dyn_image.into_rgba32f().into_raw()),
		id if id == TypeId::of::<Luma>() => bytemuck::cast_vec(dyn_image.to_luma32f().into_raw()),
		_ => return Err(Error::UnsupportedPixelType(core::any::type_name::<P>())),
	};

	Ok(Image {
		data: result_data,
		width,
		height,
		base64_string: None,
	})
}

pub fn pick_safe_imaginate_resolution((width, height): (f64, f64)) -> (u64, u64) {
	const MAX_RESOLUTION: u64 = 1000 * 1000;

	// this is the maximum width/height that can be obtained
	const MAX_DIMENSION: u64 = (MAX_RESOLUTION / 64) & !63;

	// round the resolution to the nearest multiple of 64
	let size = (DVec2::new(width, height).round().clamp(DVec2::ZERO, DVec2::splat(MAX_DIMENSION as _)).as_u64vec2() + U64Vec2::splat(32)).max(U64Vec2::splat(64)) & !U64Vec2::splat(63);
	let resolution = size.x * size.y;

	if resolution > MAX_RESOLUTION {
		// scale down the image, so it is smaller than MAX_RESOLUTION
		let scale = (MAX_RESOLUTION as f64 / resolution as f64).sqrt();
		let size = size.as_dvec2() * scale;

		if size.x < 64.0 {
			// the image is extremely wide
			(64, MAX_DIMENSION)
		} else if size.y < 64.0 {
			// the image is extremely high
			(MAX_DIMENSION, 64)
		} else {
			// round down to a multiple of 64, so that the resolution still is smaller than MAX_RESOLUTION
			(size.as_u64vec2() & !U64Vec2::splat(63)).into()
		}
	} else {
		size.into()
	}
}
