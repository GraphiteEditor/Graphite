use dyn_any::{DynAny, StaticType};
use glam::DVec2;
use graphene_core::Color;
use std::fmt::Debug;
use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc, Mutex,
};

#[derive(Default, Debug, Clone, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImaginateCache(Arc<Mutex<graphene_core::raster::Image<Color>>>);

impl ImaginateCache {
	pub fn into_inner(self) -> Arc<Mutex<graphene_core::raster::Image<Color>>> {
		self.0
	}
}

impl std::cmp::PartialEq for ImaginateCache {
	fn eq(&self, other: &Self) -> bool {
		Arc::ptr_eq(&self.0, &other.0)
	}
}

impl core::hash::Hash for ImaginateCache {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.0.lock().unwrap().hash(state);
	}
}

#[derive(Default, Debug, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct LocalImaginateController {
	status: Mutex<ImaginateStatus>,
	trigger_regenerate: AtomicBool,
}

#[derive(Default, Debug, Clone, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImaginateController(Arc<LocalImaginateController>);

impl ImaginateController {
	pub fn get_status(&self) -> ImaginateStatus {
		*self.0.status.lock().unwrap()
	}

	pub fn set_status(&self, status: ImaginateStatus) {
		*self.0.status.lock().unwrap() = status
	}

	pub fn get_trigger(&self) -> bool {
		self.0.trigger_regenerate.swap(false, Ordering::SeqCst)
	}

	pub fn set_trigger(&self, trigger: bool) {
		self.0.trigger_regenerate.store(trigger, Ordering::SeqCst)
	}
}

impl std::cmp::PartialEq for ImaginateController {
	fn eq(&self, other: &Self) -> bool {
		Arc::ptr_eq(&self.0, &other.0)
	}
}

impl core::hash::Hash for ImaginateController {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.0.status.lock().unwrap().hash(state);
	}
}

#[derive(Default, Debug, Clone, Copy, PartialEq, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImaginateStatus {
	#[default]
	Idle,
	Beginning,
	Uploading,
	Generating(f64),
	Terminating,
	Terminated,
}

#[allow(clippy::derived_hash_with_manual_eq)]
impl core::hash::Hash for ImaginateStatus {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		match self {
			Self::Idle => 0.hash(state),
			Self::Beginning => 1.hash(state),
			Self::Uploading => 2.hash(state),
			Self::Generating(f) => {
				3.hash(state);
				f.to_bits().hash(state);
			}
			Self::Terminating => 4.hash(state),
			Self::Terminated => 5.hash(state),
		}
	}
}

#[derive(Debug, Clone, PartialEq, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImaginateBaseImage {
	pub mime: String,
	#[cfg_attr(feature = "serde", serde(rename = "imageData"))]
	pub image_data: Vec<u8>,
	pub size: DVec2,
}

#[derive(Debug, Clone, PartialEq, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImaginateMaskImage {
	pub svg: String,
	pub size: DVec2,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, specta::Type, Hash)]
pub enum ImaginateMaskPaintMode {
	#[default]
	Inpaint,
	Outpaint,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, DynAny, specta::Type, Hash)]
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
			ImaginateMaskStartingFill::Original => write!(f, "Original Input Image"),
			ImaginateMaskStartingFill::LatentNoise => write!(f, "Randomness (Latent Noise)"),
			ImaginateMaskStartingFill::LatentNothing => write!(f, "Neutral (Latent Nothing)"),
		}
	}
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, DynAny, specta::Type, Hash)]
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

#[derive(Debug, Clone, PartialEq, specta::Type)]
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Hash, specta::Type)]
pub struct ImaginatePreferences {
	pub host_name: String,
}

impl graphene_core::application_io::GetImaginatePreferences for ImaginatePreferences {
	fn get_host_name(&self) -> &str {
		&self.host_name
	}
}

impl Default for ImaginatePreferences {
	fn default() -> Self {
		Self {
			host_name: "http://localhost:7860/".into(),
		}
	}
}

unsafe impl dyn_any::StaticType for ImaginatePreferences {
	type Static = ImaginatePreferences;
}
