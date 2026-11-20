use super::utility_types::{DataTransfer, DataTransferItem, IngestAction, TypeFilter};
use crate::messages::prelude::*;
use graph_craft::document::NodeId;

#[impl_message(Message, PortfolioMessage, Ingest)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum IngestMessage {
	Ingest {
		content: DataTransfer,
		action: IngestAction,
	},
	Item {
		item: DataTransferItem,
		action: IngestAction,
	},
	Browse {
		filters: Vec<TypeFilter>,
		multiple: bool,
		action: IngestAction,
	},
	Open,
	Import,
	SetResourceInput {
		document_id: DocumentId,
		node_id: NodeId,
		input_index: usize,
		filters: Vec<TypeFilter>,
	},
}
