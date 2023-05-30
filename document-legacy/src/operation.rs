use crate::layers::blend_mode::BlendMode;
use crate::layers::layer_info::Layer;
use crate::layers::style::{self, Stroke};
use crate::LayerId;

use graphene_std::vector::subpath::Subpath;

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[repr(C)]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
// TODO: Rename all instances of `path` to `layer_path`
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
	SetPivot {
		layer_path: Vec<LayerId>,
		pivot: (f64, f64),
	},
	DeleteLayer {
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
		layer: Box<Layer>,
		destination_path: Vec<LayerId>,
		insert_index: isize,
		duplicating: bool,
	},
	CreateFolder {
		path: Vec<LayerId>,
		insert_index: isize,
	},
	TransformLayer {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	TransformLayerInViewport {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	SetLayerTransformInViewport {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	SetShapePath {
		path: Vec<LayerId>,
		subpath: Subpath,
	},
	SetVectorData {
		path: Vec<LayerId>,
		vector_data: graphene_core::vector::VectorData,
	},
	SetSurface {
		path: Vec<LayerId>,
		surface_id: graphene_core::SurfaceId,
	},
	TransformLayerInScope {
		path: Vec<LayerId>,
		transform: [f64; 6],
		scope: [f64; 6],
	},
	SetLayerTransformInScope {
		path: Vec<LayerId>,
		transform: [f64; 6],
		scope: [f64; 6],
	},
	SetLayerScaleAroundPivot {
		path: Vec<LayerId>,
		new_scale: (f64, f64),
	},
	SetLayerTransform {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	SetLayerVisibility {
		path: Vec<LayerId>,
		visible: bool,
	},
	SetLayerName {
		path: Vec<LayerId>,
		name: String,
	},
	SetLayerPreserveAspect {
		layer_path: Vec<LayerId>,
		preserve_aspect: bool,
	},
	SetLayerBlendMode {
		path: Vec<LayerId>,
		blend_mode: BlendMode,
	},
	SetLayerOpacity {
		path: Vec<LayerId>,
		opacity: f64,
	},
	SetLayerFill {
		path: Vec<LayerId>,
		fill: style::Fill,
	},
	SetLayerStroke {
		path: Vec<LayerId>,
		stroke: Stroke,
	},

	// The following are used only by the legacy overlays system
	AddEllipse {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddRect {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddLine {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddPolyline {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
		points: Vec<(f64, f64)>,
	},
	AddShape {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
		subpath: Subpath,
	},
	SetLayerStyle {
		path: Vec<LayerId>,
		style: style::PathStyle,
	},
}

impl Operation {
	/// Returns the byte representation of the message.
	///
	/// # Safety
	/// This function reads from uninitialized memory!!!
	/// Only use if you know what you are doing
	unsafe fn as_slice(&self) -> &[u8] {
		core::slice::from_raw_parts(self as *const Operation as *const u8, std::mem::size_of::<Operation>())
	}
	/// Returns a pseudo hash that should uniquely identify the operation.
	/// This is needed because `Hash` is not implemented for f64s
	///
	/// # Safety
	/// This function reads from uninitialized memory but the generated value should be fine.
	pub fn pseudo_hash(&self) -> u64 {
		let mut s = DefaultHasher::new();
		unsafe { self.as_slice() }.hash(&mut s);
		s.finish()
	}
}
