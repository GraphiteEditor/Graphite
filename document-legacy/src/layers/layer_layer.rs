use graphene_core::vector::VectorData;
use graphene_core::SurfaceId;
use serde::{Deserialize, Serialize};

// ================
// CachedOutputData
// ================

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum CachedOutputData {
	#[default]
	None,
	BlobURL(String),
	VectorPath(Box<VectorData>),
	SurfaceId(SurfaceId),
	Svg(String),
}

// ================
// LayerLegacyLayer
// ================

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct LayerLegacyLayer {
	/// The document node network that this layer contains
	pub network: graph_craft::document::NodeNetwork,

	#[serde(skip)]
	pub cached_output_data: CachedOutputData,
}

impl LayerLegacyLayer {
	pub fn as_blob_url(&self) -> Option<&String> {
		if let CachedOutputData::BlobURL(blob_url) = &self.cached_output_data {
			Some(blob_url)
		} else {
			None
		}
	}
}
