use document_legacy::document::Document as DocumentLegacy;
use document_legacy::LayerId;

use crate::{messages::prelude::NodeGraphMessageHandler, node_graph_executor::NodeGraphExecutor};

pub struct PropertiesPanelMessageHandlerData<'a> {
	pub document_name: &'a str,
	pub artwork_document: &'a DocumentLegacy,
	pub selected_layers: &'a mut dyn Iterator<Item = &'a [LayerId]>,
	pub node_graph_message_handler: &'a NodeGraphMessageHandler,
	pub executor: &'a mut NodeGraphExecutor,
}
