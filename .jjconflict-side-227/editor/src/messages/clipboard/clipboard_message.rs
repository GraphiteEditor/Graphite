use crate::messages::clipboard::utility_types::{ClipboardContent, ClipboardContentRaw, ClipboardItem, ClipboardLayer, ClipboardVectorEntry};
use crate::messages::prelude::*;

#[impl_message(Message, Clipboard)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ClipboardMessage {
	Cut,
	Copy,
	Paste,
	ReadClipboard { content: ClipboardContentRaw },
	ReadSelection { content: Option<String>, cut: bool },
	Write { content: ClipboardContent },

	CopyLayers,
	CutLayers,
	WriteItems { items: Vec<ClipboardItem> },
	PasteItems { data: String },
	PasteLayers { entries: Vec<ClipboardLayer> },
	PasteVectors { paths: Vec<ClipboardVectorEntry> },
}
