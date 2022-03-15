use super::layer_panel::LayerMetadata;

use graphene::layers::layer_info::Layer;

use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Clipboard {
	Internal,

	_InternalClipboardCount, // Keep this as the last entry in internal clipboards since it is used for counting the number of enum variants

	Device,
}

pub const INTERNAL_CLIPBOARD_COUNT: u8 = Clipboard::_InternalClipboardCount as u8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopyBufferEntry {
	pub layer: Layer,
	pub layer_metadata: LayerMetadata,
}
