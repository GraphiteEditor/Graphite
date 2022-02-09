use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, PropertiesPanel)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PropertiesPanelMessage {
	ClearSelection,
	MaybeDelete(Vec<LayerId>),
	MaybeUpdate(Vec<LayerId>),
	ModifyTransform(f64, TransformOp),
	SetActiveLayer(Vec<LayerId>),
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum TransformOp {
	X,
	Y,
	Width,
	Height,
	Rotation,
}
