use crate::messages::clipboard::utility_types::{ClipboardContent, ClipboardContentRaw};
use crate::messages::prelude::*;

#[impl_message(Message, Clipboard)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ClipboardMessage {
	// Frontend/system-clipboard boundary
	Cut,
	Copy,
	Paste,
	ReadClipboard { content: ClipboardContentRaw },
	ReadSelection { content: Option<String>, cut: bool },
	Write { content: ClipboardContent },

	// Graphite-native copy/paste logic
	CopyLayers,
	CutLayers,
	PasteSerializedData { data: String },
	PasteSerializedVector { data: String },
}
