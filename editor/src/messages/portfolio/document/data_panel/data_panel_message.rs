use crate::messages::prelude::*;
use crate::node_graph_executor::InspectResult;

/// The Data panel UI allows the user to visualize the output data of the selected node.
#[impl_message(Message, DocumentMessage, DataPanel)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DataPanelMessage {
	UpdateLayout {
		#[serde(skip)]
		inspect_result: InspectResult,
	},
	ClearLayout,

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
