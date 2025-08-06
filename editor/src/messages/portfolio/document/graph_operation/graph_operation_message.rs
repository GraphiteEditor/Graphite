use super::utility_types::TransformIn;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;
use crate::messages::prelude::*;
use bezier_rs::Subpath;
use glam::{DAffine2, IVec2};
use graph_craft::document::NodeId;
use graphene_std::Artboard;
use graphene_std::brush::brush_stroke::BrushStroke;
use graphene_std::raster::BlendMode;
use graphene_std::raster_types::{CPU, Raster};
use graphene_std::table::Table;
use graphene_std::text::{Font, TypesettingConfig};
use graphene_std::vector::PointId;
use graphene_std::vector::VectorModificationType;
use graphene_std::vector::style::{Fill, Stroke};

#[impl_message(Message, DocumentMessage, GraphOperation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum GraphOperationMessage {
	FillSet {
		layer: LayerNodeIdentifier,
		fill: Fill,
	},
	BlendingFillSet {
		layer: LayerNodeIdentifier,
		fill: f64,
	},
	OpacitySet {
		layer: LayerNodeIdentifier,
		opacity: f64,
	},
	BlendModeSet {
		layer: LayerNodeIdentifier,
		blend_mode: BlendMode,
	},
	ClipModeToggle {
		layer: LayerNodeIdentifier,
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
	Vector {
		layer: LayerNodeIdentifier,
		modification_type: VectorModificationType,
	},
	Brush {
		layer: LayerNodeIdentifier,
		strokes: Vec<BrushStroke>,
	},
	SetUpstreamToChain {
		layer: LayerNodeIdentifier,
	},
	NewArtboard {
		id: NodeId,
		artboard: Artboard,
	},
	NewBitmapLayer {
		id: NodeId,
		image_frame: Table<Raster<CPU>>,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	NewBooleanOperationLayer {
		id: NodeId,
		operation: graphene_std::path_bool::BooleanOperation,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	NewCustomLayer {
		id: NodeId,
		nodes: Vec<(NodeId, NodeTemplate)>,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	NewVectorLayer {
		id: NodeId,
		subpaths: Vec<Subpath<PointId>>,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	NewTextLayer {
		id: NodeId,
		text: String,
		font: Font,
		typesetting: TypesettingConfig,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	ResizeArtboard {
		layer: LayerNodeIdentifier,
		location: IVec2,
		dimensions: IVec2,
	},
	RemoveArtboards,
	NewSvg {
		id: NodeId,
		svg: String,
		transform: DAffine2,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
}
