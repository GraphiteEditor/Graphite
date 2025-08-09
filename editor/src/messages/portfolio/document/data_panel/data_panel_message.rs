use crate::messages::prelude::*;
use crate::node_graph_executor::InspectResult;

/// The spreadsheet UI allows for graph data to be previewed.
#[impl_message(Message, DocumentMessage, DataPanel)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DataPanelMessage {
	ToggleOpen,

	UpdateLayout {
		#[serde(skip)]
		inspect_result: InspectResult,
	},

	PushToElementPath {
		index: usize,
	},
	TruncateElementPath {
		len: usize,
	},

	ViewVectorTableTab {
		tab: VectorTableTab,
	},
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorTableTab {
	#[default]
	Properties,
	Points,
	Segments,
	Regions,
}
