use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, DialogMessage, ExportDialog)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ExportDialogMessage {
	FileName(String),
	FileType(FileType),
	ScaleFactor(f64),
	ExportBounds(ExportBounds),

	Submit,
}
