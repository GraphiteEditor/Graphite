use crate::messages::prelude::*;

use document_legacy::Operation as DocumentOperation;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, Overlays)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum OverlaysMessage {
	// Sub-messages
	#[remain::unsorted]
	DispatchOperation(Box<DocumentOperation>),

	// Messages
	ClearAllOverlays,
	Rerender,
}

impl From<DocumentOperation> for OverlaysMessage {
	fn from(operation: DocumentOperation) -> OverlaysMessage {
		Self::DispatchOperation(Box::new(operation))
	}
}
