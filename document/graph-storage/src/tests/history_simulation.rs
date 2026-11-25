//! Deterministic simulation of a layer stack edited the way the editor does, with every history operation mixed in.
//! After every operation the document must still be a well-formed stack.
//!
//! Inspect one run with `SEED=<n> cargo test -p document-graph-storage history_simulation::inspect_seed -- --ignored --nocapture`.

use super::simulation::{LEAF, Random, Scenario, below, empty_export, from_env, on, simulate, stack, value};
use crate::{Implementation, Network, Node, NodeId, NodeInput, ROOT_NETWORK, Registry, RegistryDelta, Session};
use std::collections::HashSet;

/// Points what sits directly above `layer`, a layer or the export, at `input`.
fn rewire_above(registry: &Registry, layer: NodeId, input: NodeInput) -> RegistryDelta {
	let layers = stack(registry);
	match layers.iter().position(|&id| id == layer) {
		Some(0) | None => RegistryDelta::SetNetworkExport {
			id: ROOT_NETWORK,
			index: 0,
			export: Some(input),
		},
		Some(position) => RegistryDelta::ChangeNodeInput {
			id: layers[position - 1],
			index: 0,
			new_input: input,
		},
	}
}

/// A single stack of layers off the export.
struct Stack;

impl Scenario for Stack {
	fn random_edit(random: &mut Random, session: &mut Session) -> Vec<RegistryDelta> {
		let registry = session.registry().clone();
		let layers = stack(&registry);
		match random.below(10) {
			// Draw a layer on top
			0..=3 => {
				let id = session.next_node_id();
				let mut node = Node::new(ROOT_NETWORK, Implementation::Network(LEAF), 2);
				node.inputs[0].input = layers.first().map_or_else(|| value(0), |&top| on(top));
				node.inputs[1].input = value(random.below(100));
				let leaf = (!registry.networks.contains_key(&LEAF)).then(|| RegistryDelta::AddNetwork {
					id: LEAF,
					network: Network::default(),
				});
				let mut ops: Vec<RegistryDelta> = leaf.into_iter().collect();
				ops.extend([
					RegistryDelta::AddNode { id, node },
					RegistryDelta::SetNetworkExport {
						id: ROOT_NETWORK,
						index: 0,
						export: Some(on(id)),
					},
				]);
				// The layer below moves into the stack under the new one, as the editor records with its position
				if let Some(&below) = layers.first() {
					ops.push(super::crdt::change_node_attribute(below, "position", serde_json::json!(random.below(100))));
				}
				ops
			}
			// Delete a layer, reconnecting what was above it to what was below
			4..=5 => {
				let Some(layer) = random.pick(&layers) else { return Vec::new() };
				let snapshot = registry.node_instances[&layer].clone();
				vec![rewire_above(&registry, layer, below(&registry, layer)), RegistryDelta::RemoveNode { id: layer, snapshot }]
			}
			// Move a layer to the top
			6 => {
				let Some(layer) = random.pick(&layers[layers.len().min(1)..]) else { return Vec::new() };
				vec![
					rewire_above(&registry, layer, below(&registry, layer)),
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
			// Change a layer's value
			_ => {
				let Some(layer) = random.pick(&layers) else { return Vec::new() };
				vec![RegistryDelta::ChangeNodeInput {
					id: layer,
					index: 1,
					new_input: value(random.below(100)),
				}]
			}
		}
	}

	/// Every layer input and the export name a live layer, nothing loops, and every layer is in the stack, so none is
	/// left drawn by nothing.
	fn assert_well_formed(context: &str, registry: &Registry) {
		let exports = registry.networks.get(&ROOT_NETWORK).map(|network| network.exports.clone()).unwrap_or_default();
		let export = exports.first().and_then(|slot| slot.target.clone());
		if let Some(NodeInput::Node { id, .. }) = &export {
			assert!(registry.node_instances.contains_key(id), "{context}: the export names {id:?}, which is gone");
		}
		for (id, node) in &registry.node_instances {
			for (index, slot) in node.inputs.iter().enumerate() {
				if let NodeInput::Node { id: named, .. } = &slot.input {
					assert!(registry.node_instances.contains_key(named), "{context}: input {index} of {id:?} names {named:?}, which is gone");
				}
			}
		}
		let mut stacked_on = HashSet::new();
		let primary_inputs = registry.node_instances.values().filter_map(|node| node.inputs.first()).map(|slot| &slot.input);
		for input in primary_inputs.chain(export.iter()) {
			if let NodeInput::Node { id, .. } = input {
				assert!(stacked_on.insert(*id), "{context}: two layers or the export sit on {id:?}");
			}
		}
		for (id, node) in &registry.node_instances {
			if let Implementation::Network(network) = &node.implementation {
				assert!(registry.networks.contains_key(network), "{context}: {id:?} is built from {network:?}, which is gone");
			}
		}
		let layers = stack(registry);
		if let Some(bottom) = layers.last() {
			assert_ne!(below(registry, *bottom), empty_export(), "{context}: the bottom layer {bottom:?} rests on the empty export's value");
		}
		let unique: HashSet<NodeId> = layers.iter().copied().collect();
		assert_eq!(unique.len(), layers.len(), "{context}: the stack loops: {layers:?}");
		let stray: Vec<&NodeId> = registry.node_instances.keys().filter(|id| !unique.contains(id)).collect();
		assert!(stray.is_empty(), "{context}: layers {stray:?} are in no stack (stack {layers:?})");
	}
}

#[test]
fn history_operations_keep_the_stack_whole_and_peers_converged() {
	let (seeds, steps) = (from_env("SEEDS", 200), from_env("STEPS", 80));
	for seed in 0..seeds {
		simulate::<Stack>(seed, steps, &mut Vec::new());
	}
}

/// A seed fixes the run, so one that fails can be replayed.
#[test]
fn a_seed_fixes_the_history_run() {
	let histories = |seed| simulate::<Stack>(seed, 60, &mut Vec::new()).history().map(|delta| delta.id).collect::<Vec<_>>();
	for seed in 0..5 {
		assert_eq!(histories(seed), histories(seed), "seed {seed}");
	}
}

#[test]
#[ignore]
fn inspect_seed() {
	let mut log = Vec::new();
	let host = simulate::<Stack>(from_env("SEED", 0), from_env("STEPS", 80), &mut log);
	println!("{}\nstack {:?}", log.join("\n"), stack(host.retired_registry()));
}
