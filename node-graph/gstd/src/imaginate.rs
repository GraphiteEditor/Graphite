use core::future::Future;
use glam::DVec2;
use graph_craft::imaginate_input::{ImaginateMaskStartingFill, ImaginatePreferences, ImaginateSamplingMethod, ImaginateStatus};
use graphene_core::raster::{Image, Pixel};

const PROGRESS_EVERY_N_STEPS: u32 = 5;

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
struct ImaginateTextToImageRequest {
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
	sampler_index: ImaginateSamplingMethod,
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
	_image_creativity: impl Future<Output = f64>,
	_masking_layer: impl Future<Output = Option<Vec<u64>>>,
	_inpaint: impl Future<Output = bool>,
	_mask_blur: impl Future<Output = f64>,
	_mask_starting_fill: impl Future<Output = ImaginateMaskStartingFill>,
	improve_faces: impl Future<Output = bool>,
	tiling: impl Future<Output = bool>,
	_percent_complete: impl Future<Output = f64>,
	_status: impl Future<Output = ImaginateStatus>,
) -> Image<P> {
	if adapt_input_image.await {
		todo!("imaginate: adapt input image")
	} else {
		info!("properties: {:?}", preferences.await);
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
			sampler_index: sampling_method.await,
		};
		info!("request data: {request_data:?}")
	}
	info!("got an image: {}x{}", image.width, image.height);
	Image::new(100, 100, P::from_bytes(&f32::to_bits(0.5).to_ne_bytes()))
}
