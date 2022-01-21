use crate::message_prelude::*;

use graphene::Operation as DocumentOperation;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, Artboard)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ArtboardMessage {
	// Sub-messages
	#[remain::unsorted]
	DispatchOperation(Box<DocumentOperation>),

	// Messages
	AddArtboard {
		top: f64,
		left: f64,
		height: f64,
		width: f64,
	},
	RenderArtboards,
}

impl From<DocumentOperation> for ArtboardMessage {
	fn from(operation: DocumentOperation) -> Self {
		Self::DispatchOperation(Box::new(operation))
	}
}
