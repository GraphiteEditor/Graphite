use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, PropertiesPanel)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PropertiesPanelMessage {
	ClearSelection,
	SetActiveHeight(f64),
	SetActiveLayer(Vec<LayerId>),
	SetActiveRotation(f64),
	SetActiveSkew(f64),
	SetActiveWidth(f64),
	SetActiveX(f64),
	SetActiveY(f64),
}
