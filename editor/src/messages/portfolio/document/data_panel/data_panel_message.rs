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
	/// Re-render the existing layout against the latest network interface state. Use this when node metadata
	/// (display name, visibility, locked, etc.) changes but the introspected output value hasn't.
	Refresh,

	PushToElementPath {
		step: PathStep,
	},
	TruncateElementPath {
		len: usize,
	},

	ViewVectorTableTab {
		tab: VectorTableTab,
	},
}

/// One hop in the breadcrumb path through nested data the data panel is displaying.
/// Drilling into a row's element produces an `Element` step; drilling into one of a row's attributes produces an `Attribute` step.
#[derive(PartialEq, Eq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PathStep {
	Element(usize),
	Attribute { row: usize, key: String },
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorTableTab {
	#[default]
	Properties,
	Points,
	Segments,
	Regions,
}
