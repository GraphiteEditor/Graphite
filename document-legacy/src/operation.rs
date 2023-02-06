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
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, specta::Type)]
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

#[allow(clippy::derive_hash_xor_eq)]
impl Hash for Operation {
	fn hash<H: Hasher>(&self, state: &mut H) {
		use Operation::*;
		match self {
			AddEllipse { path, insert_index, transform, style } => {
				0.hash(state);
				path.hash(state);
				insert_index.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				style.hash(state);
			}
			AddRect { path, insert_index, transform, style } => {
				1.hash(state);
				path.hash(state);
				insert_index.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				style.hash(state);
			}
			AddLine { path, insert_index, transform, style } => {
				2.hash(state);
				path.hash(state);
				insert_index.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				style.hash(state);
			}
			AddText {
				path,
				insert_index,
				transform,
				style,
				text,
				size,
				font_name,
				font_style,
			} => {
				3.hash(state);
				path.hash(state);
				insert_index.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				style.hash(state);
				text.hash(state);
				size.to_bits().hash(state);
				font_name.hash(state);
				font_style.hash(state);
			}
			AddNodeGraphFrame {
				path,
				insert_index,
				transform,
				network,
			} => {
				4.hash(state);
				path.hash(state);
				insert_index.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				network.hash(state);
			}
			SetNodeGraphFrameImageData { layer_path, image_data } => {
				5.hash(state);
				layer_path.hash(state);
				image_data.hash(state);
			}
			SetLayerBlobUrl { layer_path, blob_url, resolution } => {
				6.hash(state);
				layer_path.hash(state);
				blob_url.hash(state);
				resolution.0.to_bits().hash(state);
				resolution.1.to_bits().hash(state);
			}
			ClearBlobURL { path } => {
				7.hash(state);
				path.hash(state);
			}
			SetPivot { layer_path, pivot } => {
				8.hash(state);
				layer_path.hash(state);
				[pivot.0, pivot.1].iter().for_each(|x| x.to_bits().hash(state));
			}
			SetTextEditability { path, editable } => {
				9.hash(state);
				path.hash(state);
				editable.hash(state);
			}
			SetTextContent { path, new_text } => {
				10.hash(state);
				path.hash(state);
				new_text.hash(state);
			}
			AddPolyline {
				path,
				insert_index,
				transform,
				style,
				points,
			} => {
				11.hash(state);
				path.hash(state);
				insert_index.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				style.hash(state);
				points.iter().flat_map(|x| [x.0, x.1]).for_each(|x| x.to_bits().hash(state));
			}
			AddSpline {
				path,
				insert_index,
				transform,
				style,
				points,
			} => {
				12.hash(state);
				path.hash(state);
				insert_index.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				style.hash(state);
				points.iter().flat_map(|x| [x.0, x.1]).for_each(|x| x.to_bits().hash(state));
			}
			AddNgon {
				path,
				insert_index,
				transform,
				style,
				sides,
			} => {
				13.hash(state);
				path.hash(state);
				insert_index.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				style.hash(state);
				sides.hash(state);
			}
			AddShape {
				path,
				insert_index,
				transform,
				style,
				subpath,
			} => {
				14.hash(state);
				path.hash(state);
				insert_index.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				style.hash(state);
				subpath.hash(state);
			}
			BooleanOperation { operation, selected } => {
				15.hash(state);
				operation.hash(state);
				selected.hash(state);
			}
			DeleteLayer { path } => {
				16.hash(state);
				path.hash(state);
			}
			DeleteSelectedManipulatorPoints { layer_paths } => {
				17.hash(state);
				layer_paths.hash(state);
			}
			DeselectManipulatorPoints { layer_path, point_ids } => {
				18.hash(state);
				layer_path.hash(state);
				point_ids.hash(state);
			}
			DeselectAllManipulatorPoints { layer_path } => {
				19.hash(state);
				layer_path.hash(state);
			}
			DuplicateLayer { path } => {
				20.hash(state);
				path.hash(state);
			}
			ModifyFont { path, font_family, size, font_style } => {
				21.hash(state);
				path.hash(state);
				font_family.hash(state);
				size.to_bits().hash(state);
				font_style.hash(state);
			}
			MoveSelectedManipulatorPoints { layer_path, delta, mirror_distance } => {
				22.hash(state);
				layer_path.hash(state);
				[delta.0, delta.1].iter().for_each(|x| x.to_bits().hash(state));
				mirror_distance.hash(state);
			}
			MoveManipulatorPoint {
				layer_path,
				id,
				manipulator_type,
				position,
			} => {
				23.hash(state);
				layer_path.hash(state);
				id.hash(state);
				manipulator_type.hash(state);
				[position.0, position.1].iter().for_each(|x| x.to_bits().hash(state));
			}
			SetManipulatorPoints {
				layer_path,
				id,
				manipulator_type,
				position,
			} => {
				24.hash(state);
				layer_path.hash(state);
				id.hash(state);
				manipulator_type.hash(state);
				position.map(|x| [x.0, x.1].iter().for_each(|x| x.to_bits().hash(state)));
				position.is_none().hash(state);
			}
			RenameLayer { layer_path, new_name } => {
				25.hash(state);
				layer_path.hash(state);
				new_name.hash(state);
			}
			InsertLayer {
				layer,
				destination_path,
				insert_index,
			} => {
				26.hash(state);
				layer.hash(state);
				destination_path.hash(state);
				insert_index.hash(state);
			}
			CreateFolder { path } => {
				27.hash(state);
				path.hash(state);
			}
			TransformLayer { path, transform } => {
				28.hash(state);
				path.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
			}
			TransformLayerInViewport { path, transform } => {
				29.hash(state);
				path.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
			}
			SetLayerTransformInViewport { path, transform } => {
				30.hash(state);
				path.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
			}
			SelectManipulatorPoints { layer_path, point_ids, add } => {
				31.hash(state);
				layer_path.hash(state);
				point_ids.hash(state);
				add.hash(state);
			}
			SetShapePath { path, subpath } => {
				32.hash(state);
				path.hash(state);
				subpath.hash(state);
			}
			InsertManipulatorGroup {
				layer_path,
				manipulator_group,
				after_id,
			} => {
				33.hash(state);
				layer_path.hash(state);
				manipulator_group.hash(state);
				after_id.hash(state);
			}
			PushManipulatorGroup { layer_path, manipulator_group } => {
				34.hash(state);
				layer_path.hash(state);
				manipulator_group.hash(state);
			}
			PushFrontManipulatorGroup { layer_path, manipulator_group } => {
				35.hash(state);
				layer_path.hash(state);
				manipulator_group.hash(state);
			}
			RemoveManipulatorGroup { layer_path, id } => {
				36.hash(state);
				layer_path.hash(state);
				id.hash(state);
			}
			RemoveManipulatorPoint { layer_path, id, manipulator_type } => {
				37.hash(state);
				layer_path.hash(state);
				id.hash(state);
				manipulator_type.hash(state);
			}
			TransformLayerInScope { path, transform, scope } => {
				38.hash(state);
				path.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				scope.iter().for_each(|x| x.to_bits().hash(state));
			}
			SetLayerTransformInScope { path, transform, scope } => {
				39.hash(state);
				path.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
				scope.iter().for_each(|x| x.to_bits().hash(state));
			}
			TransformLayerScaleAroundPivot { path, scale_factor } => {
				40.hash(state);
				path.hash(state);
				let (x, y) = scale_factor;
				[x, y].iter().for_each(|x| x.to_bits().hash(state));
			}
			SetLayerScaleAroundPivot { path, new_scale } => {
				41.hash(state);
				path.hash(state);
				let (x, y) = new_scale;
				[x, y].iter().for_each(|x| x.to_bits().hash(state));
			}
			SetLayerTransform { path, transform } => {
				42.hash(state);
				path.hash(state);
				transform.iter().for_each(|x| x.to_bits().hash(state));
			}
			ToggleLayerVisibility { path } => {
				43.hash(state);
				path.hash(state);
			}
			SetLayerVisibility { path, visible } => {
				44.hash(state);
				path.hash(state);
				visible.hash(state);
			}
			SetLayerName { path, name } => {
				45.hash(state);
				path.hash(state);
				name.hash(state);
			}
			SetLayerPreserveAspect { layer_path, preserve_aspect } => {
				46.hash(state);
				layer_path.hash(state);
				preserve_aspect.hash(state);
			}
			SetLayerBlendMode { path, blend_mode } => {
				47.hash(state);
				path.hash(state);
				blend_mode.hash(state);
			}
			SetLayerOpacity { path, opacity } => {
				48.hash(state);
				path.hash(state);
				opacity.to_bits().hash(state);
			}
			SetLayerStyle { path, style } => {
				49.hash(state);
				path.hash(state);
				style.hash(state);
			}
			SetLayerFill { path, fill } => {
				50.hash(state);
				path.hash(state);
				fill.hash(state);
			}
			SetLayerStroke { path, stroke } => {
				51.hash(state);
				path.hash(state);
				stroke.hash(state);
			}
			SetManipulatorHandleMirroring { layer_path, id, mirror_angle } => {
				52.hash(state);
				layer_path.hash(state);
				id.hash(state);
				mirror_angle.hash(state);
			}
			SetSelectedHandleMirroring { layer_path, toggle_angle } => {
				53.hash(state);
				layer_path.hash(state);
				toggle_angle.hash(state);
			}
		};
	}
}
