use core::any::TypeId;
use core::future::Future;
use futures::{future::Either, TryFutureExt};
use glam::DVec2;
use graph_craft::imaginate_input::{ImaginateMaskStartingFill, ImaginatePreferences, ImaginateSamplingMethod, ImaginateStatus};
use graphene_core::raster::{Color, Image, Luma, Pixel};
use reqwest::Url;

const PROGRESS_EVERY_N_STEPS: u32 = 5;
const SDAPI_TEXT_TO_IMAGE: &str = "sdapi/v1/txt2img";
const SDAPI_PROGRESS: &str = "sdapi/v1/progress?skip_current_image=true";

async fn wait_for_refresh_counter(secs: f64) {
	let timeout = (secs * 1000.).round() as _;
	let promise = js_sys::Promise::new(&mut |resolve, _| drop(web_sys::window().map(|w| w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, timeout))));
	let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

#[derive(Default, Debug, Clone, Copy)]
pub enum Progress {
	#[default]
	Idle,
	Generating(f32),
	Uploading,
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
	UnsupportedPixelType(&'static str),
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
			Self::UnsupportedPixelType(ty) => write!(f, "pixel type `{ty}` not supported for imaginate images"),
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
	progress: f32,
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

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct ImaginateTextToImageRequest<'a> {
	prompt: String,
	seed: f64,
	steps: u32,
	cfg_scale: f64,
	width: f64,
	height: f64,
	restore_faces: bool,
	tiling: bool,
	negative_prompt: String,
	override_settings: ImaginateTextToImageRequestOverrideSettings,
	sampler_index: &'a str,
}

#[cfg(feature = "imaginate")]
pub async fn imaginate<P: Pixel>(
	image: Image<P>,
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

// TODO: this function just serves as a marker. This should be replaced by code, that actually sends the progress to a client
fn set_progress(progress: Progress) {
	match progress {
		Progress::Idle => info!("imaginate progress: idling"),
		Progress::Generating(x) => info!("imaginate progress: image is being generated {:.1}%", x * 100.),
		Progress::Uploading => info!("imaginate progress: server is done generating, now uploading"),
	}
}

#[cfg(feature = "imaginate")]
async fn imaginate_maybe_fail<P: Pixel>(
	image: Image<P>,
	preferences: impl Future<Output = ImaginatePreferences>,
	seed: impl Future<Output = f64>,
	res: impl Future<Output = Option<DVec2>>,
	samples: impl Future<Output = u32>,
	sampling_method: impl Future<Output = ImaginateSamplingMethod>,
	prompt_guidance: impl Future<Output = f64>,
	prompt: impl Future<Output = String>,
	negative_prompt: impl Future<Output = String>,
	adapt_input_image: impl Future<Output = bool>,
	_image_creativity: impl Future<Output = f64>,
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

	let base_url: &str = &preferences.host_name;
	let base_url: Url = base_url.try_into().map_err(|err| Error::UrlParse { text: base_url.into(), err })?;

	let client = reqwest::ClientBuilder::new().build().map_err(Error::ClientBuild)?;

	let sampler_index = sampling_method.await;
	let sampler_index = sampler_index.api_value();

	let join_url = |path: &str| base_url.join(path).map_err(|err| Error::UrlParse { text: base_url.as_str().into(), err });

	let request_builder = if adapt_input_image.await {
		todo!("imaginate: adapt input image")
	} else {
		let res = res.await.unwrap_or_else(|| DVec2::new(image.width as _, image.height as _));
		let request_data = ImaginateTextToImageRequest {
			prompt: prompt.await,
			seed: seed.await,
			steps: samples.await,
			cfg_scale: prompt_guidance.await,
			width: res.x,
			height: res.y,
			restore_faces: improve_faces.await,
			tiling: tiling.await,
			negative_prompt: negative_prompt.await,
			override_settings: Default::default(),
			sampler_index,
		};
		let url = join_url(SDAPI_TEXT_TO_IMAGE)?;
		client.post(url).json(&request_data)
	};

	let request = request_builder.header("Accept", "*/*").build().map_err(Error::RequestBuild)?;

	let response_future = client.execute(request);

	let progress_url = join_url(SDAPI_PROGRESS)?;

	let mut time_future;
	let mut abort_handle;

	futures::pin_mut!(response_future);

	let response = loop {
		time_future = wait_for_refresh_counter(preferences.refresh_frequency);
		let progress_request = client.get(progress_url.clone()).header("Accept", "*/*").build().map_err(Error::RequestBuild)?;
		let progress_response_future = client.execute(progress_request).and_then(|response| response.json());
		let (progress_response_future, new_abort_handle) = futures::future::abortable(progress_response_future);
		abort_handle = new_abort_handle;

		futures::pin_mut!(time_future, progress_response_future);

		response_future = match futures::future::select(response_future, progress_response_future).await {
			Either::Left((response, _)) => break response,
			Either::Right((progress, response_future)) => {
				if let Ok(Ok(ProgressResponse { progress })) = progress {
					set_progress(Progress::Generating(progress));
				}
				match futures::future::select(response_future, time_future).await {
					Either::Left((response, _)) => break response,
					Either::Right(((), response_future)) => response_future,
				}
			}
		};
	};
	abort_handle.abort();
	let response = response.and_then(reqwest::Response::error_for_status).map_err(Error::Request)?;

	set_progress(Progress::Uploading);

	let ImageResponse { images } = response.json().await.map_err(Error::ResponseFormat)?;

	set_progress(Progress::Idle);

	let base64image = images.into_iter().next().ok_or(Error::NoImage)?;

	use base64::prelude::*;
	let png_data = BASE64_STANDARD.decode(base64image).map_err(Error::Base64Decode)?;
	let dyn_image = image::load_from_memory_with_format(&png_data, image::ImageFormat::Png).map_err(Error::ImageDecode)?;
	let (width, height) = (dyn_image.width(), dyn_image.height());

	// sadly we cannot use bytemucks cast functions here, because the image::Pixel types don't implement Pod
	let dyn_data: Box<dyn core::any::Any + 'static> = match TypeId::of::<P>() {
		id if id == TypeId::of::<Color>() => Box::new(
			dyn_image
				.into_rgba32f()
				.pixels()
				.map(|&image::Rgba([r, g, b, a])| Color::from_rgbaf32(r, g, b, a).unwrap_or(Color::BLACK))
				.collect::<Vec<_>>(),
		),
		id if id == TypeId::of::<Color>() => Box::new(dyn_image.to_luma32f().into_raw().into_iter().map(Luma).collect::<Vec<_>>()),
		_ => return Err(Error::UnsupportedPixelType(core::any::type_name::<P>())),
	};
	let result_image: Box<Vec<P>> = dyn_data.downcast().unwrap();

	Ok(Image { data: *result_image, width, height })
}
