//! Which entities a run of ops named, for a runtime mirror of the registry to bring back into line
//! without being rebuilt from the whole registry.
//!
//! The set is over-approximate on purpose: an op is recorded whether or not it changed anything, so a
//! late-writer-wins loser, an idempotent replay and a failed apply all land here. What the op did to the
//! registry is settled by comparing the entity's stored and mirrored forms afterwards, which is cheaper
//! than having every apply arm report its outcome and stays correct when an op resurrects an entity it
//! only references.

use std::collections::BTreeSet;

use crate::{NetworkId, NodeId, NodeInput, RegistryDelta};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Touched {
	pub nodes: BTreeSet<NodeId>,
	pub networks: BTreeSet<NetworkId>,
	/// Some resource entry changed. Resources are mirrored as one registry rather than per entity.
	pub resources: bool,
}

impl Touched {
	/// Records every entity `op` names: its target, and any node it references, since applying it can
	/// resurrect a referenced node that was concurrently removed.
	pub fn record(&mut self, op: &RegistryDelta) {
		match op {
			RegistryDelta::AddNode { id, node } => {
				self.nodes.insert(*id);
				self.reference_inputs(node.inputs().iter().map(|slot| &slot.input));
			}
			RegistryDelta::RemoveNode { id, .. }
			| RegistryDelta::SetNodeImplementation { id, .. }
			| RegistryDelta::ChangeNodeAttribute { id, .. }
			| RegistryDelta::ChangeNodeInputAttribute { id, .. } => {
				self.nodes.insert(*id);
			}
			RegistryDelta::ChangeNodeInput { id, new_input, .. } => {
				self.nodes.insert(*id);
				self.reference_inputs([new_input]);
			}
			RegistryDelta::SetNodeInputs { id, inputs } => {
				self.nodes.insert(*id);
				self.reference_inputs(inputs.iter().map(|slot| &slot.input));
			}
			RegistryDelta::SetNetworkExport { id, export, .. } => {
				self.networks.insert(*id);
				self.reference_inputs(export.iter());
			}
			RegistryDelta::ChangeNetworkAttribute { id, .. } | RegistryDelta::AddNetwork { id, .. } | RegistryDelta::RemoveNetwork { id, .. } => {
				self.networks.insert(*id);
			}
			RegistryDelta::AddResource { .. } | RegistryDelta::SetResourceHash { .. } | RegistryDelta::RemoveResource { .. } | RegistryDelta::AddSource { .. } | RegistryDelta::RemoveSource { .. } => {
				self.resources = true;
			}
			RegistryDelta::RegisterPeer { .. } | RegistryDelta::ChangeDocumentAttribute { .. } | RegistryDelta::Merge { .. } | RegistryDelta::EndTransaction | RegistryDelta::Other(_) => {}
		}
	}

	pub fn extend(&mut self, other: Touched) {
		self.nodes.extend(other.nodes);
		self.networks.extend(other.networks);
		self.resources |= other.resources;
	}

	pub fn is_empty(&self) -> bool {
		self.nodes.is_empty() && self.networks.is_empty() && !self.resources
	}

	fn reference_inputs<'a>(&mut self, inputs: impl IntoIterator<Item = &'a NodeInput>) {
		for input in inputs {
			if let NodeInput::Node { id, .. } = input {
				self.nodes.insert(*id);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{AttributeDelta, Network, Node};

	#[test]
	fn an_input_write_touches_the_node_it_references_as_well_as_its_target() {
		let mut touched = Touched::default();
		touched.record(&RegistryDelta::ChangeNodeInput {
			id: NodeId(1),
			index: 0,
			new_input: NodeInput::Node { id: NodeId(2), index: 0 },
		});

		assert_eq!(touched.nodes, BTreeSet::from([NodeId(1), NodeId(2)]));
		assert!(touched.networks.is_empty());
		assert!(!touched.resources);
	}

	#[test]
	fn an_export_touches_the_network_and_the_node_it_targets() {
		let mut touched = Touched::default();
		touched.record(&RegistryDelta::SetNetworkExport {
			id: NetworkId(3),
			index: 0,
			export: Some(NodeInput::Node { id: NodeId(4), index: 0 }),
		});

		assert_eq!(touched.networks, BTreeSet::from([NetworkId(3)]));
		assert_eq!(touched.nodes, BTreeSet::from([NodeId(4)]));
	}

	#[test]
	fn ops_outside_the_graph_touch_nothing_or_only_the_resource_flag() {
		let mut touched = Touched::default();
		touched.record(&RegistryDelta::ChangeDocumentAttribute {
			delta: AttributeDelta { key: "key".into(), value: None },
		});
		touched.record(&RegistryDelta::Merge { extra_parents: Vec::new() });
		assert!(touched.is_empty());

		touched.record(&RegistryDelta::SetResourceHash {
			id: crate::ResourceId::new(),
			hash: None,
		});
		assert!(touched.resources);
		assert!(touched.nodes.is_empty());
	}

	#[test]
	fn a_node_add_touches_the_node_and_what_its_inputs_reference() {
		let mut node = Node::dummy();
		node.inputs.push(crate::InputSlot {
			input: NodeInput::Node { id: NodeId(9), index: 0 },
			timestamp: crate::TimeStamp::ORIGIN,
			attributes: Default::default(),
			attributes_timestamp: crate::TimeStamp::ORIGIN,
		});
		let mut touched = Touched::default();
		touched.record(&RegistryDelta::AddNode { id: NodeId(8), node });
		touched.record(&RegistryDelta::AddNetwork {
			id: NetworkId(5),
			network: Network::default(),
		});

		assert_eq!(touched.nodes, BTreeSet::from([NodeId(8), NodeId(9)]));
		assert_eq!(touched.networks, BTreeSet::from([NetworkId(5)]));
	}
}
