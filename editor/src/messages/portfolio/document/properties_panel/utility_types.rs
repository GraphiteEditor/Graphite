use graph_craft::document::NodeId;

use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::node_graph_executor::NodeGraphExecutor;

pub struct PropertiesPanelMessageHandlerData<'a> {
	pub network_interface: &'a NodeNetworkInterface,
	pub selection_network_path: &'a [NodeId],
	pub document_name: &'a str,
	pub executor: &'a mut NodeGraphExecutor,
}
