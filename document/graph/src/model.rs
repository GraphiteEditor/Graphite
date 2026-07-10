use crate::{Attributes, NetworkId, NodeId, ResourceId, TimeStamp, attributes_value_equal};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
	pub(crate) implementation: Implementation,
	pub(crate) inputs: Vec<InputSlot>,
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
		if !self
			.inputs
			.iter()
			.zip(&other.inputs)
			.all(|(a, b)| a.input == b.input && attributes_value_equal(&a.attributes, &b.attributes))
		{
			return false;
		}
		attributes_value_equal(&self.attributes, &other.attributes)
	}

	#[cfg(test)]
	pub(crate) fn dummy() -> Self {
		Self {
			implementation: Implementation::ProtoNode(ResourceId::new()),
			inputs: vec![],
			attributes: Attributes::new(),
			network: crate::ROOT_NETWORK,
		}
	}
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
		value: serde_json::Value,
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
	pub exports: Vec<ExportSlot>,
	/// Per-network `ui::*` state (navigation, previewing). Separate from `Node.attributes` so
	/// view-state edits LWW independently.
	pub attributes: Attributes,
}

impl Network {
	/// True if both networks agree on every value-bearing field, ignoring slot/attribute timestamps.
	pub fn value_equal(&self, other: &Self) -> bool {
		// Compare slot targets index-by-index, treating out-of-range slots as `None`. A `SetExport(None)`
		// truncation leaves a trailing empty slot (a tombstone in the CRDT state) that is value-equal to
		// the slot being absent, so trailing `None`s must not count as drift. Mirrors `compute_deltas`
		// (emits nothing for them) and `to_runtime` (drops them).
		let max_len = self.exports.len().max(other.exports.len());
		for slot_idx in 0..max_len {
			let self_target = self.exports.get(slot_idx).and_then(|slot| slot.target.as_ref());
			let other_target = other.exports.get(slot_idx).and_then(|slot| slot.target.as_ref());
			if self_target != other_target {
				return false;
			}
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
/// `Implementation::ProtoNode(ResourceId)`. `document-graph` itself only holds the reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtoNode {
	pub identifier: String,
	pub attributes: Attributes,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::TimeStamp;

	fn target_slot(node_id: u64) -> ExportSlot {
		ExportSlot {
			target: Some(NodeInput::Node { id: NodeId(node_id), index: 0 }),
			timestamp: TimeStamp::ORIGIN,
		}
	}

	fn empty_slot() -> ExportSlot {
		ExportSlot {
			target: None,
			timestamp: TimeStamp { counter: 5, peer: crate::PeerId(1) },
		}
	}

	/// A `SetExport(None)` truncation leaves a trailing empty slot. Such a network is value-equal to
	/// the same network without that slot, so the soak oracle doesn't false-report drift.
	#[test]
	fn trailing_empty_export_slot_is_value_equal() {
		let compact = Network {
			exports: vec![target_slot(1), target_slot(2)],
			..Default::default()
		};
		let with_trailing_empty = Network {
			exports: vec![target_slot(1), target_slot(2), empty_slot()],
			..Default::default()
		};

		assert!(compact.value_equal(&with_trailing_empty));
		assert!(with_trailing_empty.value_equal(&compact));
	}

	/// A `None` slot *between* live targets is a real value difference (a hole), not a trailing
	/// tombstone, so it must still count as drift.
	#[test]
	fn interior_empty_export_slot_is_not_value_equal() {
		let dense = Network {
			exports: vec![target_slot(1), target_slot(2)],
			..Default::default()
		};
		let with_hole = Network {
			exports: vec![target_slot(1), empty_slot(), target_slot(2)],
			..Default::default()
		};

		assert!(!dense.value_equal(&with_hole));
	}
}
