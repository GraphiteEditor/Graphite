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
	#[remain::unsorted]
	#[child]
	PreferencesDialog(PreferencesDialogMessage),

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
		localized_commit_year: String,
	},
	RequestComingSoonDialog {
		issue: Option<i32>,
	},
	RequestDemoArtworkDialog,
	RequestExportDialog,
	RequestLicensesDialogWithLocalizedCommitDate {
		localized_commit_year: String,
	},
	RequestNewDocumentDialog,
	RequestPreferencesDialog,
}
