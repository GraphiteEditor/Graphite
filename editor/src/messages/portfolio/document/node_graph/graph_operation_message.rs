use crate::messages::prelude::*;

use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::vector::{consts::ManipulatorType, manipulator_group::ManipulatorGroup};

use glam::{DAffine2, DVec2};

pub type LayerIdentifier = Vec<document_legacy::LayerId>;

#[impl_message(Message, DocumentMessage, GraphOperation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum GraphOperationMessage {
	FillSet { layer: LayerIdentifier, fill: Fill },

	StrokeSet { layer: LayerIdentifier, stroke: Stroke },

	TransformChange { layer: LayerIdentifier, transform: DAffine2, transform_in: TransformIn },
	TransformSet { layer: LayerIdentifier, transform: DAffine2, transform_in: TransformIn },
	TransformSetPivot { layer: LayerIdentifier, pivot: DVec2 },

	Vector { layer: LayerIdentifier, modification: VectorDataModification },
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum TransformIn {
	Local,
	Scope { scope: DAffine2 },
	Viewport,
}

/// TODO: State like mirroring needs to be stored in tools after bézier_rs migration
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorDataModification {
	DeleteSelectedManipulatorPoints { layer_paths: Vec<LayerIdentifier> },
	DeselectAllManipulatorPoints,
	DeselectManipulatorPoints { point_ids: Vec<(u64, ManipulatorType)> },
	InsertManipulatorGroup { manipulator_group: ManipulatorGroup, after_id: u64 },
	MoveManipulatorPoint { id: u64, manipulator_type: ManipulatorType, position: DVec2 },
	MoveSelectedManipulatorPoints { delta: DVec2, mirror_distance: bool },
	PushFrontManipulatorGroup { manipulator_group: ManipulatorGroup },
	PushManipulatorGroup { manipulator_group: ManipulatorGroup },
	RemoveManipulatorGroup { id: u64 },
	RemoveManipulatorPoint { id: u64, manipulator_type: ManipulatorType },
	SelectAllAnchors,
	SelectManipulatorPoints { point_ids: Vec<(u64, ManipulatorType)>, add: bool },
	SetManipulatorHandleMirroring { id: u64, mirror_angle: bool },
	SetManipulatorPoints { id: u64, manipulator_type: ManipulatorType, position: Option<DVec2> },
	SetSelectedHandleMirroring { toggle_angle: bool },
}
