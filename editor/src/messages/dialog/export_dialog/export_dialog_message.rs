use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::prelude::*;

#[impl_message(Message, DialogMessage, ExportDialog)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ExportDialogMessage {
	#[child]
	FileType(FileType),
	#[child]
	ExportBounds(ExportBounds),
	ScaleFactor(f64),
	TransparentBackground(bool),

	Submit,
}
