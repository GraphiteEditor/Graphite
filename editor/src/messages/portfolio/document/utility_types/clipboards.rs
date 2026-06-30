use super::network_interface::NodeTemplate;
use graph_craft::application_io::resource::{DataSource, ResourceHash, ResourceId};
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
	#[serde(default)]
	pub resources: Vec<ClipboardResource>,
}

/// A snapshot of a document's resource registry entry, carried in the clipboard so a paste can re-register it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClipboardResource {
	pub id: ResourceId,
	pub sources: Vec<DataSource>,
	pub hash: Option<ResourceHash>,
}
