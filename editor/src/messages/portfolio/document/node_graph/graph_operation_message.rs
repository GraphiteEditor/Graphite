use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;

use bezier_rs::Subpath;
use graph_craft::document::DocumentNode;
use graph_craft::document::NodeId;
use graphene_core::raster::BlendMode;
use graphene_core::raster::ImageFrame;
use graphene_core::text::Font;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::vector::ManipulatorPointId;
use graphene_core::{Artboard, Color};

use glam::{DAffine2, DVec2, IVec2};

#[impl_message(Message, DocumentMessage, GraphOperation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum GraphOperationMessage {
	FillSet {
		layer: LayerNodeIdentifier,
		fill: Fill,
	},
	OpacitySet {
		layer: LayerNodeIdentifier,
		opacity: f64,
	},
	BlendModeSet {
		layer: LayerNodeIdentifier,
		blend_mode: BlendMode,
	},
	UpdateBounds {
		layer: LayerNodeIdentifier,
		old_bounds: [DVec2; 2],
		new_bounds: [DVec2; 2],
	},
	StrokeSet {
		layer: LayerNodeIdentifier,
		stroke: Stroke,
	},

	TransformChange {
		layer: LayerNodeIdentifier,
		transform: DAffine2,
		transform_in: TransformIn,
		skip_rerender: bool,
	},
	TransformSet {
		layer: LayerNodeIdentifier,
		transform: DAffine2,
		transform_in: TransformIn,
		skip_rerender: bool,
	},
	TransformSetPivot {
		layer: LayerNodeIdentifier,
		pivot: DVec2,
	},

	Vector {
		layer: LayerNodeIdentifier,
		modification: VectorDataModification,
	},
	Brush {
		layer: LayerNodeIdentifier,
		strokes: Vec<BrushStroke>,
	},

	NewArtboard {
		id: NodeId,
		artboard: Artboard,
	},
	NewBitmapLayer {
		id: NodeId,
		image_frame: ImageFrame<Color>,
		parent: LayerNodeIdentifier,
		insert_index: isize,
	},
	NewCustomLayer {
		id: NodeId,
		nodes: HashMap<NodeId, DocumentNode>,
		parent: LayerNodeIdentifier,
		insert_index: isize,
	},
	NewVectorLayer {
		id: NodeId,
		subpaths: Vec<Subpath<ManipulatorGroupId>>,
		parent: LayerNodeIdentifier,
		insert_index: isize,
	},
	NewTextLayer {
		id: NodeId,
		text: String,
		font: Font,
		size: f64,
		parent: LayerNodeIdentifier,
		insert_index: isize,
	},
	ResizeArtboard {
		id: NodeId,
		location: IVec2,
		dimensions: IVec2,
	},
	DeleteLayer {
		id: NodeId,
	},
	ClearArtboards,
	NewSvg {
		id: NodeId,
		svg: String,
		transform: DAffine2,
		parent: LayerNodeIdentifier,
		insert_index: isize,
	},
}

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
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
	UpdateSubpaths { subpaths: Vec<Subpath<ManipulatorGroupId>> },
}
