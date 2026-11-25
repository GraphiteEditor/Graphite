//! Deterministic simulation of a document whose chains branch off inputs past the primary one, with every history
//! operation mixed in. After every operation nothing may name a node the document no longer has.

use super::simulation::{LEAF, Random, Scenario, below, empty_export, from_env, on, simulate, stack, value};
use crate::{Implementation, Network, Node, NodeId, NodeInput, ROOT_NETWORK, Registry, RegistryDelta, Session};

/// A slot that can name a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Place {
	Export(u32),
	Input(NodeId, u32),
}

fn place_value(registry: &Registry, place: Place) -> Option<NodeInput> {
	match place {
		Place::Input(id, index) => registry.node_instances.get(&id)?.inputs.get(index as usize).map(|slot| slot.input.clone()),
		Place::Export(index) => registry.networks.get(&ROOT_NETWORK)?.exports.get(index as usize)?.target.clone(),
	}
}

fn set_place(place: Place, input: NodeInput) -> RegistryDelta {
	match place {
		Place::Input(id, index) => RegistryDelta::ChangeNodeInput { id, index, new_input: input },
		Place::Export(index) => RegistryDelta::SetNetworkExport {
			id: ROOT_NETWORK,
			index,
			export: Some(input),
		},
	}
}

/// Every slot naming a node, and the node it names, in a fixed order.
fn naming_places(registry: &Registry) -> Vec<(Place, NodeId)> {
	let mut places = Vec::new();
	for (&id, node) in &registry.node_instances {
		for (index, slot) in node.inputs.iter().enumerate() {
			if let NodeInput::Node { id: named, .. } = slot.input {
				places.push((Place::Input(id, index as u32), named));
			}
		}
	}
	if let Some(network) = registry.networks.get(&ROOT_NETWORK) {
		for (index, slot) in network.exports.iter().enumerate() {
			if let Some(NodeInput::Node { id: named, .. }) = slot.target {
				places.push((Place::Export(index as u32), named));
			}
		}
	}
	places.sort();
	places
}

/// The slot naming `node`, of which a document built this way has one.
fn consumer_place(registry: &Registry, node: NodeId) -> Option<Place> {
	naming_places(registry).into_iter().find(|(_, named)| *named == node).map(|(place, _)| place)
}

/// Every node under `place`, through any input.
fn under(registry: &Registry, place: Place) -> Vec<NodeId> {
	let mut seen: Vec<NodeId> = Vec::new();
	let mut queue: Vec<NodeId> = Vec::new();
	if let Some(NodeInput::Node { id, .. }) = place_value(registry, place) {
		queue.push(id);
	}
	while let Some(id) = queue.pop() {
		if seen.contains(&id) || !registry.node_instances.contains_key(&id) {
			continue;
		}
		seen.push(id);
		for slot in &registry.node_instances[&id].inputs {
			if let NodeInput::Node { id: next, .. } = slot.input {
				queue.push(next);
			}
		}
	}
	seen
}

/// Nodes chained off the export and off inputs past the primary one.
struct Branching;

impl Scenario for Branching {
	fn random_edit(random: &mut Random, session: &mut Session) -> Vec<RegistryDelta> {
		let registry = session.registry().clone();
		let layers = stack(&registry);
		let nodes = under(&registry, Place::Export(0));
		let leaf = (!registry.networks.contains_key(&LEAF)).then(|| RegistryDelta::AddNetwork {
			id: LEAF,
			network: Network::default(),
		});
		let make = |first: NodeInput, second: NodeInput| {
			let mut node = Node::new(ROOT_NETWORK, Implementation::Network(LEAF), 2);
			node.inputs[0].input = first;
			node.inputs[1].input = second;
			node
		};
		// Nodes with nothing hanging off their second input, so taking one does not leave a chain behind
		let plain: Vec<NodeId> = nodes
			.iter()
			.copied()
			.filter(|id| !matches!(place_value(&registry, Place::Input(*id, 1)), Some(NodeInput::Node { .. })))
			.collect();

		match random.below(12) {
			// Draw a node on top of the chain off the export
			0..=2 => {
				let id = session.next_node_id();
				let node = make(layers.first().map_or_else(|| value(0), |&top| on(top)), value(random.below(100)));
				let mut ops: Vec<RegistryDelta> = leaf.into_iter().collect();
				ops.extend([
					RegistryDelta::AddNode { id, node },
					RegistryDelta::SetNetworkExport {
						id: ROOT_NETWORK,
						index: 0,
						export: Some(on(id)),
					},
				]);
				if let Some(&under) = layers.first() {
					ops.push(super::crdt::change_node_attribute(under, "position", serde_json::json!(random.below(100))));
				}
				ops
			}
			// Hang a node on an input past the primary one, over whatever is there already
			3..=5 => {
				let Some(host) = random.pick(&nodes) else { return Vec::new() };
				let id = session.next_node_id();
				let first = match place_value(&registry, Place::Input(host, 1)) {
					Some(NodeInput::Node { id: under, index }) => NodeInput::Node { id: under, index },
					_ => value(0),
				};
				let node = make(first, value(random.below(100)));
				let mut ops: Vec<RegistryDelta> = leaf.into_iter().collect();
				ops.extend([
					RegistryDelta::AddNode { id, node },
					RegistryDelta::ChangeNodeInput {
						id: host,
						index: 1,
						new_input: on(id),
					},
				]);
				ops
			}
			// Delete a node, reconnecting everything that named it to what it rested on
			6..=7 => {
				let Some(node) = random.pick(&plain) else { return Vec::new() };
				let snapshot = registry.node_instances[&node].clone();
				let mut ops: Vec<RegistryDelta> = naming_places(&registry)
					.into_iter()
					.filter(|(_, named)| *named == node)
					.map(|(place, _)| set_place(place, below(&registry, node)))
					.collect();
				ops.push(RegistryDelta::RemoveNode { id: node, snapshot });
				ops
			}
			// Move a node to the top of the chain off the export
			8 => {
				let Some(layer) = random.pick(&layers[layers.len().min(1)..]) else { return Vec::new() };
				let Some(place) = consumer_place(&registry, layer) else { return Vec::new() };
				vec![
					set_place(place, below(&registry, layer)),
					RegistryDelta::ChangeNodeInput {
						id: layer,
						index: 0,
						new_input: on(layers[0]),
					},
					RegistryDelta::SetNetworkExport {
						id: ROOT_NETWORK,
						index: 0,
						export: Some(on(layer)),
					},
				]
			}
			// Change a node's value
			9..=10 => {
				let Some(node) = random.pick(&plain) else { return Vec::new() };
				vec![RegistryDelta::ChangeNodeInput {
					id: node,
					index: 1,
					new_input: value(random.below(100)),
				}]
			}
			// Cut the chain off an input past the primary one, taking its nodes with it
			_ => {
				let hosts: Vec<NodeId> = nodes
					.iter()
					.copied()
					.filter(|id| matches!(place_value(&registry, Place::Input(*id, 1)), Some(NodeInput::Node { .. })))
					.collect();
				let Some(host) = random.pick(&hosts) else { return Vec::new() };
				let mut gone = under(&registry, Place::Input(host, 1));
				// Nothing outside the cut may name a node inside it, or taking them would leave that naming nothing
				let held = naming_places(&registry)
					.into_iter()
					.any(|(place, named)| gone.contains(&named) && place != Place::Input(host, 1) && !matches!(place, Place::Input(owner, _) if gone.contains(&owner)));
				if held {
					return Vec::new();
				}
				let mut ops = vec![RegistryDelta::ChangeNodeInput {
					id: host,
					index: 1,
					new_input: value(random.below(100)),
				}];
				gone.sort();
				for id in gone {
					ops.push(RegistryDelta::RemoveNode {
						id,
						snapshot: registry.node_instances[&id].clone(),
					});
				}
				ops
			}
		}
	}

	/// Nothing names a node the document no longer has, every node is built from a network it still has, and no node
	/// input holds the empty export's value. A node may feed nothing or more than one input.
	fn assert_well_formed(context: &str, registry: &Registry) {
		for (place, named) in naming_places(registry) {
			assert!(registry.node_instances.contains_key(&named), "{context}: {place:?} names {named:?}, which is gone");
		}
		for (id, node) in &registry.node_instances {
			if let Implementation::Network(network) = &node.implementation {
				assert!(registry.networks.contains_key(network), "{context}: {id:?} is built from {network:?}, which is gone");
			}
			for (index, slot) in node.inputs.iter().enumerate() {
				assert_ne!(slot.input, empty_export(), "{context}: input {index} of {id:?} holds the empty export's value");
			}
		}
	}
}

#[test]
fn history_operations_keep_a_branching_document_whole() {
	let (seeds, steps) = (from_env("SEEDS", 200), from_env("STEPS", 80));
	for seed in 0..seeds {
		simulate::<Branching>(seed, steps, &mut Vec::new());
	}
}
