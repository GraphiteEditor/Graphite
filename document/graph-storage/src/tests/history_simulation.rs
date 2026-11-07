//! Deterministic simulation of a layer stack edited the way the editor does, with every history operation the History
//! panel offers mixed in: undo, redo, going back, removing a step and bringing one back. After every operation the
//! document must still be a well-formed stack, the snapshot must be what history folds to, and a peer following the
//! head moves must hold the same document.
//!
//! Inspect one run with `SEED=<n> cargo test -p document-graph-storage history_simulation::inspect_seed -- --ignored --nocapture`.

use crate::{Implementation, Network, NetworkId, Node, NodeId, NodeInput, PeerId, ROOT_NETWORK, Registry, RegistryDelta, Rev, Session, UserId, Value};
use std::collections::HashSet;

/// What every layer's implementation names, so nodes need no proto-node declarations.
const LEAF: NetworkId = NetworkId(1);

/// A small xorshift generator, so a seed fixes the run.
struct Random(u64);

impl Random {
	fn new(seed: u64) -> Self {
		Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
	}

	fn below(&mut self, bound: usize) -> usize {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		(self.0 % bound as u64) as usize
	}

	fn pick<T: Copy>(&mut self, options: &[T]) -> Option<T> {
		(!options.is_empty()).then(|| options[self.below(options.len())])
	}
}

/// The layers from the top of the stack down, following each layer's first input.
fn stack(registry: &Registry) -> Vec<NodeId> {
	let mut layers = Vec::new();
	let mut below = registry.networks.get(&ROOT_NETWORK).and_then(|network| network.exports.first()).and_then(|slot| slot.target.clone());
	while let Some(NodeInput::Node { id, .. }) = below {
		if layers.contains(&id) || layers.len() > registry.node_instances.len() {
			break;
		}
		layers.push(id);
		below = registry.node_instances.get(&id).and_then(|node| node.inputs.first()).map(|slot| slot.input.clone());
	}
	layers
}

/// What feeds into `layer` from below.
fn below(registry: &Registry, layer: NodeId) -> NodeInput {
	registry.node_instances[&layer].inputs[0].input.clone()
}

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

fn on(id: NodeId) -> NodeInput {
	NodeInput::Node { id, index: 0 }
}

fn value(number: usize) -> NodeInput {
	NodeInput::Value {
		value: Value::from(serde_json::json!(number)),
		exposed: false,
	}
}

/// One interaction's ops, as the editor would stage them for a random edit of the stack.
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
			let mut ops = vec![
				RegistryDelta::AddNode { id, node },
				RegistryDelta::SetNetworkExport {
					id: ROOT_NETWORK,
					index: 0,
					export: Some(on(id)),
				},
			];
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

/// The ops of an edit, short enough to read in a failing run's log.
fn summarize(ops: &[RegistryDelta]) -> String {
	let short = |id: &NodeId| id.0 % 1000;
	let input = |input: &NodeInput| match input {
		NodeInput::Node { id, .. } => format!("{}", short(id)),
		_ => "value".to_string(),
	};
	ops.iter()
		.map(|op| match op {
			RegistryDelta::AddNode { id, node } => format!("add {} on {}", short(id), input(&node.inputs[0].input)),
			RegistryDelta::RemoveNode { id, .. } => format!("remove {}", short(id)),
			RegistryDelta::ChangeNodeInput { id, index, new_input } => format!("{}.{index} = {}", short(id), input(new_input)),
			RegistryDelta::SetNetworkExport { export, .. } => format!("export = {}", export.as_ref().map_or("none".to_string(), input)),
			other => format!("{other:?}"),
		})
		.collect::<Vec<_>>()
		.join(", ")
}

/// The revs closing each interaction on the head's line, newest first.
fn line_steps(session: &Session) -> Vec<Rev> {
	let mut steps = Vec::new();
	let mut current = session.head_rev();
	while let Some(delta) = current.and_then(|rev| session.delta(rev)) {
		if delta.is_interaction_end() {
			steps.push(delta.id);
		}
		current = delta.parent;
	}
	steps
}

/// The revs closing interactions left behind on branches.
fn branch_steps(session: &Session) -> Vec<Rev> {
	let line: HashSet<Rev> = {
		let mut line = HashSet::new();
		let mut current = session.head_rev();
		while let Some(delta) = current.and_then(|rev| session.delta(rev)) {
			line.insert(delta.id);
			current = delta.parent;
		}
		line
	};
	let mut steps: Vec<Rev> = session
		.history()
		.filter(|delta| delta.is_interaction_end() && !line.contains(&delta.id))
		.map(|delta| delta.id)
		.collect();
	steps.sort();
	steps
}

/// The layers the interaction ending at `end` added and deleted, by what it did in all.
fn added_and_removed(session: &Session, end: Rev) -> (Vec<NodeId>, Vec<NodeId>) {
	let (mut added, mut removed) = (Vec::new(), Vec::new());
	let mut current = session.delta(end);
	while let Some(delta) = current {
		match &delta.kind {
			RegistryDelta::AddNode { id, .. } => added.push(*id),
			RegistryDelta::RemoveNode { id, .. } => removed.push(*id),
			_ => {}
		}
		current = delta.parent.and_then(|parent| session.delta(parent)).filter(|parent| !parent.is_interaction_end());
	}
	let net = |these: &[NodeId], those: &[NodeId]| these.iter().filter(|id| !those.contains(id)).copied().collect();
	(net(&added, &removed), net(&removed, &added))
}

/// The layers the step ending at `end` added that a newer deletion on its own line had taken again.
fn lost_on_its_line(session: &Session, end: Rev, added: &[NodeId]) -> HashSet<NodeId> {
	let mut lost = HashSet::new();
	let mut added_at = std::collections::HashMap::new();
	let mut line = Vec::new();
	let mut current = session.delta(end);
	while let Some(delta) = current {
		line.push(delta);
		current = delta.parent.and_then(|parent| session.delta(parent));
	}
	for delta in &line {
		if let RegistryDelta::AddNode { id, .. } = &delta.kind {
			added_at.entry(*id).or_insert(delta.timestamp);
		}
	}
	for delta in &line {
		if let RegistryDelta::RemoveNode { id, .. } = &delta.kind
			&& added.contains(id)
			&& added_at.get(id).is_some_and(|stamp| *stamp < delta.timestamp)
		{
			lost.insert(*id);
		}
	}
	lost
}

/// The layers live on the line just before the interaction ending at `end`.
fn live_before(session: &Session, end: Rev) -> HashSet<NodeId> {
	let mut current = session.delta(end);
	while let Some(delta) = current.filter(|delta| delta.id == end || !delta.is_interaction_end()) {
		current = delta.parent.and_then(|parent| session.delta(parent));
	}
	let mut line = Vec::new();
	while let Some(delta) = current {
		line.push(delta.clone());
		current = delta.parent.and_then(|parent| session.delta(parent));
	}
	line.reverse();
	let replayed = Session::replay_from_history(PeerId(3), UserId(3), line, 0).expect("the line replays");
	replayed.retired_registry().node_instances.keys().copied().collect()
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
	let layers = stack(registry);
	let unique: HashSet<NodeId> = layers.iter().copied().collect();
	assert_eq!(unique.len(), layers.len(), "{context}: the stack loops: {layers:?}");
	let stray: Vec<&NodeId> = registry.node_instances.keys().filter(|id| !unique.contains(id)).collect();
	assert!(stray.is_empty(), "{context}: layers {stray:?} are in no stack (stack {layers:?})");
}

fn assert_consistent(context: &str, session: &Session) {
	assert!(session.hot_log().is_empty(), "{context}: everything retires before history changes");
	assert!(session.registry().value_equal(session.retired_registry()), "{context}: working and snapshot differ");
	let folded = session.snapshot_from_history().expect("history folds");
	assert!(folded.value_equal(session.retired_registry()), "{context}: the snapshot is not what history folds to");
	assert_well_formed(context, session.retired_registry());
}

/// Closes the open interaction and retires it as one step.
fn retire_step(session: &mut Session) {
	session.end_interaction().expect("close");
	for interaction in session.closed_interactions() {
		session.retire_interaction(&interaction).expect("retire");
	}
}

/// Brings `follower` up to the host's history and head, as the room does with what the host broadcasts.
fn catch_up(follower: &mut Session, host: &Session) {
	let missing: Vec<_> = host.cloned_deltas().into_iter().filter(|delta| follower.delta(delta.id).is_none()).collect();
	follower.follow(missing, host.head_rev()).expect("follow");
}

/// A peer that joins now and holds what the host holds.
fn fresh_follower(host: &Session) -> Session {
	Session::load(PeerId(2), UserId(2), host.retired_registry().clone(), host.cloned_deltas(), host.head_rev(), Vec::new(), 0)
}

fn simulate(seed: u64, steps: usize, log: &mut Vec<String>) -> Session {
	let mut random = Random::new(seed);
	let mut host = Session::with_identity(PeerId(1), UserId(1));
	let networks = [
		RegistryDelta::AddNetwork {
			id: ROOT_NETWORK,
			network: Network::default(),
		},
		RegistryDelta::AddNetwork {
			id: LEAF,
			network: Network::default(),
		},
	];
	host.stage_ops(networks).expect("stage the networks");
	retire_step(&mut host);
	let mut follower = fresh_follower(&host);

	for step in 0..steps {
		let context = |what: &str, log: &[String]| format!("seed {seed}, step {step}, {what}\n{}", log.join("\n"));
		match random.below(12) {
			0..=5 => {
				let ops = random_edit(&mut random, &mut host);
				if ops.is_empty() {
					continue;
				}
				log.push(format!("{step}: edit {}", summarize(&ops)));
				host.stage_ops(ops).expect("stage");
				retire_step(&mut host);
				catch_up(&mut follower, &host);
			}
			6 if host.can_undo() => {
				log.push(format!("{step}: undo"));
				host.undo().expect("undo");
				follower = fresh_follower(&host);
			}
			7 if host.can_redo() => {
				log.push(format!("{step}: redo"));
				host.redo().expect("redo");
				follower = fresh_follower(&host);
			}
			8 => {
				// Not the newest step, which going back to would leave everything as it is
				let Some(target) = random.pick(&line_steps(&host)[1.min(line_steps(&host).len())..]) else {
					continue;
				};
				log.push(format!("{step}: go back to {target:?}"));
				let (moved, _) = host.move_head_to(target).expect("go back");
				follower.apply_head_move(&moved).expect("follow the move");
			}
			9 => {
				let steps = line_steps(&host);
				// The first step adds the networks, which nothing can do without
				let Some(target) = random.pick(&steps[..steps.len().saturating_sub(1)]) else { continue };
				log.push(format!("{step}: remove {target:?}"));
				let (mut added, _) = added_and_removed(&host, target);
				// A layer the line had before the step stays
				let earlier = live_before(&host, target);
				added.retain(|id| !earlier.contains(id));
				let (moved, _) = host.drop_interaction(target).expect("remove the step");
				follower.apply_head_move(&moved).expect("follow the removal");
				let kept: Vec<&NodeId> = added.iter().filter(|id| host.retired_registry().node_instances.contains_key(id)).collect();
				assert!(kept.is_empty(), "{}", context(&format!("the removed step's layers {kept:?} are still there"), log));
			}
			10 | 11 => {
				let Some(target) = random.pick(&branch_steps(&host)) else { continue };
				log.push(format!("{step}: bring back {target:?}"));
				let (mut added, removed) = added_and_removed(&host, target);
				// A layer the line already had from this step and deleted since stays deleted
				added.retain(|id| !host.retired_registry().removed_nodes.contains_key(id));
				// As does one a newer deletion took on the step's own line
				let lost = lost_on_its_line(&host, target, &added);
				added.retain(|id| !lost.contains(id));
				host.restore_interaction(target).expect("bring back");
				catch_up(&mut follower, &host);
				let registry = host.retired_registry();
				let missing: Vec<&NodeId> = added.iter().filter(|id| !registry.node_instances.contains_key(id)).collect();
				let kept: Vec<&NodeId> = removed.iter().filter(|id| registry.node_instances.contains_key(id)).collect();
				assert!(missing.is_empty(), "{}", context(&format!("the brought back step's layers {missing:?} are missing"), log));
				assert!(kept.is_empty(), "{}", context(&format!("the layers {kept:?} the brought back step deleted are still there"), log));
			}
			_ => continue,
		}
		let what = log.last().cloned().unwrap_or_default();
		assert_consistent(&context(&what, log), &host);
		assert_eq!(follower.head_rev(), host.head_rev(), "{}", context("the follower is on another head", log));
		assert!(
			follower.retired_registry().value_equal(host.retired_registry()),
			"{}",
			context("the follower holds another document", log)
		);
	}
	host
}

#[test]
fn history_operations_keep_the_stack_whole_and_peers_converged() {
	let seeds: u64 = std::env::var("SEEDS").ok().and_then(|seeds| seeds.parse().ok()).unwrap_or(200);
	let steps = std::env::var("STEPS").ok().and_then(|steps| steps.parse().ok()).unwrap_or(80);
	for seed in 0..seeds {
		simulate(seed, steps, &mut Vec::new());
	}
}

/// A seed fixes the run, so one that fails can be replayed.
#[test]
fn a_seed_fixes_the_history_run() {
	let stacks = |seed| stack(simulate(seed, 60, &mut Vec::new()).retired_registry());
	for seed in 0..5 {
		assert_eq!(stacks(seed), stacks(seed), "seed {seed}");
	}
}

#[test]
#[ignore]
fn inspect_seed() {
	let seed = std::env::var("SEED").ok().and_then(|seed| seed.parse().ok()).unwrap_or(0);
	let mut log = Vec::new();
	let steps = std::env::var("STEPS").ok().and_then(|steps| steps.parse().ok()).unwrap_or(80);
	let host = simulate(seed, steps, &mut log);
	println!("{}\nstack {:?}", log.join("\n"), stack(host.retired_registry()));
}
