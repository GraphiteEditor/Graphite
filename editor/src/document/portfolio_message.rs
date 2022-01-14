use super::clipboards::Clipboard;
use crate::message_prelude::*;

use graphene::LayerId;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Portfolio)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PortfolioMessage {
	AutoSaveActiveDocument,
	AutoSaveDocument(u64),
	CloseActiveDocumentWithConfirmation,
	CloseAllDocuments,
	CloseAllDocumentsWithConfirmation,
	CloseDocument(u64),
	CloseDocumentWithConfirmation(u64),
	Copy(Clipboard),
	Cut(Clipboard),
	#[child]
	Document(DocumentMessage),
	NewDocument,
	NextDocument,
	OpenDocument,
	OpenDocumentFile(String, String),
	OpenDocumentFileWithId {
		document: String,
		document_name: String,
		document_id: u64,
		document_is_saved: bool,
	},
	Paste(Clipboard),
	PasteIntoFolder {
		clipboard: Clipboard,
		path: Vec<LayerId>,
		insert_index: isize,
	},
	PrevDocument,
	RequestAboutGraphiteDialog,
	SelectDocument(u64),
	UpdateOpenDocumentsList,
}
