use crate::message_prelude::*;
use serde::{Deserialize, Serialize};

use super::{ExportDialogUpdate, NewDocumentDialogUpdate};

#[remain::sorted]
#[impl_message(Message, Dialog)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum DialogMessage {
	// Sub-messages
	#[remain::unsorted]
	#[child]
	ExportDialog(ExportDialogUpdate),
	#[remain::unsorted]
	#[child]
	NewDocumentDialog(NewDocumentDialogUpdate),

	// Messages
	CloseAllDocumentsWithConfirmation,
	CloseDialogAndThen {
		followup: Box<Message>,
	},
	DisplayDialogError {
		title: String,
		description: String,
	},
	RequestAboutGraphiteDialog,
	RequestAboutGraphiteDialogWithLocalizedCommitDate {
		localized_commit_date: String,
	},
	RequestComingSoonDialog {
		issue: Option<i32>,
	},
	RequestExportDialog,
	RequestNewDocumentDialog,
}
