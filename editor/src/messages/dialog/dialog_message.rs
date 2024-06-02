use crate::messages::prelude::*;

#[impl_message(Message, Dialog)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DialogMessage {
	// Sub-messages
	#[child]
	ExportDialog(ExportDialogMessage),
	#[child]
	NewDocumentDialog(NewDocumentDialogMessage),
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
		issue: Option<u32>,
	},
	RequestDemoArtworkDialog,
	RequestExportDialog,
	RequestLicensesDialogWithLocalizedCommitDate {
		localized_commit_year: String,
	},
	RequestNewDocumentDialog,
	RequestPreferencesDialog,
}
