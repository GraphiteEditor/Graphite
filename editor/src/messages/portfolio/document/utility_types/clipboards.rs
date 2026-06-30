use super::network_interface::NodeTemplate;
use graph_craft::document::NodeId;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CopyBufferEntry {
	pub nodes: Vec<(NodeId, NodeTemplate)>,
	pub selected: bool,
	pub visible: bool,
	pub locked: bool,
	pub collapsed: bool,
}
