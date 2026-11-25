//! Deterministic simulation of a document whose chains branch, with every history operation the History panel offers
//! mixed in: undo, redo, going back, removing a step and bringing one back. The stack simulation next door builds one
//! chain off the export, while `repair` also treats an input past the primary one as heading a chain, so this builds a
//! tree of them. After every operation nothing may name a node the document no longer has, and the snapshot must still
//! be what history folds to and what a peer following along holds.

use crate::{Implementation, Network, NetworkId, Node, NodeId, NodeInput, PeerId, ROOT_NETWORK, Registry, RegistryDelta, Rev, Session, UserId, Value};
use std::collections::{HashMap, HashSet};

/// What every node's implementation names, so nodes need no proto-node declarations. The first node drawn brings it.
const LEAF: NetworkId = NetworkId(1);

/// What the export holds while the document is empty, of another type than what a chain's bottom rests on.
fn empty_export() -> NodeInput {
	NodeInput::Value {
		value: Value::from(serde_json::json!("empty")),
		exposed: false,
	}
}

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

/// Every node hanging off the export, through any input.
fn reachable(registry: &Registry) -> Vec<NodeId> {
	let mut seen: Vec<NodeId> = Vec::new();
	let mut queue: Vec<NodeId> = Vec::new();
	if let Some(NodeInput::Node { id, .. }) = place_value(registry, Place::Export(0)) {
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

/// The nodes down the chain off the export, following each one's primary input.
fn stack(registry: &Registry) -> Vec<NodeId> {
	let mut nodes = Vec::new();
	let mut below = place_value(registry, Place::Export(0));
	while let Some(NodeInput::Node { id, .. }) = below {
		if nodes.contains(&id) || nodes.len() > registry.node_instances.len() {
			break;
		}
		nodes.push(id);
		below = registry.node_instances.get(&id).and_then(|node| node.inputs.first()).map(|slot| slot.input.clone());
	}
	nodes
}

/// What feeds into `node` from below.
fn below(registry: &Registry, node: NodeId) -> NodeInput {
	registry.node_instances[&node].inputs[0].input.clone()
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

/// Every node under `place`.
fn under(registry: &Registry, place: Place) -> Vec<NodeId> {
	let mut gone: Vec<NodeId> = Vec::new();
	let mut queue: Vec<NodeId> = Vec::new();
	if let Some(NodeInput::Node { id, .. }) = place_value(registry, place) {
		queue.push(id);
	}
	while let Some(id) = queue.pop() {
		if gone.contains(&id) || !registry.node_instances.contains_key(&id) {
			continue;
		}
		gone.push(id);
		for slot in &registry.node_instances[&id].inputs {
			if let NodeInput::Node { id: next, .. } = slot.input {
				queue.push(next);
			}
		}
	}
	gone
}

/// One interaction's ops, as the editor would stage them for a random edit of a branching document.
fn random_edit(random: &mut Random, session: &mut Session) -> Vec<RegistryDelta> {
	let registry = session.registry().clone();
	let layers = stack(&registry);
	let nodes = reachable(&registry);
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
			RegistryDelta::AddNetwork { id, .. } => format!("add network {}", id.0),
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

/// The nodes the interaction ending at `end` added and deleted, by what it did in all.
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

/// The nodes the step ending at `end` added that a newer deletion on its own line had taken again.
fn lost_on_its_line(session: &Session, end: Rev, added: &[NodeId]) -> HashSet<NodeId> {
	let mut lost = HashSet::new();
	let mut added_at = HashMap::new();
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

/// The nodes live on the line just before the interaction ending at `end`.
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

/// Nothing names a node the document no longer has, every node is built from a network it still has, and no node input
/// holds the value the export carries while the document is empty. A node on no chain and two slots naming one node
/// are both left alone, since the graph allows a node to feed nothing and to feed more than one input.
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

fn simulate(seed: u64, steps: usize, log: &mut Vec<String>) {
	let mut random = Random::new(seed);
	let mut host = Session::with_identity(PeerId(1), UserId(1));
	let document = [
		RegistryDelta::AddNetwork {
			id: ROOT_NETWORK,
			network: Network::default(),
		},
		RegistryDelta::SetNetworkExport {
			id: ROOT_NETWORK,
			index: 0,
			export: Some(empty_export()),
		},
	];
	host.stage_ops(document).expect("stage the document");
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
				// The first step makes the document, which nothing can do without
				let Some(target) = random.pick(&steps[..steps.len().saturating_sub(1)]) else { continue };
				log.push(format!("{step}: remove {target:?}"));
				let (mut added, _) = added_and_removed(&host, target);
				// A node the line had before the step stays
				let earlier = live_before(&host, target);
				added.retain(|id| !earlier.contains(id));
				let (moved, _) = host.drop_interaction(target).expect("remove the step");
				follower.apply_head_move(&moved).expect("follow the removal");
				let kept: Vec<&NodeId> = added.iter().filter(|id| host.retired_registry().node_instances.contains_key(id)).collect();
				assert!(kept.is_empty(), "{}", context(&format!("the removed step's nodes {kept:?} are still there"), log));
			}
			10 | 11 => {
				let Some(target) = random.pick(&branch_steps(&host)) else { continue };
				log.push(format!("{step}: bring back {target:?}"));
				let (mut added, removed) = added_and_removed(&host, target);
				// A node the line already had from this step and deleted since stays deleted
				added.retain(|id| !host.retired_registry().removed_nodes.contains_key(id));
				// As does one a newer deletion took on the step's own line
				let lost = lost_on_its_line(&host, target, &added);
				added.retain(|id| !lost.contains(id));
				host.restore_interaction(target).expect("bring back");
				catch_up(&mut follower, &host);
				let registry = host.retired_registry();
				let missing: Vec<&NodeId> = added.iter().filter(|id| !registry.node_instances.contains_key(id)).collect();
				let kept: Vec<&NodeId> = removed.iter().filter(|id| registry.node_instances.contains_key(id)).collect();
				assert!(missing.is_empty(), "{}", context(&format!("the brought back step's nodes {missing:?} are missing"), log));
				assert!(kept.is_empty(), "{}", context(&format!("the nodes {kept:?} the brought back step deleted are still there"), log));
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
}

#[test]
fn history_operations_keep_a_branching_document_whole() {
	let seeds: u64 = std::env::var("SEEDS").ok().and_then(|seeds| seeds.parse().ok()).unwrap_or(200);
	let steps = std::env::var("STEPS").ok().and_then(|steps| steps.parse().ok()).unwrap_or(80);
	for seed in 0..seeds {
		simulate(seed, steps, &mut Vec::new());
	}
}
