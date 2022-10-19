use graphene::document::Document as GrapheneDocument;
use graphene::LayerId;

use serde::{Deserialize, Serialize};

pub struct PropertiesPanelMessageHandlerData<'a> {
	pub artwork_document: &'a GrapheneDocument,
	pub artboard_document: &'a GrapheneDocument,
	pub selected_layers: &'a mut dyn Iterator<Item = &'a [LayerId]>,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TransformOp {
	X,
	Y,
	ScaleX,
	ScaleY,
	Width,
	Height,
	Rotation,
}
