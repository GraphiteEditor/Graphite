//! Model manifest – the serialisable identity of a machine-learning model.
use serde::{Deserialize, Serialize};

/// The shape of a single model input or output tensor.
///
/// Each element is the size of that dimension; a value of `None` indicates a
/// dynamic (batch / variable-length) dimension.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TensorShape(pub Vec<Option<usize>>);

impl TensorShape {
	/// Convenience constructor from a fixed-size shape.
	pub fn fixed(dims: impl IntoIterator<Item = usize>) -> Self {
		Self(dims.into_iter().map(Some).collect())
	}

	/// Convenience constructor that marks the first dimension as dynamic
	/// (batch) and the rest as fixed.
	pub fn batched(dims: impl IntoIterator<Item = usize>) -> Self {
		let mut shape: Vec<Option<usize>> = dims.into_iter().map(Some).collect();
		if let Some(first) = shape.first_mut() {
			*first = None;
		}
		Self(shape)
	}
}

/// The open-source licence under which a model is distributed.
///
/// Only the three variants listed in [`License::is_permissive`] are considered
/// compatible with Graphite's licensing standards.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum License {
	/// MIT Licence
	Mit,
	/// BSD 2-Clause or 3-Clause licence
	Bsd,
	/// Apache Licence 2.0
	Apache2,
	/// Any other licence whose SPDX identifier is not explicitly recognised.
	Other(String),
}

impl License {
	/// Returns `true` only for licences that are permissive enough to be
	/// distributed alongside Graphite without additional restrictions.
	///
	/// Currently the permissive set is **MIT**, **BSD**, and **Apache-2.0**.
	pub fn is_permissive(&self) -> bool {
		matches!(self, License::Mit | License::Bsd | License::Apache2)
	}
}

impl std::fmt::Display for License {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			License::Mit => write!(f, "MIT"),
			License::Bsd => write!(f, "BSD"),
			License::Apache2 => write!(f, "Apache-2.0"),
			License::Other(id) => write!(f, "{id}"),
		}
	}
}

/// The complete identity and capability description of a machine-learning model.
///
/// A manifest can be serialised to / deserialised from JSON so that it can be
/// shipped alongside the model weights on the CDN.
///
/// # Example
/// ```rust
/// use ai_models::manifest::{License, ModelManifest, TensorShape};
///
/// let manifest = ModelManifest {
///     model_id: "sam2-base".to_string(),
///     version: "1.0.0".to_string(),
///     display_name: "SAM 2 (base)".to_string(),
///     description: "Segment Anything Model 2, base variant".to_string(),
///     license: License::Apache2,
///     input_shapes: vec![TensorShape::batched([3, 1024, 1024])],
///     output_shapes: vec![TensorShape::batched([1, 1024, 1024])],
///     download_url: "https://cdn.graphite.art/models/sam2-base/weights.bin".to_string(),
///     size_bytes: 358_000_000,
/// };
///
/// assert!(manifest.license.is_permissive());
/// assert_eq!(manifest.model_id, "sam2-base");
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelManifest {
	/// Short machine-readable identifier, e.g. `"sam2-base"`.
	pub model_id: String,

	/// Semantic version of the model weights, e.g. `"1.0.0"`.
	pub version: String,

	/// Human-readable name shown in the UI, e.g. `"SAM 2 (base)"`.
	pub display_name: String,

	/// Short description of what the model does.
	pub description: String,

	/// Licence under which the model weights are distributed.
	pub license: License,

	/// Expected shapes of the model's input tensors (one entry per input port).
	pub input_shapes: Vec<TensorShape>,

	/// Expected shapes of the model's output tensors (one entry per output port).
	pub output_shapes: Vec<TensorShape>,

	/// URL from which the model weights can be downloaded.
	pub download_url: String,

	/// Total download size in bytes.
	pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sam2_manifest() -> ModelManifest {
		ModelManifest {
			model_id: "sam2-base".to_string(),
			version: "1.0.0".to_string(),
			display_name: "SAM 2 (base)".to_string(),
			description: "Segment Anything Model 2, base variant".to_string(),
			license: License::Apache2,
			input_shapes: vec![TensorShape::batched([3, 1024, 1024])],
			output_shapes: vec![TensorShape::batched([1, 1024, 1024])],
			download_url: "https://cdn.graphite.art/models/sam2-base/weights.bin".to_string(),
			size_bytes: 358_000_000,
		}
	}

	#[test]
	fn apache2_is_permissive() {
		assert!(License::Apache2.is_permissive());
	}

	#[test]
	fn mit_is_permissive() {
		assert!(License::Mit.is_permissive());
	}

	#[test]
	fn bsd_is_permissive() {
		assert!(License::Bsd.is_permissive());
	}

	#[test]
	fn other_is_not_permissive() {
		assert!(!License::Other("GPL-3.0".to_string()).is_permissive());
	}

	#[test]
	fn manifest_roundtrip_json() {
		let manifest = sam2_manifest();
		let json = serde_json::to_string(&manifest).expect("serialise");
		let back: ModelManifest = serde_json::from_str(&json).expect("deserialise");
		assert_eq!(manifest, back);
	}

	#[test]
	fn tensor_shape_fixed() {
		let shape = TensorShape::fixed([3, 224, 224]);
		assert_eq!(shape.0, vec![Some(3), Some(224), Some(224)]);
	}

	#[test]
	fn tensor_shape_batched_first_dim_is_none() {
		let shape = TensorShape::batched([3, 1024, 1024]);
		assert_eq!(shape.0[0], None);
		assert_eq!(shape.0[1], Some(1024));
	}
}
