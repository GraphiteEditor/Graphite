use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, PropertiesPanel)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PropertiesPanelMessage {
	CheckSelectedWasDeleted { path: Vec<LayerId> },
	CheckSelectedWasUpdated { path: Vec<LayerId> },
	ClearSelection,
	ModifyFill { value: String },
	ModifyName { name: String },
	ModifyStroke { color: Option<String>, weight: Option<f64> },
	ModifyTransform { value: f64, transform_op: TransformOp },
	SetActiveLayers { paths: Vec<Vec<LayerId>> },
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum TransformOp {
	X,
	Y,
	Width,
	Height,
	Rotation,
}
