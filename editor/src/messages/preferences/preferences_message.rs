use crate::messages::portfolio::document::utility_types::wires::GraphWireStyle;
use crate::messages::preferences::SelectionMode;
use crate::messages::prelude::*;

#[impl_message(Message, Preferences)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PreferencesMessage {
	// Management messages
	Load { preferences: Option<PreferencesMessageHandler> },
	ResetToDefaults,

	// Per-preference messages
	UseVello { use_vello: bool },
	SelectionMode { selection_mode: SelectionMode },
	VectorMeshes { enabled: bool },
	ModifyLayout { zoom_with_scroll: bool },
	GraphWireStyle { style: GraphWireStyle },
	ViewportZoomWheelRate { rate: f64 },
}
