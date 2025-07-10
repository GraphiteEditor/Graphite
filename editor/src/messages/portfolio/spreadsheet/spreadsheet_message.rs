use crate::messages::prelude::*;
use graph_craft::document::AbsoluteInputConnector;
use graphene_std::uuid::CompiledProtonodeInput;

/// The spreadsheet UI allows for instance data to be previewed.
#[impl_message(Message, PortfolioMessage, Spreadsheet)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SpreadsheetMessage {
	ToggleOpen,

	UpdateLayout { inspect_input: InspectInputConnector },

	PushToInstancePath { index: usize },
	TruncateInstancePath { len: usize },

	ViewVectorDataDomain { domain: VectorDataDomain },
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorDataDomain {
	#[default]
	Points,
	Segments,
	Regions,
}

/// The mapping of input where the data is extracted from to the selected input to display data for
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
// #[cfg_attr(feature = "decouple-execution", derive(serde::Serialize, serde::Deserialize))]
pub struct InspectInputConnector {
	pub input_connector: AbsoluteInputConnector,
	pub protonode_input: CompiledProtonodeInput,
}
