use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Dialog)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum DialogMessage {
	// Sub-messages
	#[remain::unsorted]
	#[child]
	ExportDialog(ExportDialogMessage),
	#[remain::unsorted]
	#[child]
	NewDocumentDialog(NewDocumentDialogMessage),

	// Messages
	CloseAllDocumentsWithConfirmation,
	CloseDialogAndThen {
		followups: Vec<Message>,
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
