use crate::{Attributes, NetworkId, NodeId, ResourceId, TimeStamp, attributes_value_equal};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
	pub(crate) implementation: Implementation,
	pub(crate) inputs: Vec<InputSlot>,
	pub(crate) inputs_attributes: Vec<Attributes>,
	pub(crate) attributes: Attributes,
	pub(crate) network: NetworkId,
}

impl Node {
	pub fn implementation(&self) -> &Implementation {
		&self.implementation
	}
	pub fn inputs(&self) -> &[InputSlot] {
		&self.inputs
	}
	pub fn inputs_attributes(&self) -> &[Attributes] {
		&self.inputs_attributes
	}
	pub fn attributes(&self) -> &Attributes {
		&self.attributes
	}
	pub fn network(&self) -> NetworkId {
		self.network
	}

	/// True if both nodes agree on every value-bearing field, ignoring slot/attribute timestamps.
	pub fn value_equal(&self, other: &Self) -> bool {
		if self.implementation != other.implementation || self.network != other.network {
			return false;
		}
		if self.inputs.len() != other.inputs.len() {
			return false;
		}
		if !self.inputs.iter().zip(&other.inputs).all(|(a, b)| a.input == b.input) {
			return false;
		}
		if self.inputs_attributes.len() != other.inputs_attributes.len() {
			return false;
		}
		if !self.inputs_attributes.iter().zip(&other.inputs_attributes).all(|(a, b)| attributes_value_equal(a, b)) {
			return false;
		}
		attributes_value_equal(&self.attributes, &other.attributes)
	}
}

/// One positional input. The timestamp drives LWW on concurrent `ChangeNodeInput` ops targeting
/// the same `(node_id, input_idx)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputSlot {
	pub input: NodeInput,
	pub timestamp: TimeStamp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeInput {
	Node {
		node_id: NodeId,
		output_index: usize,
	},
	Value {
		value: serde_json::Value,
		exposed: bool,
	},
	Scope(Cow<'static, str>),
	Import {
		import_idx: usize,
	},
	/// Marker; the `DocumentNodeMetadata` lives in `inputs_attributes`.
	Reflection,
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
	pub exports: Vec<ExportSlot>,
	/// Per-network `ui::*` state (navigation, previewing). Separate from `Node.attributes` so
	/// view-state edits LWW independently.
	pub attributes: Attributes,
}

impl Network {
	/// True if both networks agree on every value-bearing field, ignoring slot/attribute timestamps.
	pub fn value_equal(&self, other: &Self) -> bool {
		if self.exports.len() != other.exports.len() {
			return false;
		}
		if !self.exports.iter().zip(&other.exports).all(|(a, b)| a.target == b.target) {
			return false;
		}
		attributes_value_equal(&self.attributes, &other.attributes)
	}
}

/// One positional export slot. `target == None` marks an empty/removed slot. Timestamp drives LWW
/// on concurrent `SetExport` ops.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportSlot {
	pub target: Option<NodeInput>,
	pub timestamp: TimeStamp,
}

/// Content of a proto-node declaration. Stored as a content-addressed resource (serialized bytes
/// keyed by `ResourceHash`, held by the `Gdd` byte store) and referenced from
/// `Implementation::ProtoNode(ResourceId)`. `graph-storage` itself only holds the reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtoNode {
	pub identifier: String,
	pub code: Option<String>,
	pub wasm: Option<Vec<u8>>,
	pub attributes: Attributes,
}
