use crate::LayerId;

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[repr(C)]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
/// Operations that can be performed to mutate the legacy document (soon to be fully removed).
pub enum Operation {
	/// Sets a blob URL as the image source for an Image or Imaginate layer type.
	/// **Be sure to call `FrontendMessage::TriggerRevokeBlobUrl` together with this.**
	SetLayerBlobUrl {
		layer_path: Vec<LayerId>,
		blob_url: String,
		resolution: (f64, f64),
	},
	/// Clears the image to leave the layer un-rendered.
	/// **Be sure to call `FrontendMessage::TriggerRevokeBlobUrl` together with this.**
	ClearBlobURL {
		path: Vec<LayerId>,
	},
	AddFrame {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		network: graph_craft::document::NodeNetwork,
	},
	SetSurface {
		path: Vec<LayerId>,
		surface_id: graphene_core::SurfaceId,
	},
}

impl Operation {
	pub fn pseudo_hash(&self) -> u64 {
		let mut s = DefaultHasher::new();
		std::mem::discriminant(self).hash(&mut s);
		s.finish()
	}
}
