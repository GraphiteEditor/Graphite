use crate::message_prelude::*;
use serde::{Deserialize, Serialize};

use super::{ExportDialogUpdate, NewDocumentDialogUpdate};

#[remain::sorted]
#[impl_message(Message, Dialog)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum DialogMessage {
	#[remain::unsorted]
	#[child]
	ExportDialog(ExportDialogUpdate),

	#[remain::unsorted]
	#[child]
	NewDocumentDialog(NewDocumentDialogUpdate),

	CloseAllDocumentsWithConfirmation,
	CloseDialogAndThen {
		followup: Box<Message>,
	},
	DisplayDialogError {
		title: String,
		description: String,
	},
	RequestAboutGraphiteDialog,
	RequestComingSoonDialog {
		issue: Option<i32>,
	},
	RequestExportDialog,
	RequestNewDocumentDialog,
}
