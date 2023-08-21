use super::utility_types::TransformOp;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use document_legacy::layers::style::{Fill, Stroke};
use document_legacy::LayerId;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, PropertiesPanel)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PropertiesPanelMessage {
	// Messages
	CheckSelectedWasDeleted { path: Vec<LayerId> },
	CheckSelectedWasUpdated { path: Vec<LayerId> },
	ClearSelection,
	Deactivate,
	Init,
	ModifyFill { fill: Fill },
	ModifyName { name: String },
	ModifyPreserveAspect { preserve_aspect: bool },
	ModifyStroke { stroke: Stroke },
	ModifyTransform { value: f64, transform_op: TransformOp },
	ResendActiveProperties,
	SetActiveLayers { paths: Vec<Vec<LayerId>> },
	SetPivot { new_position: PivotPosition },
	UpdateSelectedDocumentProperties,
}
