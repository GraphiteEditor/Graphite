use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::prelude::*;

#[impl_message(Message, DialogMessage, ExportDialog)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ExportDialogMessage {
	FileType { file_type: FileType },
	ScaleFactor { factor: f64 },
	ExportBounds { bounds: ExportBounds },
	Animated { animated: bool },
	Fps { fps: f64 },
	StartSeconds { start: f64 },
	EndSeconds { end: f64 },

	Submit,
}
