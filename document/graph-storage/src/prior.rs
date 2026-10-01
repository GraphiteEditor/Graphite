use crate::{
	AttributeValue, ExportSlot, Implementation, InputSlot, Network, NetworkId, Node, NodeId, NodeInput, PeerId, PeerRegistration, Registry, RegistryDelta, ResourceEntry, ResourceId, TimeStamp,
	Tombstone,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;

/// What a slot held before a delta wrote it, with its own stamps. Undo puts these back as they were, bypassing
/// LWW, so the registry is again exactly what folding history up to the delta's parent gives. Never sent or
/// applied as an op.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Prior {
	/// The whole entity, live or tombstoned. `None, None` means it didn't exist, so undoing an addition leaves no
	/// tombstone behind.
	Node {
		id: NodeId,
		live: Option<Node>,
		removed: Option<Tombstone<Node>>,
	},
	Network {
		id: NetworkId,
		live: Option<Network>,
		removed: Option<Tombstone<Network>>,
	},
	Resource {
		id: ResourceId,
		live: Option<ResourceEntry>,
		removed: Option<Tombstone<ResourceEntry>>,
	},
	/// One field of a live node, and the node's presence, which every write moves.
	NodeField {
		id: NodeId,
		presence: TimeStamp,
		field: NodeField,
	},
	/// One field of a live network, and its presence.
	NetworkField {
		id: NetworkId,
		presence: TimeStamp,
		field: NetworkField,
	},
	DocumentAttribute {
		key: String,
		previous: Option<AttributeValue>,
	},
	PeerUser {
		peer: PeerId,
		previous: Option<PeerRegistration>,
	},
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeField {
	/// Only the presence moved, by a reference from another entity's op.
	Presence,
	Attribute {
		key: String,
		previous: Option<AttributeValue>,
	},
	Input {
		index: u32,
		previous: InputSlot,
	},
	InputAttribute {
		index: u32,
		key: String,
		previous: Option<AttributeValue>,
	},
	Inputs {
		previous: Vec<InputSlot>,
		inputs_timestamp: TimeStamp,
	},
	Implementation {
		previous: Implementation,
		implementation_timestamp: TimeStamp,
	},
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NetworkField {
	/// Only the presence moved, by a node added to or implemented by it.
	Presence,
	Attribute {
		key: String,
		previous: Option<AttributeValue>,
	},
	Exports {
		previous: Vec<ExportSlot>,
	},
}

/// What `op` is about to write in `registry`, in write order. Taken before the op applies.
pub(crate) fn capture(registry: &Registry, op: &RegistryDelta) -> Vec<Prior> {
	let mut priors = Vec::new();
	let node_input = |input: &NodeInput| match input {
		NodeInput::Node { id, .. } => Some(*id),
		_ => None,
	};
	match op {
		RegistryDelta::AddNode { id, node } => {
			priors.push(network_field(registry, node.network, |_| NetworkField::Presence));
			priors.push(whole_node(registry, *id));
		}
		RegistryDelta::RemoveNode { id, .. } => priors.push(whole_node(registry, *id)),
		RegistryDelta::SetNodeInputs { id, inputs } => {
			priors.push(node_field(registry, *id, |node| NodeField::Inputs {
				previous: node.inputs.clone(),
				inputs_timestamp: node.inputs_timestamp,
			}));
			priors.extend(
				inputs
					.iter()
					.filter_map(|slot| node_input(&slot.input))
					.map(|referenced| node_field(registry, referenced, |_| NodeField::Presence)),
			);
		}
		RegistryDelta::ChangeNodeInput { id, index, new_input } => {
			priors.push(node_field(registry, *id, |node| {
				input_field(node, *index, |slot| NodeField::Input {
					index: *index,
					previous: slot.clone(),
				})
			}));
			priors.extend(node_input(new_input).map(|referenced| node_field(registry, referenced, |_| NodeField::Presence)));
		}
		RegistryDelta::SetNodeImplementation { id, implementation } => {
			if let Implementation::Network(network) = implementation {
				priors.push(network_field(registry, *network, |_| NetworkField::Presence));
			}
			priors.push(node_field(registry, *id, |node| NodeField::Implementation {
				previous: node.implementation.clone(),
				implementation_timestamp: node.implementation_timestamp,
			}));
		}
		RegistryDelta::ChangeNodeAttribute { id, delta } => priors.push(node_field(registry, *id, |node| NodeField::Attribute {
			key: delta.key.clone(),
			previous: node.attributes.get(&delta.key).cloned(),
		})),
		RegistryDelta::ChangeNodeInputAttribute { id, index, delta } => priors.push(node_field(registry, *id, |node| {
			input_field(node, *index, |slot| NodeField::InputAttribute {
				index: *index,
				key: delta.key.clone(),
				previous: slot.attributes.get(&delta.key).cloned(),
			})
		})),
		RegistryDelta::SetNetworkExport { id, export, .. } => {
			priors.push(network_field(registry, *id, |network| NetworkField::Exports { previous: network.exports.clone() }));
			priors.extend(export.as_ref().and_then(node_input).map(|referenced| node_field(registry, referenced, |_| NodeField::Presence)));
		}
		RegistryDelta::AddNetwork { id, .. } | RegistryDelta::RemoveNetwork { id, .. } => priors.push(whole_network(registry, *id)),
		RegistryDelta::ChangeNetworkAttribute { id, delta } => {
			priors.push(network_field(registry, *id, |network| NetworkField::Attribute {
				key: delta.key.clone(),
				previous: network.attributes.get(&delta.key).cloned(),
			}));
		}
		RegistryDelta::AddResource { id, .. }
		| RegistryDelta::RemoveResource { id, .. }
		| RegistryDelta::SetResourceHash { id, .. }
		| RegistryDelta::AddSource { id, .. }
		| RegistryDelta::RemoveSource { id, .. } => priors.push(Prior::Resource {
			id: *id,
			live: registry.resources.get(id).cloned(),
			removed: registry.removed_resources.get(id).cloned(),
		}),
		RegistryDelta::RegisterPeer { peer, .. } => priors.push(Prior::PeerUser {
			peer: *peer,
			previous: registry.peer_users.get(peer).copied(),
		}),
		RegistryDelta::ChangeDocumentAttribute { delta } => priors.push(Prior::DocumentAttribute {
			key: delta.key.clone(),
			previous: registry.attributes.get(&delta.key).cloned(),
		}),
		RegistryDelta::Merge { .. } | RegistryDelta::EndTransaction | RegistryDelta::Other(_) => {}
	}
	priors
}

/// Puts back what `priors` recorded, newest write first.
pub(crate) fn restore(registry: &mut Registry, priors: &[Prior]) {
	for prior in priors.iter().rev() {
		match prior {
			Prior::Node { id, live, removed } => put_whole(&mut registry.node_instances, &mut registry.removed_nodes, *id, live.as_ref(), removed.as_ref()),
			Prior::Network { id, live, removed } => put_whole(&mut registry.networks, &mut registry.removed_networks, *id, live.as_ref(), removed.as_ref()),
			Prior::Resource { id, live, removed } => put_whole(&mut registry.resources, &mut registry.removed_resources, *id, live.as_ref(), removed.as_ref()),
			Prior::NodeField { id, presence, field } => {
				let Some(node) = registry.node_instances.get_mut(id) else {
					debug_assert!(false, "a field prior is taken from a live node, which the later priors put back");
					continue;
				};
				node.presence = *presence;
				match field {
					NodeField::Presence => {}
					NodeField::Attribute { key, previous } => put_attribute(&mut node.attributes, key, previous.as_ref()),
					NodeField::Input { index, previous } => {
						if let Some(slot) = node.inputs.get_mut(*index as usize) {
							*slot = previous.clone();
						}
					}
					NodeField::InputAttribute { index, key, previous } => {
						if let Some(slot) = node.inputs.get_mut(*index as usize) {
							put_attribute(&mut slot.attributes, key, previous.as_ref());
						}
					}
					NodeField::Inputs { previous, inputs_timestamp } => {
						node.inputs = previous.clone();
						node.inputs_timestamp = *inputs_timestamp;
					}
					NodeField::Implementation { previous, implementation_timestamp } => {
						node.implementation = previous.clone();
						node.implementation_timestamp = *implementation_timestamp;
					}
				}
			}
			Prior::NetworkField { id, presence, field } => {
				let Some(network) = registry.networks.get_mut(id) else {
					debug_assert!(false, "a field prior is taken from a live network, which the later priors put back");
					continue;
				};
				network.presence = *presence;
				match field {
					NetworkField::Presence => {}
					NetworkField::Attribute { key, previous } => put_attribute(&mut network.attributes, key, previous.as_ref()),
					NetworkField::Exports { previous } => network.exports = previous.clone(),
				}
			}
			Prior::DocumentAttribute { key, previous } => put_attribute(&mut registry.attributes, key, previous.as_ref()),
			Prior::PeerUser { peer, previous } => match previous {
				Some(registration) => _ = registry.peer_users.insert(*peer, *registration),
				None => _ = registry.peer_users.remove(peer),
			},
		}
	}
}

fn whole_node(registry: &Registry, id: NodeId) -> Prior {
	Prior::Node {
		id,
		live: registry.node_instances.get(&id).cloned(),
		removed: registry.removed_nodes.get(&id).cloned(),
	}
}

fn whole_network(registry: &Registry, id: NetworkId) -> Prior {
	Prior::Network {
		id,
		live: registry.networks.get(&id).cloned(),
		removed: registry.removed_networks.get(&id).cloned(),
	}
}

/// One field of a live node; anything else, a write that lands on a tombstone or a placeholder, records the
/// whole entity.
fn node_field(registry: &Registry, id: NodeId, field: impl FnOnce(&Node) -> NodeField) -> Prior {
	match registry.node_instances.get(&id) {
		Some(node) => Prior::NodeField {
			id,
			presence: node.presence,
			field: field(node),
		},
		None => whole_node(registry, id),
	}
}

fn network_field(registry: &Registry, id: NetworkId, field: impl FnOnce(&Network) -> NetworkField) -> Prior {
	match registry.networks.get(&id) {
		Some(network) => Prior::NetworkField {
			id,
			presence: network.presence,
			field: field(network),
		},
		None => whole_network(registry, id),
	}
}

/// A write to one slot, or to the whole list when the slot doesn't exist yet, since writing past the end grows it.
fn input_field(node: &Node, index: u32, field: impl FnOnce(&InputSlot) -> NodeField) -> NodeField {
	match node.inputs.get(index as usize) {
		Some(slot) => field(slot),
		None => NodeField::Inputs {
			previous: node.inputs.clone(),
			inputs_timestamp: node.inputs_timestamp,
		},
	}
}

fn put_whole<K: Hash + Eq + Copy, T: Clone>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, was_live: Option<&T>, was_removed: Option<&Tombstone<T>>) {
	match was_live {
		Some(content) => _ = live.insert(id, content.clone()),
		None => _ = live.remove(&id),
	}
	match was_removed {
		Some(mark) => _ = dead.insert(id, mark.clone()),
		None => _ = dead.remove(&id),
	}
}

fn put_attribute(attributes: &mut crate::Attributes, key: &str, previous: Option<&AttributeValue>) {
	match previous {
		Some(value) => _ = attributes.insert(key.to_string(), value.clone()),
		None => _ = attributes.remove(key),
	}
}
