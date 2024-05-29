use super::utility_types::TransformIn;
use super::utility_types::VectorDataModification;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;

use bezier_rs::Subpath;
use graph_craft::document::DocumentNode;
use graph_craft::document::NodeId;
use graph_craft::document::NodeInput;
use graphene_core::raster::BlendMode;
use graphene_core::raster::ImageFrame;
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
	AddNodesAsChild {
		nodes: HashMap<NodeId, DocumentNode>,
		new_ids: HashMap<NodeId, NodeId>,
		parent: LayerNodeIdentifier,
		insert_index: isize,
	},
	CreateBooleanOperationNode {
		node_id: NodeId,
		operation: BooleanOperation,
	},
	DeleteLayer {
		layer: LayerNodeIdentifier,
		reconnect: bool,
	},
	DisconnectInput {
		node_id: NodeId,
		input_index: usize,
	},
	DisconnectNodeFromStack {
		node_id: NodeId,
		reconnect_to_sibling: bool,
	},
	FillSet {
		layer: LayerNodeIdentifier,
		fill: Fill,
	},
	InsertNodeAtStackIndex {
		node_id: NodeId,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	InsertBooleanOperation {
		operation: BooleanOperation,
	},
	InsertNodeBetween {
		// Post node
		post_node_id: NodeId,
		post_node_input_index: usize,
		// Inserted node
		insert_node_id: NodeId,
		insert_node_output_index: usize,
		insert_node_input_index: usize,
		// Pre node
		pre_node_id: NodeId,
		pre_node_output_index: usize,
	},
	MoveSelectedSiblingsToChild {
		new_parent: LayerNodeIdentifier,
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
		alias: String,
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
	SetNodeInput {
		node_id: NodeId,
		input_index: usize,
		input: NodeInput,
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
