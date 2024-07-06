use super::utility_types::TransformIn;
use super::utility_types::VectorDataModification;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;
use crate::messages::prelude::*;

use bezier_rs::Subpath;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::text::Font;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::{Artboard, Color};
use graphene_std::vector::misc::BooleanOperation;

use glam::{DAffine2, DVec2, IVec2};

#[impl_message(Message, DocumentMessage, GraphOperation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum GraphOperationMessage {
	AddNodes {
		nodes: HashMap<NodeId, NodeTemplate>,
		new_ids: HashMap<NodeId, NodeId>,
	},
	// TODO: Eventually remove this (probably starting late 2024)
	DeleteLegacyOutputNode,
	FillSet {
		layer: LayerNodeIdentifier,
		fill: Fill,
	},
	InsertBooleanOperation {
		operation: BooleanOperation,
	},
	MoveLayerToStack {
		layer: LayerNodeIdentifier,
		parent: LayerNodeIdentifier,
		insert_index: usize,
		skip_rerender: bool,
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
		nodes: HashMap<NodeId, NodeTemplate>,
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
		layer: LayerNodeIdentifier,
		location: IVec2,
		dimensions: IVec2,
	},
	ClearArtboards,
	NewSvg {
		id: NodeId,
		svg: String,
		transform: DAffine2,
		parent: LayerNodeIdentifier,
		insert_index: isize,
	},
	ShiftUpstream {
		node_id: NodeId,
		shift: IVec2,
		shift_self: bool,
	},
	SetNodePosition {
		node_id: NodeId,
		position: IVec2,
	},
	SetName {
		layer: LayerNodeIdentifier,
		name: String,
	},
	SetNameImpl {
		layer: LayerNodeIdentifier,
		name: String,
	},
	ToggleSelectedVisibility,
	ToggleVisibility {
		node_id: NodeId,
	},
	SetVisibility {
		node_id: NodeId,
		visible: bool,
	},
	StartPreviewingWithoutRestore,
	ToggleSelectedLocked,
	ToggleLocked {
		node_id: NodeId,
	},
	SetLocked {
		node_id: NodeId,
		locked: bool,
	},
}
