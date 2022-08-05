use crate::messages::prelude::*;

use graphene::LayerId;
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
		id: Option<LayerId>,
		position: (f64, f64),
		size: (f64, f64),
	},
	DeleteArtboard {
		artboard: LayerId,
	},
	RenderArtboards,
	ResizeArtboard {
		artboard: LayerId,
		position: (f64, f64),
		size: (f64, f64),
	},
}

impl From<DocumentOperation> for ArtboardMessage {
	fn from(operation: DocumentOperation) -> Self {
		Self::DispatchOperation(Box::new(operation))
	}
}
