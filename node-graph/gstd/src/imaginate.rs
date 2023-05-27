use core::future::Future;
use glam::DVec2;
use graph_craft::imaginate_input::{ImaginateMaskStartingFill, ImaginatePreferences, ImaginateSamplingMethod, ImaginateStatus};
use graphene_core::raster::{Image, Pixel};
use reqwest::Url;

const PROGRESS_EVERY_N_STEPS: u32 = 5;
const SDAPI_TEXT_TO_IMAGE: &str = "sdapi/v1/txt2img";

#[derive(Debug)]
enum Error {
	UrlParse { text: String, err: <&'static str as TryInto<Url>>::Error },
	ClientBuild(reqwest::Error),
	RequestBuild(reqwest::Error),
	Request(reqwest::Error),
}

impl core::fmt::Display for Error {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		match self {
			Self::UrlParse { text, err } => write!(f, "invalid url '{text}' ({err})"),
			Self::ClientBuild(err) => write!(f, "failed to create a reqwest client ({err})"),
			Self::RequestBuild(err) => write!(f, "failed to create a reqwest request ({err})"),
			Self::Request(err) => write!(f, "request failed ({err})"),
		}
	}
}

impl std::error::Error for Error {}

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
		info!("request data: {request_data:?}");
		let url = base_url.join(SDAPI_TEXT_TO_IMAGE).map_err(|err| Error::UrlParse { text: base_url.clone().into(), err })?;
		info!("request url: {url}");
		client.post(url).json(&request_data)
	};

	let request = request_builder.header("Accept", "*/*").fetch_mode_no_cors().build().map_err(Error::RequestBuild)?;

	let response = client.execute(request).await.map_err(Error::Request)?;

	info!("got a response (code={}): {:?}", response.status(), response.text().await);

	Ok(Image::new(100, 100, P::from_bytes(&f32::to_bits(0.5).to_ne_bytes())))
}
