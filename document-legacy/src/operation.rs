use crate::boolean_ops::BooleanOperation as BooleanOperationType;
use crate::layers::blend_mode::BlendMode;
use crate::layers::layer_info::Layer;
use crate::layers::style::{self, Stroke};
use crate::LayerId;

use graphene_std::vector::consts::ManipulatorType;
use graphene_std::vector::manipulator_group::ManipulatorGroup;
use graphene_std::vector::subpath::Subpath;

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
	AddText {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
		text: String,
		size: f64,
		font_name: String,
		font_style: String,
	},
	AddNodeGraphFrame {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		network: graph_craft::document::NodeNetwork,
	},
	SetNodeGraphFrameImageData {
		layer_path: Vec<LayerId>,
		image_data: Vec<u8>,
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
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
		points: Vec<(f64, f64)>,
	},
	AddSpline {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
		points: Vec<(f64, f64)>,
	},
	AddNgon {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
		sides: u32,
	},
	AddShape {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
		// TODO This will become a compound path once we support them.
		subpath: Subpath,
	},
	BooleanOperation {
		operation: BooleanOperationType,
		selected: Vec<Vec<LayerId>>,
	},
	DeleteLayer {
		path: Vec<LayerId>,
	},
	DeleteSelectedManipulatorPoints {
		layer_paths: Vec<Vec<LayerId>>,
	},
	DeselectManipulatorPoints {
		layer_path: Vec<LayerId>,
		point_ids: Vec<(u64, ManipulatorType)>,
	},
	DeselectAllManipulatorPoints {
		layer_path: Vec<LayerId>,
	},
	SelectAllAnchors {
		layer_path: Vec<LayerId>,
	},
	DuplicateLayer {
		path: Vec<LayerId>,
	},
	ModifyFont {
		path: Vec<LayerId>,
		font_family: String,
		size: f64,
		font_style: String,
	},
	MoveSelectedManipulatorPoints {
		layer_path: Vec<LayerId>,
		delta: (f64, f64),
		mirror_distance: bool,
	},
	MoveManipulatorPoint {
		layer_path: Vec<LayerId>,
		id: u64,
		manipulator_type: ManipulatorType,
		position: (f64, f64),
	},
	SetManipulatorPoints {
		layer_path: Vec<LayerId>,
		id: u64,
		manipulator_type: ManipulatorType,
		position: Option<(f64, f64)>,
	},
	RenameLayer {
		layer_path: Vec<LayerId>,
		new_name: String,
	},
	InsertLayer {
		layer: Box<Layer>,
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
	SelectManipulatorPoints {
		layer_path: Vec<LayerId>,
		point_ids: Vec<(u64, ManipulatorType)>,
		add: bool,
	},
	SetShapePath {
		path: Vec<LayerId>,
		subpath: Subpath,
	},
	InsertManipulatorGroup {
		layer_path: Vec<LayerId>,
		manipulator_group: ManipulatorGroup,
		after_id: u64,
	},
	PushManipulatorGroup {
		layer_path: Vec<LayerId>,
		manipulator_group: ManipulatorGroup,
	},
	PushFrontManipulatorGroup {
		layer_path: Vec<LayerId>,
		manipulator_group: ManipulatorGroup,
	},
	RemoveManipulatorGroup {
		layer_path: Vec<LayerId>,
		id: u64,
	},
	RemoveManipulatorPoint {
		layer_path: Vec<LayerId>,
		id: u64,
		manipulator_type: ManipulatorType,
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
	TransformLayerScaleAroundPivot {
		path: Vec<LayerId>,
		scale_factor: (f64, f64),
	},
	SetLayerScaleAroundPivot {
		path: Vec<LayerId>,
		new_scale: (f64, f64),
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
	SetManipulatorHandleMirroring {
		layer_path: Vec<LayerId>,
		id: u64,
		mirror_angle: bool,
	},
	SetSelectedHandleMirroring {
		layer_path: Vec<LayerId>,
		toggle_angle: bool,
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
