use super::layer_panel::LayerMetadata;

use graphene::layers::layer_info::Layer;

use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Clipboard {
	System,
	User,
	_ClipboardCount, // Keep this as the last entry since it is used for counting the number of enum variants
}

pub const CLIPBOARD_COUNT: u8 = Clipboard::_ClipboardCount as u8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopyBufferEntry {
	pub layer: Layer,
	pub layer_metadata: LayerMetadata,
}
