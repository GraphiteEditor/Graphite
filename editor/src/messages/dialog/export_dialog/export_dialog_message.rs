use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::prelude::*;

#[impl_message(Message, DialogMessage, ExportDialog)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ExportDialogMessage {
	FileType(FileType),
	ScaleFactor(f64),
	TransparentBackground(bool),
	ExportBounds(ExportBounds),

	Submit,
}
