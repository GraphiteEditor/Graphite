use crate::messages::prelude::*;

#[impl_message(Message, PortfolioMessage, PersistentState)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PersistentStateMessage {
	ReadState,
	WriteState,
	ReadDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
	},
	WriteDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
		document: String,
	},
	DeleteDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
	},
}
