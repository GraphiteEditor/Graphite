use crate::messages::portfolio::document::utility_types::document_metadata::DocumentMetadata;
use crate::messages::portfolio::document::utility_types::network_metadata::NodeNetworkInterface;
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::prelude::NodeGraphMessageHandler;
use crate::node_graph_executor::NodeGraphExecutor;

use graph_craft::document::{NodeId, NodeNetwork};

pub struct PropertiesPanelMessageHandlerData<'a> {
	pub network_interface: &'a NodeNetworkInterface,
	pub document_metadata: &'a mut DocumentMetadata,
	pub document_name: &'a str,
	pub selected_nodes: &'a SelectedNodes,
	pub executor: &'a mut NodeGraphExecutor,
}
