use crate::messages::prelude::*;

use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::vector::ManipulatorPointId;

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

type ManipulatorGroup = bezier_rs::ManipulatorGroup<ManipulatorGroupId>;

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorDataModification {
	AddEndManipulatorGroup { subpath_index: usize, manipulator_group: ManipulatorGroup },
	AddManipulatorGroup { manipulator_group: ManipulatorGroup, after_id: ManipulatorGroupId },
	AddStartManipulatorGroup { subpath_index: usize, manipulator_group: ManipulatorGroup },
	RemoveManipulatorGroup { id: ManipulatorGroupId },
	RemoveManipulatorPoint { point: ManipulatorPointId },
	SetClosed { index: usize, closed: bool },
	SetManipulatorHandleMirroring { id: ManipulatorGroupId, mirror_angle: bool },
	SetManipulatorPosition { point: ManipulatorPointId, position: DVec2 },
	ToggleManipulatorHandleMirroring { id: ManipulatorGroupId },
}
