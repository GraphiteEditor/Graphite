use crate::boolean_ops::BooleanOperation as BooleanOperationType;
use crate::layers::blend_mode::BlendMode;
use crate::layers::layer_info::Layer;
use crate::layers::style::{self, Stroke};
use crate::LayerId;

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[repr(C)]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
// TODO: Rename all instances of `path` to `layer_path`
/// Operations that can be performed to mutate the document.
pub enum Operation {
	AddEllipse {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddOverlayEllipse {
		path: Vec<LayerId>,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddRect {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddOverlayRect {
		path: Vec<LayerId>,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddLine {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddOverlayLine {
		path: Vec<LayerId>,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddText {
		path: Vec<LayerId>,
		transform: [f64; 6],
		insert_index: isize,
		text: String,
		style: style::PathStyle,
		size: f64,
		font_name: String,
		font_style: String,
		font_file: Option<String>,
	},
	AddImage {
		path: Vec<LayerId>,
		transform: [f64; 6],
		insert_index: isize,
		mime: String,
		image_data: Vec<u8>,
	},
	SetImageBlobUrl {
		path: Vec<LayerId>,
		blob_url: String,
		dimensions: (f64, f64),
	},
	SetTextEditability {
		path: Vec<LayerId>,
		editable: bool,
	},
	SetTextContent {
		path: Vec<LayerId>,
		new_text: String,
	},
	AddPolyline {
		path: Vec<LayerId>,
		transform: [f64; 6],
		insert_index: isize,
		points: Vec<(f64, f64)>,
		style: style::PathStyle,
	},
	AddSpline {
		path: Vec<LayerId>,
		transform: [f64; 6],
		insert_index: isize,
		points: Vec<(f64, f64)>,
		style: style::PathStyle,
	},
	AddNgon {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		sides: u8,
		style: style::PathStyle,
	},
	AddOverlayShape {
		path: Vec<LayerId>,
		bez_path: kurbo::BezPath,
		style: style::PathStyle,
		closed: bool,
	},
	AddShape {
		path: Vec<LayerId>,
		transform: [f64; 6],
		insert_index: isize,
		bez_path: kurbo::BezPath,
		style: style::PathStyle,
		closed: bool,
	},
	BooleanOperation {
		operation: BooleanOperationType,
		selected: Vec<Vec<LayerId>>,
	},
	DeleteLayer {
		path: Vec<LayerId>,
	},
	DuplicateLayer {
		path: Vec<LayerId>,
	},
	ModifyFont {
		path: Vec<LayerId>,
		font_family: String,
		font_style: String,
		font_file: Option<String>,
		size: f64,
	},
	RenameLayer {
		layer_path: Vec<LayerId>,
		new_name: String,
	},
	InsertLayer {
		layer: Layer,
		destination_path: Vec<LayerId>,
		insert_index: isize,
	},
	CreateFolder {
		path: Vec<LayerId>,
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
		bez_path: kurbo::BezPath,
	},
	SetShapePathInViewport {
		path: Vec<LayerId>,
		bez_path: kurbo::BezPath,
		transform: [f64; 6],
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
	SetLayerTransform {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	ToggleLayerVisibility {
		path: Vec<LayerId>,
	},
	SetLayerVisibility {
		path: Vec<LayerId>,
		visible: bool,
	},
	SetLayerName {
		path: Vec<LayerId>,
		name: String,
	},
	SetLayerBlendMode {
		path: Vec<LayerId>,
		blend_mode: BlendMode,
	},
	SetLayerOpacity {
		path: Vec<LayerId>,
		opacity: f64,
	},
	SetLayerStyle {
		path: Vec<LayerId>,
		style: style::PathStyle,
	},
	SetLayerFill {
		path: Vec<LayerId>,
		fill: style::Fill,
	},
	SetLayerStroke {
		path: Vec<LayerId>,
		stroke: Stroke,
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
