use super::{Attributes, NetworkId, NodeId, TimeStamp, Value};
use graphene_resource::ResourceId;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
	/// The latest addition or write, so a concurrent removal resolves by stamp; see [`Registry::removed_nodes`](crate::Registry::removed_nodes).
	#[serde(default)]
	pub(crate) presence: TimeStamp,
	pub(crate) network: NetworkId,
	/// When the node was last added, which decides its network between concurrent additions of one id.
	#[serde(default)]
	pub(crate) network_timestamp: TimeStamp,
	pub(crate) implementation: Implementation,
	/// When the implementation was last written, so a swap resolves by last-writer-wins rather than by
	/// the order ops happen to arrive in. Absent from documents written before swaps were expressible.
	#[serde(default)]
	pub(crate) implementation_timestamp: TimeStamp,
	pub(crate) inputs: Vec<InputSlot>,
	/// When the input list last changed shape, so list and slot writes resolve alike in either order.
	#[serde(default)]
	pub(crate) inputs_timestamp: TimeStamp,
	pub(crate) attributes: Attributes,
}

/// One positional input. The timestamp drives LWW on concurrent `ChangeNodeInput` ops targeting
/// the same `(node_id, input_idx)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputSlot {
	pub input: NodeInput,
	pub timestamp: TimeStamp,
	pub attributes: Attributes,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeInput {
	Node {
		id: NodeId,
		index: u32,
	},
	Value {
		value: Value,
		exposed: bool,
	},
	Scope(Cow<'static, str>),
	Import {
		index: u32,
	},
	/// Marker; the `DocumentNodeMetadata` lives in `inputs_attributes`.
	Reflection,
	Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Implementation {
	/// References a proto-node declaration resource (see [`ProtoNode`]); the binding to content lives
	/// in `Registry.resources` like any other resource.
	ProtoNode(ResourceId),
	Network(NetworkId),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Network {
	/// The latest addition or write; see [`Node::presence`](crate::Node).
	#[serde(default)]
	pub(crate) presence: TimeStamp,
	pub exports: Vec<ExportSlot>,
	/// When the export list last changed shape; see [`Node::inputs_timestamp`](crate::Node).
	#[serde(default)]
	pub(crate) exports_timestamp: TimeStamp,
	/// Per-network `ui::*` state (navigation, previewing). Separate from `Node.attributes` so
	/// view-state edits LWW independently.
	pub attributes: Attributes,
}

/// One positional export slot. `target == None` marks an empty/removed slot. Timestamp drives LWW
/// on concurrent `SetExport` ops.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExportSlot {
	pub target: Option<NodeInput>,
	pub timestamp: TimeStamp,
}

/// Content of a proto-node declaration. Stored as a content-addressed resource (serialized bytes
/// keyed by `ResourceHash`, held by the `Gdd` byte store) and referenced from
/// `Implementation::ProtoNode(ResourceId)`. `document-graph-storage` itself only holds the reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtoNode {
	pub identifier: String,
	/// The declaration's position in its node's lineage. Omitted at version 1, so first versions keep the bytes, and with them the
	/// content-addressed `ResourceId`, they had before declarations were versioned.
	#[serde(default = "ProtoNode::first_version", skip_serializing_if = "ProtoNode::is_first_version")]
	pub version: u32,
	pub attributes: Attributes,
}

impl ProtoNode {
	fn first_version() -> u32 {
		1
	}

	fn is_first_version(version: &u32) -> bool {
		*version == 1
	}
}
