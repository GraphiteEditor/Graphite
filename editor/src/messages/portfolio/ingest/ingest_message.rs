use super::utility_types::{IngestAction, TypeFilter};
use crate::messages::prelude::*;
use graph_craft::document::NodeId;
use std::path::PathBuf;

#[impl_message(Message, PortfolioMessage, Ingest)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum IngestMessage {
	Ingest {
		data: Vec<u8>,
		action: IngestAction,
		mime_type: String,
		path: Option<PathBuf>,
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
