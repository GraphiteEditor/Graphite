use document_legacy::document::Document as DocumentLegacy;
use document_legacy::LayerId;

use serde::{Deserialize, Serialize};

use crate::messages::prelude::NodeGraphMessageHandler;

pub struct PropertiesPanelMessageHandlerData<'a> {
	pub artwork_document: &'a DocumentLegacy,
	pub artboard_document: &'a DocumentLegacy,
	pub selected_layers: &'a mut dyn Iterator<Item = &'a [LayerId]>,
	pub node_graph_message_handler: &'a NodeGraphMessageHandler,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TransformOp {
	X,
	Y,
	ScaleX,
	ScaleY,
	Width,
	Height,
	Rotation,
}
