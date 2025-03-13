use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::node_graph_executor::NodeGraphExecutor;
use graph_craft::document::NodeId;

pub struct PropertiesPanelMessageHandlerData<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub selection_network_path: &'a [NodeId],
	pub document_name: &'a str,
	pub executor: &'a mut NodeGraphExecutor,
}
