use super::network_interface::NodeTemplate;
use graph_craft::document::NodeId;

#[repr(u8)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Clipboard {
	Internal,
	Device,

	_InternalClipboardCount, // Keep this as the last entry of **internal** clipboards since it is used for counting the number of enum variants
}

pub const INTERNAL_CLIPBOARD_COUNT: u8 = Clipboard::_InternalClipboardCount as u8;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CopyBufferEntry {
	pub nodes: Vec<(NodeId, NodeTemplate)>,
	pub selected: bool,
	pub visible: bool,
	pub locked: bool,
	pub collapsed: bool,
}
