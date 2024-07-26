use super::utility_types::TransformIn;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;
use crate::messages::prelude::*;

use bezier_rs::Subpath;
use graph_craft::document::NodeId;
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::text::Font;
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::vector::PointId;
use graphene_core::vector::VectorModificationType;
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
		image_frame: ImageFrame<Color>,
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
		size: f64,
		parent: LayerNodeIdentifier,
		insert_index: usize,
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
		insert_index: usize,
	},
}
