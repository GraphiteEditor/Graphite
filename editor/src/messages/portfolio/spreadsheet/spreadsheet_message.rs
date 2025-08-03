use crate::messages::prelude::*;
use crate::node_graph_executor::InspectResult;

/// The spreadsheet UI allows for graph data to be previewed.
#[impl_message(Message, PortfolioMessage, Spreadsheet)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SpreadsheetMessage {
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

	ViewVectorDataDomain {
		domain: VectorDataDomain,
	},
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorDataDomain {
	#[default]
	Points,
	Segments,
	Regions,
}
