use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, PropertiesPanel)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PropertiesPanelMessage {
	ClearSelection,
	MaybeDelete { path: Vec<LayerId> },
	MaybeUpdate { path: Vec<LayerId> },
	ModifyTransform { value: f64, transform_op: TransformOp },
	SetActiveLayer { path: Vec<LayerId> },
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum TransformOp {
	X,
	Y,
	Width,
	Height,
	Rotation,
}
