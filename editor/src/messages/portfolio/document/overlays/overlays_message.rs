use crate::messages::portfolio::document::overlays::overlays_message_handler::OverlayContext;
use crate::messages::prelude::*;
use document_legacy::Operation as DocumentOperation;

use serde::{Deserialize, Serialize};

pub type OverlayProvider = fn(OverlayContext) -> Message;
fn empty_provider() -> OverlayProvider {
	|_| Message::NoOp
}

#[impl_message(Message, DocumentMessage, Overlays)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum OverlaysMessage {
	// Sub-messages
	DispatchOperation(Box<DocumentOperation>),

	// Messages
	ClearAllOverlays,
	Rerender,

	Render,

	// I don't know why we need to serde messages - we never use this functionality
	AddProvider(#[serde(skip, default = "empty_provider")] OverlayProvider),
	RemoveProvider(#[serde(skip, default = "empty_provider")] OverlayProvider),
}

impl From<DocumentOperation> for OverlaysMessage {
	fn from(operation: DocumentOperation) -> OverlaysMessage {
		Self::DispatchOperation(Box::new(operation))
	}
}
