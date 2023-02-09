use document_legacy::document::Document as DocumentLegacy;
use document_legacy::LayerId;

use serde::{Deserialize, Serialize};

use crate::{messages::prelude::NodeGraphMessageHandler, node_graph_executor::NodeGraphExecutor};

pub struct PropertiesPanelMessageHandlerData<'a> {
	pub artwork_document: &'a DocumentLegacy,
	pub artboard_document: &'a DocumentLegacy,
	pub selected_layers: &'a mut dyn Iterator<Item = &'a [LayerId]>,
	pub node_graph_message_handler: &'a NodeGraphMessageHandler,
	pub executor: &'a mut NodeGraphExecutor,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, specta::Type)]
pub enum TransformOp {
	X,
	Y,
	ScaleX,
	ScaleY,
	Width,
	Height,
	Rotation,
}
