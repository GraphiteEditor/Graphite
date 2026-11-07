use crate::messages::prelude::*;

#[impl_message(Message, PortfolioMessage, History)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HistoryMessage {
	Refresh,
	Tick,
	Expand {
		id: String,
		expanded: bool,
	},
	LoadMore,
	GoBack {
		id: String,
	},
	RemoveStep {
		id: String,
	},
	BringBack {
		id: String,
	},
	Follow {
		id: Option<String>,
	},
	Rename {
		id: String,
		label: String,
	},
	Tag {
		id: String,
		tag: String,
		on: bool,
	},
	Captured {
		document_id: DocumentId,
		step: document_graph_storage::TimeStamp,
		capture: Option<Box<super::utility_types::DocumentCapture>>,
	},
}
