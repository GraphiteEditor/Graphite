use super::utility_types::TransformOp;
use crate::messages::layout::utility_types::widgets::assist_widgets::PivotPosition;
use crate::messages::portfolio::document::utility_types::misc::TargetDocument;
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
	ModifyFont { font_family: String, font_style: String, size: f64 },
	ModifyName { name: String },
	ModifyPreserveAspect { preserve_aspect: bool },
	ModifyStroke { stroke: Stroke },
	ModifyTransform { value: f64, transform_op: TransformOp },
	ResendActiveProperties,
	SetActiveLayers { paths: Vec<Vec<LayerId>>, document: TargetDocument },
	SetPivot { new_position: PivotPosition },
	UpdateSelectedDocumentProperties,
}
