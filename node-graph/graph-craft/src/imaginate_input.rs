use dyn_any::{DynAny, StaticType};
use graphene_core::Color;
use std::borrow::Cow;
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

pub trait ImaginateTerminationHandle: Debug + Send + Sync + 'static {
	fn terminate(&self);
}

#[derive(Default, Debug, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct InternalImaginateControl {
	#[serde(skip)]
	status: Mutex<ImaginateStatus>,
	trigger_regenerate: AtomicBool,
	#[serde(skip)]
	#[specta(skip)]
	termination_sender: Mutex<Option<Box<dyn ImaginateTerminationHandle>>>,
}

#[derive(Debug, Default, Clone, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImaginateController(Arc<InternalImaginateControl>);

impl ImaginateController {
	pub fn get_status(&self) -> ImaginateStatus {
		self.0.status.lock().as_deref().cloned().unwrap_or_default()
	}

	pub fn set_status(&self, status: ImaginateStatus) {
		if let Ok(mut lock) = self.0.status.lock() {
			*lock = status
		}
	}

	pub fn take_regenerate_trigger(&self) -> bool {
		self.0.trigger_regenerate.swap(false, Ordering::SeqCst)
	}

	pub fn trigger_regenerate(&self) {
		self.0.trigger_regenerate.store(true, Ordering::SeqCst)
	}

	pub fn request_termination(&self) {
		if let Some(handle) = self.0.termination_sender.lock().ok().and_then(|mut lock| lock.take()) {
			handle.terminate()
		}
	}

	pub fn set_termination_handle<H: ImaginateTerminationHandle>(&self, handle: Box<H>) {
		if let Ok(mut lock) = self.0.termination_sender.lock() {
			*lock = Some(handle)
		}
	}
}

impl std::cmp::PartialEq for ImaginateController {
	fn eq(&self, other: &Self) -> bool {
		Arc::ptr_eq(&self.0, &other.0)
	}
}

impl core::hash::Hash for ImaginateController {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::ptr::hash(Arc::as_ptr(&self.0), state)
	}
}

#[derive(Default, Debug, Clone, PartialEq, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImaginateStatus {
	#[default]
	Ready,
	ReadyDone,
	Beginning,
	Uploading,
	Generating(f64),
	Terminating,
	Terminated,
	Failed(String),
}

impl ImaginateStatus {
	pub fn to_text(&self) -> Cow<'static, str> {
		match self {
			Self::Ready => Cow::Borrowed("Ready"),
			Self::ReadyDone => Cow::Borrowed("Done"),
			Self::Beginning => Cow::Borrowed("Beginning…"),
			Self::Uploading => Cow::Borrowed("Downloading Image…"),
			Self::Generating(percent) => Cow::Owned(format!("Generating {percent:.0}%")),
			Self::Terminating => Cow::Owned("Terminating…".to_string()),
			Self::Terminated => Cow::Owned("Terminated".to_string()),
			Self::Failed(err) => Cow::Owned(format!("Failed: {err}")),
		}
	}
}

#[allow(clippy::derived_hash_with_manual_eq)]
impl core::hash::Hash for ImaginateStatus {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::mem::discriminant(self).hash(state);
		match self {
			Self::Ready | Self::ReadyDone | Self::Beginning | Self::Uploading | Self::Terminating | Self::Terminated => (),
			Self::Generating(f) => f.to_bits().hash(state),
			Self::Failed(err) => err.hash(state),
		}
	}
}

#[derive(PartialEq, Eq, Clone, Default, Debug)]
pub enum ImaginateServerStatus {
	#[default]
	Unknown,
	Checking,
	Connected,
	Failed(String),
	Unavailable,
}

impl ImaginateServerStatus {
	pub fn to_text(&self) -> Cow<'static, str> {
		match self {
			Self::Unknown | Self::Checking => Cow::Borrowed("Checking..."),
			Self::Connected => Cow::Borrowed("Connected"),
			Self::Failed(err) => Cow::Owned(err.clone()),
			Self::Unavailable => Cow::Borrowed("Unavailable"),
		}
	}
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

#[derive(Clone, Debug, PartialEq, Hash, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
