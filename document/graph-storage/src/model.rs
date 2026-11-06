use crate::registry::attributes_value_equal;
use crate::{Attributes, Implementation, InputSlot, Network, NetworkId, Node, NodeInput, TimeStamp, Value};

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

	// Mutable access for edits on a registry clone that is diffed back into deltas. Live-session edits go through the delta paths instead.
	pub fn implementation_mut(&mut self) -> &mut Implementation {
		&mut self.implementation
	}
	pub fn inputs_mut(&mut self) -> &mut Vec<InputSlot> {
		&mut self.inputs
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

	/// A node in `network` whose `inputs` slots are unset, for a caller that fills them in with
	/// `ChangeNodeInput`. The slot count is fixed at creation, since changing an input addresses a
	/// slot by position.
	pub fn new(network: NetworkId, implementation: Implementation, inputs: usize) -> Self {
		Self {
			presence: TimeStamp::ORIGIN,
			network_timestamp: TimeStamp::ORIGIN,
			inputs_timestamp: TimeStamp::ORIGIN,
			implementation,
			implementation_timestamp: TimeStamp::ORIGIN,
			inputs: vec![InputSlot::unset(TimeStamp::ORIGIN); inputs],
			attributes: Attributes::new(),
			network,
		}
	}

	#[cfg(test)]
	pub(crate) fn dummy() -> Self {
		Self::new(crate::ROOT_NETWORK, Implementation::ProtoNode(crate::ResourceId::new()), 0)
	}
}

impl InputSlot {
	/// A slot holding nothing, stamped `timestamp`.
	pub fn unset(timestamp: TimeStamp) -> Self {
		// `to_runtime` reads this back as a `TaggedValue`, whose unit `None` variant encodes as this string.
		// Pinned by `unset_input_slot_deserializes_as_tagged_value_none`.
		Self {
			input: NodeInput::Value {
				value: Value::Str("None".to_string()),
				exposed: false,
			},
			timestamp,
			attributes: Attributes::empty_at(timestamp),
		}
	}
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{ExportSlot, NodeId, TimeStamp};

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

	/// `to_runtime` has to read back what `Node::new` writes into an unset slot, so its literal must stay
	/// in step with how `TaggedValue::None` serializes.
	#[test]
	fn unset_input_slot_deserializes_as_tagged_value_none() {
		let node = Node::new(crate::ROOT_NETWORK, Implementation::Network(crate::ROOT_NETWORK), 1);
		let NodeInput::Value { value, .. } = &node.inputs()[0].input else {
			panic!("a fresh slot holds a value input");
		};

		let tagged: graph_craft::document::value::TaggedValue = crate::from_value(value).expect("the unset slot value must deserialize");
		assert_eq!(tagged, graph_craft::document::value::TaggedValue::None);
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
