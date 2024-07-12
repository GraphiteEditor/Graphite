use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::node_graph_executor::NodeGraphExecutor;

pub struct PropertiesPanelMessageHandlerData<'a> {
	pub network_interface: &'a NodeNetworkInterface,
	pub document_name: &'a str,
	pub selected_nodes: &'a SelectedNodes,
	pub executor: &'a mut NodeGraphExecutor,
}
