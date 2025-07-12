use crate::messages::prelude::*;
use graphene_std::uuid::{NodeId, SNI};

/// The spreadsheet UI allows for instance data to be previewed.
#[impl_message(Message, PortfolioMessage, Spreadsheet)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SpreadsheetMessage {
	ToggleOpen,

	RequestUpdateLayout,
	ProcessUpdateLayout { node_to_inspect: NodeId, protonode_id: SNI },

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
