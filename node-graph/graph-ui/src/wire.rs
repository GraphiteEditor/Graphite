use crate::card::DataType;
use crate::replicated_map::Keyed;
use core_types::uuid::NodeId;
use graphene_hash::CacheHash;
use vello_gpu::kurbo::{BezPath, PathEl};

#[derive(Clone, Debug, PartialEq, CacheHash)]
pub struct Wire {
	pub key: WireKey,
	pub path: BezPath,
	pub data_type: DataType,
	pub thick: bool,
	pub center_line: BezPath,
	pub list: bool,
	pub insertion_target: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, CacheHash)]
pub enum WireKey {
	Input { node: NodeId, index: usize },
	Export(usize),
	Preview,
}

impl Keyed for Wire {
	type Key = WireKey;

	fn key(&self) -> WireKey {
		self.key
	}
}

#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
pub struct WireInProgress {
	pub path: [PathEl; 4],
	pub data_type: DataType,
}
