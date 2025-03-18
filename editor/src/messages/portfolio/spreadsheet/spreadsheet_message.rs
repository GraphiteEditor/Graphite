use crate::messages::prelude::*;
use crate::node_graph_executor::InspectResult;
use graphene_std::vector::InstanceId;

/// The spreadsheet UI allows for instance data to be previewed.
#[impl_message(Message, PortfolioMessage, Spreadsheet)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SpreadsheetMessage {
	SetOpen {
		open: bool,
	},
	UpdateLayout {
		#[serde(skip)]
		inspect_result: InspectResult,
	},
	PushInstance {
		id: InstanceId,
	},
}
