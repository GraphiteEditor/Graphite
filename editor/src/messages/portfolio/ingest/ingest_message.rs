use super::utility_types::{IngestAction, TypeFilter, TypeHint};
use crate::messages::prelude::*;
use graph_craft::document::NodeId;
use std::path::PathBuf;
use std::sync::Arc;

#[impl_message(Message, PortfolioMessage, Ingest)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum IngestMessage {
	Ingest {
		data: Arc<[u8]>,
		action: IngestAction,
		hint: TypeHint,
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
		node_id: NodeId,
		input_index: usize,
		filters: Vec<TypeFilter>,
	},
}
