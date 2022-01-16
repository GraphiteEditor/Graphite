use crate::message_prelude::*;

use graphene::Operation as DocumentOperation;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, Overlays)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum OverlaysMessage {
	ClearAllOverlays,
	DispatchOperation(Box<DocumentOperation>),
	Rerender,
}

impl From<DocumentOperation> for OverlaysMessage {
	fn from(operation: DocumentOperation) -> OverlaysMessage {
		Self::DispatchOperation(Box::new(operation))
	}
}
