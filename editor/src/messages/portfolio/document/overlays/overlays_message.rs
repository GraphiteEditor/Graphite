use crate::messages::prelude::*;

use graphene::Operation as DocumentOperation;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, Overlays)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
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
