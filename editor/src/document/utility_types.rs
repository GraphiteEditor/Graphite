pub use super::layer_panel::{layer_panel_entry, LayerMetadata, LayerPanelEntry, RawBuffer};
use graphene::document::Document as GrapheneDocument;
use graphene::LayerId;

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
