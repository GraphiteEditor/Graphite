use crate::messages::prelude::*;

#[impl_message(Message, PortfolioMessage, FailedDocuments)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum FailedDocumentsMessage {
	// TODO: Eventually remove this document upgrade code
	ShowFailedToLoadDocumentsDialog,
	// TODO: Eventually remove this document upgrade code
	DiscardFailedToLoadDocuments,
	// TODO: Eventually remove this document upgrade code
	DownloadFailedToLoadDocuments,
}
