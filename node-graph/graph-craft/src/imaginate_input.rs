#[cfg(feature = "serde")]
mod base64_serde {
	use serde::{Deserialize, Deserializer, Serializer};

	pub fn as_base64<S>(key: &std::sync::Arc<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(&base64::encode(key.as_slice()))
	}

	pub fn from_base64<'a, D>(deserializer: D) -> Result<std::sync::Arc<Vec<u8>>, D::Error>
	where
		D: Deserializer<'a>,
	{
		use serde::de::Error;

		String::deserialize(deserializer)
			.and_then(|string| base64::decode(string).map_err(|err| Error::custom(err.to_string())))
			.map(std::sync::Arc::new)
			.map_err(serde::de::Error::custom)
	}
}

use dyn_any::{DynAny, StaticType};
use glam::DVec2;
use std::fmt::Debug;

#[derive(Clone, PartialEq, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImaginateInput {
	// User-configurable layer parameters
	pub seed: u64,
	pub samples: u32,
	pub sampling_method: ImaginateSamplingMethod,
	pub use_img2img: bool,
	pub denoising_strength: f64,
	pub mask_layer_ref: Option<Vec<u64>>,
	pub mask_paint_mode: ImaginateMaskPaintMode,
	pub mask_blur_px: u32,
	pub mask_fill_content: ImaginateMaskStartingFill,
	pub cfg_scale: f64,
	pub prompt: String,
	pub negative_prompt: String,
	pub restore_faces: bool,
	pub tiling: bool,

	pub image_data: Option<ImaginateImageData>,
	pub mime: String,
	/// 0 is not started, 100 is complete.
	pub percent_complete: f64,

	// TODO: Have the browser dispose of this blob URL when this is dropped (like when the layer is deleted)
	#[cfg_attr(feature = "serde", serde(skip))]
	pub blob_url: Option<String>,
	#[cfg_attr(feature = "serde", serde(skip))]
	pub status: ImaginateStatus,
	#[cfg_attr(feature = "serde", serde(skip))]
	pub dimensions: DVec2,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImaginateStatus {
	#[default]
	Idle,
	Beginning,
	Uploading(f64),
	Generating,
	Terminating,
	Terminated,
}

#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImaginateImageData {
	#[cfg_attr(feature = "serde", serde(serialize_with = "base64_serde::as_base64", deserialize_with = "base64_serde::from_base64"))]
	pub image_data: std::sync::Arc<Vec<u8>>,
}

impl Debug for ImaginateImageData {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("[image data...]")
	}
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImaginateBaseImage {
	pub mime: String,
	#[serde(rename = "imageData")]
	pub image_data: Vec<u8>,
	pub size: DVec2,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImaginateMaskImage {
	pub svg: String,
	pub size: DVec2,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum ImaginateMaskPaintMode {
	#[default]
	Inpaint,
	Outpaint,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, DynAny)]
pub enum ImaginateMaskStartingFill {
	#[default]
	Fill,
	Original,
	LatentNoise,
	LatentNothing,
}

impl ImaginateMaskStartingFill {
	pub fn list() -> [ImaginateMaskStartingFill; 4] {
		[
			ImaginateMaskStartingFill::Fill,
			ImaginateMaskStartingFill::Original,
			ImaginateMaskStartingFill::LatentNoise,
			ImaginateMaskStartingFill::LatentNothing,
		]
	}
}

impl std::fmt::Display for ImaginateMaskStartingFill {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ImaginateMaskStartingFill::Fill => write!(f, "Smeared Surroundings"),
			ImaginateMaskStartingFill::Original => write!(f, "Original Base Image"),
			ImaginateMaskStartingFill::LatentNoise => write!(f, "Randomness (Latent Noise)"),
			ImaginateMaskStartingFill::LatentNothing => write!(f, "Neutral (Latent Nothing)"),
		}
	}
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImaginateSamplingMethod {
	#[default]
	EulerA,
	Euler,
	LMS,
	Heun,
	DPM2,
	DPM2A,
	DPMPlusPlus2sA,
	DPMPlusPlus2m,
	DPMFast,
	DPMAdaptive,
	LMSKarras,
	DPM2Karras,
	DPM2AKarras,
	DPMPlusPlus2sAKarras,
	DPMPlusPlus2mKarras,
	DDIM,
	PLMS,
}

impl ImaginateSamplingMethod {
	pub fn api_value(&self) -> &str {
		match self {
			ImaginateSamplingMethod::EulerA => "Euler a",
			ImaginateSamplingMethod::Euler => "Euler",
			ImaginateSamplingMethod::LMS => "LMS",
			ImaginateSamplingMethod::Heun => "Heun",
			ImaginateSamplingMethod::DPM2 => "DPM2",
			ImaginateSamplingMethod::DPM2A => "DPM2 a",
			ImaginateSamplingMethod::DPMPlusPlus2sA => "DPM++ 2S a",
			ImaginateSamplingMethod::DPMPlusPlus2m => "DPM++ 2M",
			ImaginateSamplingMethod::DPMFast => "DPM fast",
			ImaginateSamplingMethod::DPMAdaptive => "DPM adaptive",
			ImaginateSamplingMethod::LMSKarras => "LMS Karras",
			ImaginateSamplingMethod::DPM2Karras => "DPM2 Karras",
			ImaginateSamplingMethod::DPM2AKarras => "DPM2 a Karras",
			ImaginateSamplingMethod::DPMPlusPlus2sAKarras => "DPM++ 2S a Karras",
			ImaginateSamplingMethod::DPMPlusPlus2mKarras => "DPM++ 2M Karras",
			ImaginateSamplingMethod::DDIM => "DDIM",
			ImaginateSamplingMethod::PLMS => "PLMS",
		}
	}

	pub fn list() -> [ImaginateSamplingMethod; 17] {
		[
			ImaginateSamplingMethod::EulerA,
			ImaginateSamplingMethod::Euler,
			ImaginateSamplingMethod::LMS,
			ImaginateSamplingMethod::Heun,
			ImaginateSamplingMethod::DPM2,
			ImaginateSamplingMethod::DPM2A,
			ImaginateSamplingMethod::DPMPlusPlus2sA,
			ImaginateSamplingMethod::DPMPlusPlus2m,
			ImaginateSamplingMethod::DPMFast,
			ImaginateSamplingMethod::DPMAdaptive,
			ImaginateSamplingMethod::LMSKarras,
			ImaginateSamplingMethod::DPM2Karras,
			ImaginateSamplingMethod::DPM2AKarras,
			ImaginateSamplingMethod::DPMPlusPlus2sAKarras,
			ImaginateSamplingMethod::DPMPlusPlus2mKarras,
			ImaginateSamplingMethod::DDIM,
			ImaginateSamplingMethod::PLMS,
		]
	}
}

impl std::fmt::Display for ImaginateSamplingMethod {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ImaginateSamplingMethod::EulerA => write!(f, "Euler A (Recommended)"),
			ImaginateSamplingMethod::Euler => write!(f, "Euler"),
			ImaginateSamplingMethod::LMS => write!(f, "LMS"),
			ImaginateSamplingMethod::Heun => write!(f, "Heun"),
			ImaginateSamplingMethod::DPM2 => write!(f, "DPM2"),
			ImaginateSamplingMethod::DPM2A => write!(f, "DPM2 A"),
			ImaginateSamplingMethod::DPMPlusPlus2sA => write!(f, "DPM++ 2S a"),
			ImaginateSamplingMethod::DPMPlusPlus2m => write!(f, "DPM++ 2M"),
			ImaginateSamplingMethod::DPMFast => write!(f, "DPM Fast"),
			ImaginateSamplingMethod::DPMAdaptive => write!(f, "DPM Adaptive"),
			ImaginateSamplingMethod::LMSKarras => write!(f, "LMS Karras"),
			ImaginateSamplingMethod::DPM2Karras => write!(f, "DPM2 Karras"),
			ImaginateSamplingMethod::DPM2AKarras => write!(f, "DPM2 A Karras"),
			ImaginateSamplingMethod::DPMPlusPlus2sAKarras => write!(f, "DPM++ 2S a Karras"),
			ImaginateSamplingMethod::DPMPlusPlus2mKarras => write!(f, "DPM++ 2M Karras"),
			ImaginateSamplingMethod::DDIM => write!(f, "DDIM"),
			ImaginateSamplingMethod::PLMS => write!(f, "PLMS"),
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImaginateGenerationParameters {
	pub seed: u64,
	pub samples: u32,
	/// Use `ImaginateSamplingMethod::api_value()` to generate this string
	#[cfg_attr(feature = "serde", serde(rename = "samplingMethod"))]
	pub sampling_method: String,
	#[cfg_attr(feature = "serde", serde(rename = "denoisingStrength"))]
	pub image_creativity: Option<f64>,
	#[cfg_attr(feature = "serde", serde(rename = "cfgScale"))]
	pub text_guidance: f64,
	#[cfg_attr(feature = "serde", serde(rename = "prompt"))]
	pub text_prompt: String,
	#[cfg_attr(feature = "serde", serde(rename = "negativePrompt"))]
	pub negative_prompt: String,
	pub resolution: (u32, u32),
	#[cfg_attr(feature = "serde", serde(rename = "restoreFaces"))]
	pub restore_faces: bool,
	pub tiling: bool,
}

impl Default for ImaginateInput {
	fn default() -> Self {
		Self {
			seed: 0,
			samples: 30,
			sampling_method: Default::default(),
			use_img2img: false,
			denoising_strength: 0.66,
			mask_paint_mode: ImaginateMaskPaintMode::default(),
			mask_layer_ref: None,
			mask_blur_px: 4,
			mask_fill_content: ImaginateMaskStartingFill::default(),
			cfg_scale: 10.,
			prompt: "".into(),
			negative_prompt: "".into(),
			restore_faces: false,
			tiling: false,

			image_data: None,
			mime: "image/png".into(),

			blob_url: None,
			percent_complete: 0.,
			status: Default::default(),
			dimensions: Default::default(),
		}
	}
}
