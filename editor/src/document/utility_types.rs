pub use super::layer_panel::{layer_panel_entry, LayerMetadata, LayerPanelEntry, RawBuffer};

use graphene::document::Document as GrapheneDocument;
use graphene::LayerId;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type DocumentSave = (GrapheneDocument, HashMap<Vec<LayerId>, LayerMetadata>);

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
	Average,
}

#[derive(PartialEq, Clone, Debug)]
pub enum VectorManipulatorSegment {
	Line(DVec2, DVec2),
	Quad(DVec2, DVec2, DVec2),
	Cubic(DVec2, DVec2, DVec2, DVec2),
}

#[derive(PartialEq, Clone, Debug)]
pub struct VectorManipulatorShape {
	/// The path to the layer
	pub layer_path: Vec<LayerId>,
	/// The outline of the shape
	pub path: kurbo::BezPath,
	/// The control points / manipulator handles
	pub segments: Vec<VectorManipulatorSegment>,
	/// The compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	pub transform: DAffine2,
}
