use crate::messages::clipboard::utility_types::{ClipboardItem, ClipboardLayer, ClipboardVectorEntry};
use crate::messages::portfolio::ingest::utility_types::DataTransfer;
use crate::messages::prelude::*;

#[impl_message(Message, Clipboard)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ClipboardMessage {
	Cut,
	Copy,
	Paste,
	ReadSelection { content: Option<String>, cut: bool },
	Write { content: DataTransfer },

	CopyLayers,
	CutLayers,
	WriteItems { items: Vec<ClipboardItem> },
	Rendered { id: u64, svg: Option<String> },
	Loaded { id: u64, content: DataTransfer },
	PasteItems { items: Vec<ClipboardItem> },
	PasteLayers { entries: Vec<ClipboardLayer> },
	PasteVectors { paths: Vec<ClipboardVectorEntry> },
}
