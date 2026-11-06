use crate::{AttributeValue, Implementation, InputSlot, Network, NetworkField, NetworkId, Node, NodeField, NodeId, NodeInput, Prior, Registry, RegistryDelta, ResourceId, Tombstone};
use std::collections::HashMap;
use std::hash::Hash;

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
		| RegistryDelta::RemoveSource { id, .. } => priors.push(whole_resource(registry, *id)),
		RegistryDelta::RegisterPeer { peer, .. } => priors.push(Prior::PeerUser {
			peer: *peer,
			previous: registry.peer_users.get(peer).copied(),
		}),
		RegistryDelta::ChangeDocumentAttribute { delta } => priors.push(Prior::DocumentAttribute {
			key: delta.key.clone(),
			previous: registry.attributes.get(&delta.key).cloned(),
		}),
		RegistryDelta::Merge { .. } | RegistryDelta::Meta | RegistryDelta::Other(_) => {}
	}
	priors
}

/// Puts back what `priors` recorded, newest write first.
pub(crate) fn restore(registry: &mut Registry, priors: &[Prior]) {
	for prior in priors.iter().rev() {
		match prior {
			Prior::Node { id, node } => put_live(&mut registry.node_instances, &mut registry.removed_nodes, *id, node),
			Prior::RemovedNode { id, tombstone } => put_removed(&mut registry.node_instances, &mut registry.removed_nodes, *id, tombstone),
			Prior::AbsentNode { id } => put_absent(&mut registry.node_instances, &mut registry.removed_nodes, *id),
			Prior::Network { id, network } => put_live(&mut registry.networks, &mut registry.removed_networks, *id, network),
			Prior::RemovedNetwork { id, tombstone } => put_removed(&mut registry.networks, &mut registry.removed_networks, *id, tombstone),
			Prior::AbsentNetwork { id } => put_absent(&mut registry.networks, &mut registry.removed_networks, *id),
			Prior::Resource { id, resource } => put_live(&mut registry.resources, &mut registry.removed_resources, *id, resource),
			Prior::RemovedResource { id, tombstone } => put_removed(&mut registry.resources, &mut registry.removed_resources, *id, tombstone),
			Prior::AbsentResource { id } => put_absent(&mut registry.resources, &mut registry.removed_resources, *id),
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
	if let Some(node) = registry.node_instances.get(&id) {
		Prior::Node { id, node: node.clone() }
	} else if let Some(tombstone) = registry.removed_nodes.get(&id) {
		Prior::RemovedNode { id, tombstone: tombstone.clone() }
	} else {
		Prior::AbsentNode { id }
	}
}

fn whole_network(registry: &Registry, id: NetworkId) -> Prior {
	if let Some(network) = registry.networks.get(&id) {
		Prior::Network { id, network: network.clone() }
	} else if let Some(tombstone) = registry.removed_networks.get(&id) {
		Prior::RemovedNetwork { id, tombstone: tombstone.clone() }
	} else {
		Prior::AbsentNetwork { id }
	}
}

fn whole_resource(registry: &Registry, id: ResourceId) -> Prior {
	if let Some(resource) = registry.resources.get(&id) {
		Prior::Resource { id, resource: resource.clone() }
	} else if let Some(tombstone) = registry.removed_resources.get(&id) {
		Prior::RemovedResource { id, tombstone: tombstone.clone() }
	} else {
		Prior::AbsentResource { id }
	}
}

/// One field of a live node; a write landing on a tombstone or placeholder records the whole entity instead.
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

fn put_live<K: Hash + Eq + Copy, T: Clone>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, content: &T) {
	dead.remove(&id);
	live.insert(id, content.clone());
}

fn put_removed<K: Hash + Eq + Copy, T: Clone>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K, tombstone: &Tombstone<T>) {
	live.remove(&id);
	dead.insert(id, tombstone.clone());
}

fn put_absent<K: Hash + Eq + Copy, T>(live: &mut HashMap<K, T>, dead: &mut HashMap<K, Tombstone<T>>, id: K) {
	live.remove(&id);
	dead.remove(&id);
}

fn put_attribute(attributes: &mut crate::Attributes, key: &str, previous: Option<&AttributeValue>) {
	match previous {
		Some(value) => _ = attributes.insert(key.to_string(), value.clone()),
		None => _ = attributes.remove(key),
	}
}
