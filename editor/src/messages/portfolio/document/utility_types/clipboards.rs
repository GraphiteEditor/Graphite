use graph_craft::document::NodeId;

use std::collections::HashMap;

use super::network_interface::NodeTemplate;

#[repr(u8)]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug, specta::Type)]
pub enum Clipboard {
	Internal,

	_InternalClipboardCount, // Keep this as the last entry of **internal** clipboards since it is used for counting the number of enum variants

	Device,
}

pub const INTERNAL_CLIPBOARD_COUNT: u8 = Clipboard::_InternalClipboardCount as u8;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CopyBufferEntry {
	pub nodes: HashMap<NodeId, NodeTemplate>,
	pub selected: bool,
	pub visible: bool,
	pub locked: bool,
	pub collapsed: bool,
}
