use crate::wasm_application_io::WasmEditorApi;
use core::any::TypeId;
use core::future::Future;
use futures::{future::Either, TryFutureExt};
use glam::DVec2;
use graph_craft::imaginate_input::{ImaginateMaskStartingFill, ImaginateOutputStatus, ImaginatePreferences, ImaginateSamplingMethod, ImaginateStatus};
use graphene_core::application_io::{NodeGraphUpdateMessage, NodeGraphUpdateSender};
use graphene_core::raster::{Color, Image, Luma, Pixel};
use image::{DynamicImage, ImageBuffer, ImageOutputFormat};
use reqwest::Url;

const PROGRESS_EVERY_N_STEPS: u32 = 5;
const SDAPI_TEXT_TO_IMAGE: &str = "sdapi/v1/txt2img";
const SDAPI_IMAGE_TO_IMAGE: &str = "sdapi/v1/img2img";
const SDAPI_PROGRESS: &str = "sdapi/v1/progress?skip_current_image=true";

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
pub async fn imaginate<'a, P: Pixel>(
	image: Image<P>,
	editor_api: impl Future<Output = WasmEditorApi<'a>>,
	output_status: impl Future<Output = ImaginateOutputStatus>,
	preferences: impl Future<Output = ImaginatePreferences>,
	seed: impl Future<Output = f64>,
	res: impl Future<Output = Option<DVec2>>,
	samples: impl Future<Output = u32>,
	sampling_method: impl Future<Output = ImaginateSamplingMethod>,
	prompt_guidance: impl Future<Output = f64>,
	prompt: impl Future<Output = String>,
	negative_prompt: impl Future<Output = String>,
	adapt_input_image: impl Future<Output = bool>,
	image_creativity: impl Future<Output = f64>,
	masking_layer: impl Future<Output = Option<Vec<u64>>>,
	inpaint: impl Future<Output = bool>,
	mask_blur: impl Future<Output = f64>,
	mask_starting_fill: impl Future<Output = ImaginateMaskStartingFill>,
	improve_faces: impl Future<Output = bool>,
	tiling: impl Future<Output = bool>,
	percent_complete: impl Future<Output = f64>,
	status: impl Future<Output = ImaginateStatus>,
) -> Image<P> {
	imaginate_maybe_fail(
		image,
		editor_api,
		output_status,
		preferences,
		seed,
		res,
		samples,
		sampling_method,
		prompt_guidance,
		prompt,
		negative_prompt,
		adapt_input_image,
		image_creativity,
		masking_layer,
		inpaint,
		mask_blur,
		mask_starting_fill,
		improve_faces,
		tiling,
		percent_complete,
		status,
	)
	.await
	.unwrap_or_else(|err| {
		error!("{err}");
		Image::empty()
	})
}

#[cfg(feature = "imaginate")]
async fn imaginate_maybe_fail<'a, P: Pixel>(
	image: Image<P>,
	editor_api: impl Future<Output = WasmEditorApi<'a>>,
	output_status: impl Future<Output = ImaginateOutputStatus>,
	preferences: impl Future<Output = ImaginatePreferences>,
	seed: impl Future<Output = f64>,
	res: impl Future<Output = Option<DVec2>>,
	samples: impl Future<Output = u32>,
	sampling_method: impl Future<Output = ImaginateSamplingMethod>,
	prompt_guidance: impl Future<Output = f64>,
	prompt: impl Future<Output = String>,
	negative_prompt: impl Future<Output = String>,
	adapt_input_image: impl Future<Output = bool>,
	image_creativity: impl Future<Output = f64>,
	_masking_layer: impl Future<Output = Option<Vec<u64>>>,
	_inpaint: impl Future<Output = bool>,
	_mask_blur: impl Future<Output = f64>,
	_mask_starting_fill: impl Future<Output = ImaginateMaskStartingFill>,
	improve_faces: impl Future<Output = bool>,
	tiling: impl Future<Output = bool>,
	_percent_complete: impl Future<Output = f64>,
	_status: impl Future<Output = ImaginateStatus>,
) -> Result<Image<P>, Error> {
	let preferences = preferences.await;

	let editor_api = editor_api.await;
	let output_status = output_status.await;
	let set_progress = |progress: ImaginateStatus| {
		output_status.set(progress);
		editor_api.node_graph_message_sender.send(NodeGraphUpdateMessage::ImaginateStatusUpdate);
	};

	let base_url: &str = &preferences.host_name;
	let base_url: Url = base_url.try_into().map_err(|err| Error::UrlParse { text: base_url.into(), err })?;

	let client = reqwest::ClientBuilder::new().build().map_err(Error::ClientBuild)?;

	let sampler_index = sampling_method.await;
	let sampler_index = sampler_index.api_value();

	let join_url = |path: &str| base_url.join(path).map_err(|err| Error::UrlParse { text: base_url.as_str().into(), err });

	let res = res.await.unwrap_or_else(|| DVec2::new(image.width as _, image.height as _));
	let common_request_data = ImaginateCommonImageRequest {
		prompt: prompt.await,
		seed: seed.await,
		steps: samples.await,
		cfg_scale: prompt_guidance.await,
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
			denoising_strength: image_creativity.await * 0.01,
			mask: None,
		};
		let url = join_url(SDAPI_IMAGE_TO_IMAGE)?;
		client.post(url).json(&request_data)
	} else {
		let request_data = ImaginateTextToImageRequest {
			common: common_request_data,
			override_settings: Default::default(),
		};
		let url = join_url(SDAPI_TEXT_TO_IMAGE)?;
		client.post(url).json(&request_data)
	};

	let request = request_builder.header("Accept", "*/*").build().map_err(Error::RequestBuild)?;

	let response_future = client.execute(request);

	let progress_url = join_url(SDAPI_PROGRESS)?;

	futures::pin_mut!(response_future);

	let response = loop {
		let progress_request = client.get(progress_url.clone()).header("Accept", "*/*").build().map_err(Error::RequestBuild)?;
		let progress_response_future = client.execute(progress_request).and_then(|response| response.json());
		let (progress_response_future, abort_handle) = futures::future::abortable(progress_response_future);

		futures::pin_mut!(progress_response_future);

		response_future = match futures::future::select(response_future, progress_response_future).await {
			Either::Left((response, _)) => {
				abort_handle.abort();
				break response;
			}
			Either::Right((progress, response_future)) => {
				if let Ok(Ok(ProgressResponse { progress })) = progress {
					set_progress(ImaginateStatus::Generating(progress * 100.));
				}
				response_future
			}
		};
	};
	let response = response.and_then(reqwest::Response::error_for_status).map_err(Error::Request)?;

	set_progress(ImaginateStatus::Uploading);

	let ImageResponse { images } = response.json().await.map_err(Error::ResponseFormat)?;

	set_progress(ImaginateStatus::Idle);

	images.into_iter().next().ok_or(Error::NoImage).and_then(|base64_data| base64_to_image(base64_data))
}

fn image_to_base64<P: Pixel>(image: Image<P>) -> Result<String, Error> {
	use base64::prelude::*;

	let Image { width, height, data } = image;

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

	Ok(Image { data: result_data, width, height })
}
