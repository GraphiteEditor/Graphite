//! Model registry – the centralised service that tracks every model's lifecycle.
use std::collections::HashMap;

use thiserror::Error;

use crate::manifest::ModelManifest;

/// The lifecycle state of a single model.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
	/// The model weights are present on disk and have been verified.
	Ready,
	/// The model is listed in the registry but has not been downloaded yet.
	NotStarted,
	/// The model weights are currently being fetched from the CDN.
	Downloading {
		/// Download progress in the range `[0.0, 1.0]`.
		progress: f32,
	},
	/// A previous download or verification attempt failed.
	Failed {
		/// Human-readable reason for the failure.
		reason: String,
	},
}

impl PartialEq for ModelStatus {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(ModelStatus::Ready, ModelStatus::Ready) => true,
			(ModelStatus::NotStarted, ModelStatus::NotStarted) => true,
			(ModelStatus::Downloading { progress: a }, ModelStatus::Downloading { progress: b }) => a.to_bits() == b.to_bits(),
			(ModelStatus::Failed { reason: a }, ModelStatus::Failed { reason: b }) => a == b,
			_ => false,
		}
	}
}

impl std::fmt::Display for ModelStatus {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ModelStatus::Ready => write!(f, "Ready"),
			ModelStatus::NotStarted => write!(f, "Not Started"),
			ModelStatus::Downloading { progress } => write!(f, "Downloading ({:.0}%)", progress * 100.0),
			ModelStatus::Failed { reason } => write!(f, "Failed: {reason}"),
		}
	}
}

/// Errors that can be returned by [`ModelRegistry`] operations.
#[derive(Debug, Error, PartialEq)]
pub enum RegistryError {
	/// The model ID was not found in the registry.
	#[error("model '{0}' is not registered")]
	NotFound(String),

	/// The model's licence is not permissive enough for Graphite to use.
	#[error("model '{id}' uses a non-permissive licence ({license}); only MIT, BSD, and Apache-2.0 are accepted")]
	LicenceNotPermissive {
		/// The model identifier.
		id: String,
		/// The non-permissive licence that was rejected.
		license: String,
	},

	/// Attempted to register a model whose ID already exists in the registry.
	#[error("model '{0}' is already registered")]
	AlreadyRegistered(String),
}

/// Entry stored for each model inside the registry.
#[derive(Clone, Debug)]
struct RegistryEntry {
	manifest: ModelManifest,
	status: ModelStatus,
}

/// The central manager that keeps track of which models are available and their
/// current lifecycle status.
///
/// # Usage
/// ```rust
/// use ai_models::manifest::{License, ModelManifest, TensorShape};
/// use ai_models::registry::{ModelRegistry, ModelStatus};
///
/// let mut registry = ModelRegistry::new();
///
/// let manifest = ModelManifest {
///     model_id: "sam2-base".to_string(),
///     version: "1.0.0".to_string(),
///     display_name: "SAM 2 (base)".to_string(),
///     description: "Segment Anything Model 2".to_string(),
///     license: License::Apache2,
///     input_shapes: vec![TensorShape::batched([3, 1024, 1024])],
///     output_shapes: vec![TensorShape::batched([1, 1024, 1024])],
///     download_url: "https://cdn.graphite.art/models/sam2-base/weights.bin".to_string(),
///     size_bytes: 358_000_000,
/// };
///
/// registry.register(manifest).expect("register model");
/// assert_eq!(registry.status("sam2-base").unwrap(), &ModelStatus::NotStarted);
/// ```
#[derive(Debug, Default)]
pub struct ModelRegistry {
	entries: HashMap<String, RegistryEntry>,
}

impl ModelRegistry {
	/// Creates a new, empty registry.
	pub fn new() -> Self {
		Self::default()
	}

	/// Registers a new model manifest.
	///
	/// # Errors
	/// * [`RegistryError::LicenceNotPermissive`] – the manifest's licence is not MIT, BSD, or Apache-2.0.
	/// * [`RegistryError::AlreadyRegistered`] – a model with the same `model_id` already exists.
	pub fn register(&mut self, manifest: ModelManifest) -> Result<(), RegistryError> {
		if !manifest.license.is_permissive() {
			return Err(RegistryError::LicenceNotPermissive {
				id: manifest.model_id.clone(),
				license: manifest.license.to_string(),
			});
		}

		if self.entries.contains_key(&manifest.model_id) {
			return Err(RegistryError::AlreadyRegistered(manifest.model_id));
		}

		log::info!("Registering model '{}' v{}", manifest.model_id, manifest.version);

		self.entries.insert(
			manifest.model_id.clone(),
			RegistryEntry {
				manifest,
				status: ModelStatus::NotStarted,
			},
		);
		Ok(())
	}

	/// Returns the current [`ModelStatus`] for `model_id`.
	///
	/// # Errors
	/// [`RegistryError::NotFound`] if the model is not registered.
	pub fn status(&self, model_id: &str) -> Result<&ModelStatus, RegistryError> {
		self.entries
			.get(model_id)
			.map(|e| &e.status)
			.ok_or_else(|| RegistryError::NotFound(model_id.to_string()))
	}

	/// Updates the status of a registered model.
	///
	/// # Errors
	/// [`RegistryError::NotFound`] if the model is not registered.
	pub fn set_status(&mut self, model_id: &str, status: ModelStatus) -> Result<(), RegistryError> {
		self.entries
			.get_mut(model_id)
			.map(|e| {
				log::debug!("Model '{}' status → {status}", model_id);
				e.status = status;
			})
			.ok_or_else(|| RegistryError::NotFound(model_id.to_string()))
	}

	/// Returns the [`ModelManifest`] for `model_id`.
	///
	/// # Errors
	/// [`RegistryError::NotFound`] if the model is not registered.
	pub fn manifest(&self, model_id: &str) -> Result<&ModelManifest, RegistryError> {
		self.entries
			.get(model_id)
			.map(|e| &e.manifest)
			.ok_or_else(|| RegistryError::NotFound(model_id.to_string()))
	}

	/// Returns `true` if the model is registered **and** its status is [`ModelStatus::Ready`].
	pub fn is_ready(&self, model_id: &str) -> bool {
		self.entries.get(model_id).is_some_and(|e| matches!(e.status, ModelStatus::Ready))
	}

	/// Returns an iterator over all registered model IDs.
	pub fn model_ids(&self) -> impl Iterator<Item = &str> {
		self.entries.keys().map(String::as_str)
	}

	/// Returns a list of (model_id, status) pairs for every registered model.
	pub fn all_statuses(&self) -> Vec<(&str, &ModelStatus)> {
		let mut pairs: Vec<(&str, &ModelStatus)> = self.entries.iter().map(|(id, e)| (id.as_str(), &e.status)).collect();
		pairs.sort_by_key(|(id, _)| *id);
		pairs
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::manifest::{License, TensorShape};

	fn make_manifest(id: &str, license: License) -> ModelManifest {
		ModelManifest {
			model_id: id.to_string(),
			version: "1.0.0".to_string(),
			display_name: id.to_string(),
			description: String::new(),
			license,
			input_shapes: vec![TensorShape::fixed([3, 224, 224])],
			output_shapes: vec![TensorShape::fixed([1000])],
			download_url: format!("https://cdn.example.com/{id}/weights.bin"),
			size_bytes: 1_000_000,
		}
	}

	#[test]
	fn register_and_default_status_is_not_started() {
		let mut registry = ModelRegistry::new();
		registry.register(make_manifest("model-a", License::Mit)).unwrap();
		assert_eq!(registry.status("model-a").unwrap(), &ModelStatus::NotStarted);
	}

	#[test]
	fn register_non_permissive_licence_is_blocked() {
		let mut registry = ModelRegistry::new();
		let err = registry.register(make_manifest("bad-model", License::Other("GPL-3.0".to_string()))).unwrap_err();
		assert!(matches!(err, RegistryError::LicenceNotPermissive { .. }));
	}

	#[test]
	fn duplicate_registration_returns_error() {
		let mut registry = ModelRegistry::new();
		registry.register(make_manifest("model-b", License::Bsd)).unwrap();
		let err = registry.register(make_manifest("model-b", License::Bsd)).unwrap_err();
		assert_eq!(err, RegistryError::AlreadyRegistered("model-b".to_string()));
	}

	#[test]
	fn set_status_ready() {
		let mut registry = ModelRegistry::new();
		registry.register(make_manifest("model-c", License::Apache2)).unwrap();
		registry.set_status("model-c", ModelStatus::Ready).unwrap();
		assert!(registry.is_ready("model-c"));
	}

	#[test]
	fn set_status_downloading() {
		let mut registry = ModelRegistry::new();
		registry.register(make_manifest("model-d", License::Mit)).unwrap();
		registry.set_status("model-d", ModelStatus::Downloading { progress: 0.42 }).unwrap();
		assert!(!registry.is_ready("model-d"));
		assert_eq!(registry.status("model-d").unwrap(), &ModelStatus::Downloading { progress: 0.42 });
	}

	#[test]
	fn set_status_failed() {
		let mut registry = ModelRegistry::new();
		registry.register(make_manifest("model-e", License::Apache2)).unwrap();
		registry.set_status("model-e", ModelStatus::Failed { reason: "network error".to_string() }).unwrap();
		assert!(!registry.is_ready("model-e"));
	}

	#[test]
	fn status_of_unknown_model_returns_not_found() {
		let registry = ModelRegistry::new();
		assert_eq!(registry.status("ghost"), Err(RegistryError::NotFound("ghost".to_string())));
	}

	#[test]
	fn is_ready_returns_false_for_unknown_model() {
		let registry = ModelRegistry::new();
		assert!(!registry.is_ready("ghost"));
	}

	#[test]
	fn manifest_retrieval() {
		let mut registry = ModelRegistry::new();
		let m = make_manifest("model-f", License::Mit);
		registry.register(m.clone()).unwrap();
		assert_eq!(registry.manifest("model-f").unwrap().model_id, "model-f");
	}

	#[test]
	fn all_statuses_is_sorted() {
		let mut registry = ModelRegistry::new();
		registry.register(make_manifest("zzz-model", License::Mit)).unwrap();
		registry.register(make_manifest("aaa-model", License::Bsd)).unwrap();
		let statuses = registry.all_statuses();
		assert_eq!(statuses[0].0, "aaa-model");
		assert_eq!(statuses[1].0, "zzz-model");
	}

	#[test]
	fn model_status_display() {
		assert_eq!(ModelStatus::Ready.to_string(), "Ready");
		assert_eq!(ModelStatus::NotStarted.to_string(), "Not Started");
		assert_eq!(ModelStatus::Downloading { progress: 0.5 }.to_string(), "Downloading (50%)");
		assert_eq!(ModelStatus::Failed { reason: "err".to_string() }.to_string(), "Failed: err");
	}
}
Cargo.lock
Cargo.toml
libraries/ai-models
Cargo.toml
src
lib.rs
manifest.rs
registry.rs

