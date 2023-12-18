use crate::layers::layer_info::LegacyLayer;
use crate::LayerId;

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[repr(C)]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
/// Operations that can be performed to mutate the document.
pub enum Operation {
	AddFrame {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		network: graph_craft::document::NodeNetwork,
	},
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
	DuplicateLayer {
		path: Vec<LayerId>,
	},
	RenameLayer {
		layer_path: Vec<LayerId>,
		new_name: String,
	},
	InsertLayer {
		layer: Box<LegacyLayer>,
		destination_path: Vec<LayerId>,
		insert_index: isize,
		duplicating: bool,
	},
	SetSurface {
		path: Vec<LayerId>,
		surface_id: graphene_core::SurfaceId,
	},
	SetLayerScaleAroundPivot {
		path: Vec<LayerId>,
		new_scale: (f64, f64),
	},
	SetLayerTransform {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	SetLayerPreserveAspect {
		layer_path: Vec<LayerId>,
		preserve_aspect: bool,
	},
}

impl Operation {
	pub fn pseudo_hash(&self) -> u64 {
		let mut s = DefaultHasher::new();
		std::mem::discriminant(self).hash(&mut s);
		s.finish()
	}
}
