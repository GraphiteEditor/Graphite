use crate::messages::portfolio::document::utility_types::document_metadata::DocumentMetadata;
use crate::messages::portfolio::document::utility_types::LayerId;
use crate::messages::prelude::NodeGraphMessageHandler;
use crate::node_graph_executor::NodeGraphExecutor;

use graph_craft::document::NodeNetwork;

pub struct PropertiesPanelMessageHandlerData<'a> {
	pub document_name: &'a str,
	pub document_network: &'a mut NodeNetwork,
	pub document_metadata: &'a mut DocumentMetadata,
	pub node_graph_message_handler: &'a NodeGraphMessageHandler,
	pub executor: &'a mut NodeGraphExecutor,
}
