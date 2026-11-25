//! Shared harness of the deterministic history simulations: a seeded generator, every history operation the History
//! panel offers, and the checks each run makes after every operation.

use crate::{Network, NetworkId, NodeId, NodeInput, PeerId, ROOT_NETWORK, Registry, RegistryDelta, Rev, Session, UserId, Value};
use std::collections::{HashMap, HashSet};

/// What every node's implementation names, so nodes need no proto-node declarations. The first node drawn brings it,
/// as the editor's first layer brings the subgraph every later one is built from.
pub(super) const LEAF: NetworkId = NetworkId(1);

/// What the export holds while the document is empty, of another type than what a chain's bottom rests on.
pub(super) fn empty_export() -> NodeInput {
	NodeInput::Value {
		value: Value::from(serde_json::json!("empty")),
		exposed: false,
	}
}

/// A small xorshift generator, so a seed fixes the run.
pub(super) struct Random(u64);

impl Random {
	fn new(seed: u64) -> Self {
		Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
	}

	pub(super) fn below(&mut self, bound: usize) -> usize {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		(self.0 % bound as u64) as usize
	}

	pub(super) fn pick<T: Copy>(&mut self, options: &[T]) -> Option<T> {
		(!options.is_empty()).then(|| options[self.below(options.len())])
	}
}

/// A kind of document the simulation edits.
pub(super) trait Scenario {
	/// One interaction's ops for a random edit, as the editor would stage them.
	fn random_edit(random: &mut Random, session: &mut Session) -> Vec<RegistryDelta>;

	/// Panics with `context` when `registry` is not a document this scenario allows.
	fn assert_well_formed(context: &str, registry: &Registry);
}

/// The nodes down the chain off the export, following each one's primary input.
pub(super) fn stack(registry: &Registry) -> Vec<NodeId> {
	let mut nodes = Vec::new();
	let mut below = registry.networks.get(&ROOT_NETWORK).and_then(|network| network.exports.first()).and_then(|slot| slot.target.clone());
	while let Some(NodeInput::Node { id, .. }) = below {
		if nodes.contains(&id) {
			break;
		}
		nodes.push(id);
		below = registry.node_instances.get(&id).and_then(|node| node.inputs.first()).map(|slot| slot.input.clone());
	}
	nodes
}

/// What feeds into `node` from below.
pub(super) fn below(registry: &Registry, node: NodeId) -> NodeInput {
	registry.node_instances[&node].inputs[0].input.clone()
}

pub(super) fn on(id: NodeId) -> NodeInput {
	NodeInput::Node { id, index: 0 }
}

pub(super) fn value(number: usize) -> NodeInput {
	NodeInput::Value {
		value: Value::from(serde_json::json!(number)),
		exposed: false,
	}
}

/// Reads a number from the environment variable `name`, so a run can be widened or replayed.
pub(super) fn from_env<T: std::str::FromStr>(name: &str, default: T) -> T {
	std::env::var(name).ok().and_then(|text| text.parse().ok()).unwrap_or(default)
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

fn assert_consistent<S: Scenario>(context: &str, session: &Session) {
	assert!(session.hot_log().is_empty(), "{context}: everything retires before history changes");
	assert!(session.registry().value_equal(session.retired_registry()), "{context}: working and snapshot differ");
	let folded = session.snapshot_from_history().expect("history folds");
	assert!(folded.value_equal(session.retired_registry()), "{context}: the snapshot is not what history folds to");
	S::assert_well_formed(context, session.retired_registry());
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

/// Runs `steps` random edits and history operations from `seed`. After each one the document must be well formed, the
/// snapshot must be what history folds to, and a peer following the host must hold the same document.
pub(super) fn simulate<S: Scenario>(seed: u64, steps: usize, log: &mut Vec<String>) -> Session {
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
				let ops = S::random_edit(&mut random, &mut host);
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
				let steps = line_steps(&host);
				// Not the newest step, which going back to would leave everything as it is
				let Some(target) = random.pick(&steps[1.min(steps.len())..]) else { continue };
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
		assert_consistent::<S>(&context(&what, log), &host);
		assert_eq!(follower.head_rev(), host.head_rev(), "{}", context("the follower is on another head", log));
		assert!(
			follower.retired_registry().value_equal(host.retired_registry()),
			"{}",
			context("the follower holds another document", log)
		);
	}
	host
}
