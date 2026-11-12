//! The node tier: `M` as a function on registry states, and the history rewrite derived from it.

use crate::{MigrationBlobs, RunnerError};
use document_graph_storage::TimeStamp;
use document_graph_storage::attributes::attr::node::ui::REFERENCE;
use document_graph_storage::delta::compute_deltas;
use document_graph_storage::{Delta, Implementation, NetworkId, Node, NodeId, NodeInput, ProtoNode, Registry, RegistryDelta, ResourceHash, ResourceId, Rev, apply_retired_op};
use migration_core::{AnchorContext, Compiler, Declaration, DeclarationResolver, Edits, InputConnector, InputsEdit, MigrationHost, NodeMigration, OutputConnector, implements};
use std::collections::{HashMap, HashSet};

/// A [`DeclarationResolver`] over a fixed set of declarations for tests, whose resource IDs derive from `identifier@version` rather than the encoded declarations documents store.
#[derive(Clone, Debug, Default)]
pub struct DeclarationTable {
	declarations: HashMap<ResourceId, Declaration>,
}

impl DeclarationTable {
	pub fn new(declarations: impl IntoIterator<Item = Declaration>) -> Self {
		let mut table = Self::default();
		for declaration in declarations {
			table.register(declaration);
		}
		table
	}

	/// Register every declaration `migration` reads or writes.
	pub fn register_migration(&mut self, migration: &dyn NodeMigration) {
		self.register(migration.source());
		self.register(migration.target());
		for pin in migration.pins() {
			self.register(*pin);
		}
	}

	pub fn register(&mut self, declaration: Declaration) -> ResourceId {
		let resource = Self::resource_id(declaration);
		self.declarations.insert(resource, declaration);
		resource
	}

	pub fn resource_id(declaration: Declaration) -> ResourceId {
		ResourceId::from_hash(&ResourceHash::from(declaration.to_string().as_bytes()))
	}
}

impl DeclarationResolver for DeclarationTable {
	fn declaration(&self, resource: ResourceId) -> Option<Declaration> {
		self.declarations.get(&resource).copied()
	}

	fn resource(&self, declaration: Declaration) -> ResourceId {
		Self::resource_id(declaration)
	}

	fn proto(&self, resource: ResourceId) -> Option<ProtoNode> {
		let declaration = self.declaration(resource)?;
		Some(ProtoNode {
			identifier: declaration.identifier.to_string(),
			version: declaration.version,
			attributes: Default::default(),
		})
	}
}

/// `M`: run `migration` on every anchor in `source` and apply the edits. Every run reads `source`, so no run observes another's edits.
pub(crate) fn apply_migration(
	migration: &dyn NodeMigration,
	source: &Registry,
	resolver: &dyn DeclarationResolver,
	host: Option<&dyn MigrationHost>,
	blobs: &mut MigrationBlobs,
) -> Result<Registry, RunnerError> {
	let anchors = anchors(migration.source(), source, resolver);
	if anchors.is_empty() {
		return Ok(source.clone());
	}

	// A migration that reads beyond its anchors needs every value decoded
	let reads_beyond_anchors = migration.footprint().reads_network() || migration.asks_compiler();
	let resolved = blobs.resolve(source, (!reads_beyond_anchors).then_some(anchors.as_slice()))?;

	let mut migrated = apply_to_values(migration, &resolved, anchors, resolver, host)?;
	blobs.store(&mut migrated, source);
	Ok(migrated)
}

/// A migration asking the compiler runs upstream anchors first, in rounds, since the compiler can't describe what's still in the old shape.
fn apply_to_values(migration: &dyn NodeMigration, source: &Registry, anchors: Vec<NodeId>, resolver: &dyn DeclarationResolver, host: Option<&dyn MigrationHost>) -> Result<Registry, RunnerError> {
	let Some(host) = host.filter(|_| migration.asks_compiler()) else {
		return apply_runs(migration, source, &anchors, resolver, None);
	};

	let mut state = source.clone();
	let mut remaining = anchors;
	while !remaining.is_empty() {
		let ready = upstream_most(&state, &remaining);
		let compiler = host.compiler(&state, resolver);
		state = apply_runs(migration, &state, &ready, resolver, compiler.as_deref())?;
		remaining.retain(|anchor| !ready.contains(anchor));
	}
	Ok(state)
}

/// The `anchors` with no other of them upstream, or all of them if they form a cycle. O(registry) per anchor.
fn upstream_most(registry: &Registry, anchors: &[NodeId]) -> Vec<NodeId> {
	let mut callers: HashMap<NetworkId, Vec<&Node>> = HashMap::new();
	for node in registry.node_instances.values() {
		if let Implementation::Network(network) = node.implementation() {
			callers.entry(*network).or_default().push(node);
		}
	}

	let others: HashSet<NodeId> = anchors.iter().copied().collect();
	let reads_another = |anchor: NodeId| {
		let mut visited = HashSet::from([anchor]);
		let mut pending = vec![anchor];

		while let Some(id) = pending.pop() {
			let Some(node) = registry.node_instances.get(&id) else { continue };
			let mut read = |input: &NodeInput| {
				if let NodeInput::Node { id, .. } = input
					&& visited.insert(*id)
				{
					pending.push(*id);
				}
			};

			for slot in node.inputs() {
				read(&slot.input);
				if let NodeInput::Import { index } = slot.input {
					for caller in callers.get(&node.network()).into_iter().flatten() {
						if let Some(slot) = caller.inputs().get(index as usize) {
							read(&slot.input);
						}
					}
				}
			}
			if let Implementation::Network(network) = node.implementation()
				&& let Some(network) = registry.networks.get(network)
			{
				for target in network.exports.iter().filter_map(|export| export.target.as_ref()) {
					read(target);
				}
			}
		}

		visited.iter().any(|id| *id != anchor && others.contains(id))
	};

	let ready: Vec<NodeId> = anchors.iter().copied().filter(|anchor| !reads_another(*anchor)).collect();
	if ready.is_empty() { anchors.to_vec() } else { ready }
}

/// Run `migration` on `anchors` and apply the edits, with every run reading `source`.
fn apply_runs(migration: &dyn NodeMigration, source: &Registry, anchors: &[NodeId], resolver: &dyn DeclarationResolver, compiler: Option<&dyn Compiler>) -> Result<Registry, RunnerError> {
	let mut target = source.clone();

	let mut runs = Vec::new();
	for anchor in anchors.iter().copied() {
		let Some(context) = AnchorContext::new(source, anchor, migration.source(), migration.target(), resolver) else {
			continue;
		};
		let context = context.with_compiler(compiler);
		let edits = migration.run(&context).map_err(|error| RunnerError::Migration {
			migration: migration.source(),
			anchor,
			error,
		})?;
		runs.push((anchor, edits));
	}
	if runs.is_empty() {
		return Ok(target);
	}

	add_nodes(&mut target, &runs, resolver)?;

	let input_edits: HashMap<NodeId, &InputsEdit> = runs.iter().filter_map(|(anchor, edits)| Some((*anchor, edits.inputs.as_ref()?))).collect();
	for (anchor, inputs) in &input_edits {
		let Some(node) = target.node_instances.get_mut(anchor) else { continue };
		*node.inputs_mut() = inputs.slots.clone();
	}

	for (_, edits) in &runs {
		for (output, consumer) in &edits.connections {
			connect(&mut target, *output, *consumer, &input_edits)?;
		}
	}

	for (_, edits) in &runs {
		for removed in &edits.removed_nodes {
			target.node_instances.remove(removed);
		}
	}

	let mut used_declarations = vec![migration.target()];
	for (anchor, edits) in &runs {
		let chosen = edits.target.unwrap_or(migration.target());
		if chosen != migration.target() && !migration.pins().contains(&chosen) {
			return Err(RunnerError::UnnamedTarget {
				migration: migration.source(),
				anchor: *anchor,
				target: chosen,
			});
		}

		switch_implementation(&mut target, *anchor, chosen, resolver).map_err(|error| error.for_anchor(migration.source(), *anchor))?;
		used_declarations.push(chosen);
		used_declarations.extend(edits.added_nodes.iter().map(|added| added.node.declaration));
	}
	for declaration in used_declarations.into_iter().filter(|declaration| !declaration.is_network()) {
		if let Some(entry) = resolver.resource_entry(declaration) {
			target.resources.entry(resolver.resource(declaration)).or_insert(entry);
		}
	}

	Ok(target)
}

/// Why an anchor can't switch to the declaration its run chose.
enum SwitchError {
	/// Only a network anchor can become a network definition, since a proto node has no network to name it.
	ProtoToNetwork(Declaration),
}

impl SwitchError {
	fn for_anchor(self, migration: Declaration, anchor: NodeId) -> RunnerError {
		match self {
			Self::ProtoToNetwork(target) => RunnerError::ProtoToNetwork { migration, anchor, target },
		}
	}
}

/// Switch `anchor` to `declaration`. A network anchor becoming a proto node drops the network it implemented, with every network nested in it.
fn switch_implementation(registry: &mut Registry, anchor: NodeId, declaration: Declaration, resolver: &dyn DeclarationResolver) -> Result<(), SwitchError> {
	let Some(implementation) = registry.node_instances.get(&anchor).map(|node| node.implementation().clone()) else {
		return Ok(());
	};

	match (implementation, declaration.is_network()) {
		(Implementation::ProtoNode(_), true) => Err(SwitchError::ProtoToNetwork(declaration)),
		(Implementation::Network(network), true) => {
			if let Some(network) = registry.networks.get_mut(&network) {
				network.attributes.set(REFERENCE, declaration.network_reference().to_value(), TimeStamp::ORIGIN);
			}
			Ok(())
		}
		(implementation, false) => {
			if let Implementation::Network(network) = implementation {
				remove_network_tree(registry, network);
			}
			if let Some(node) = registry.node_instances.get_mut(&anchor) {
				*node.implementation_mut() = Implementation::ProtoNode(resolver.resource(declaration));
			}
			Ok(())
		}
	}
}

/// Remove `network`, its nodes, and every network one of them implements.
fn remove_network_tree(registry: &mut Registry, network: NetworkId) {
	let mut pending = vec![network];

	while let Some(network) = pending.pop() {
		registry.networks.remove(&network);

		registry.node_instances.retain(|_, node| {
			if node.network() != network {
				return true;
			}
			if let Implementation::Network(nested) = node.implementation() {
				pending.push(*nested);
			}
			false
		});
	}
}

/// Whether a live node of `registry` implements `declaration`.
pub fn holds(registry: &Registry, declaration: Declaration, resolver: &dyn DeclarationResolver) -> bool {
	!anchors(declaration, registry, resolver).is_empty()
}

/// The live nodes of `declaration`, in ID order.
fn anchors(declaration: Declaration, registry: &Registry, resolver: &dyn DeclarationResolver) -> Vec<NodeId> {
	let mut anchors: Vec<NodeId> = registry
		.node_instances
		.iter()
		.filter(|(_, node)| implements(registry, node, declaration, resolver))
		.map(|(id, _)| *id)
		.collect();
	anchors.sort_unstable();
	anchors
}

fn add_nodes(target: &mut Registry, runs: &[(NodeId, Edits)], resolver: &dyn DeclarationResolver) -> Result<(), RunnerError> {
	let mut added_ids = HashSet::new();

	for (anchor, edits) in runs {
		let Some(network) = target.node_instances.get(anchor).map(Node::network) else { continue };

		for added in &edits.added_nodes {
			if !added_ids.insert(added.id) {
				return Err(RunnerError::DuplicateAddedNode {
					anchor: *anchor,
					declaration: added.node.declaration,
					key: added.key.clone(),
				});
			}
			if target.node_instances.contains_key(&added.id) {
				return Err(RunnerError::AddedNodeExists(added.id));
			}

			let implementation = Implementation::ProtoNode(resolver.resource(added.node.declaration));
			let mut node = Node::new(network, implementation, added.node.inputs.len());
			for (slot, input) in node.inputs_mut().iter_mut().zip(&added.node.inputs) {
				slot.input = input.clone();
			}
			target.node_instances.insert(added.id, node);
		}
	}

	Ok(())
}

/// Make `consumer`, named in the source layout, read `output` in the target layout. Fails when the consumer's own migration replaced or
/// removed that slot, since two runs then disagree on what it should read.
fn connect(target: &mut Registry, output: OutputConnector, consumer: InputConnector, input_edits: &HashMap<NodeId, &InputsEdit>) -> Result<(), RunnerError> {
	match consumer {
		InputConnector::Node { node, input } => {
			let index = match input_edits.get(&node) {
				Some(inputs) => inputs.target_index(input).ok_or(RunnerError::ConflictingSlot(consumer))?,
				None => input,
			};
			let slot = target.node_instances.get_mut(&node).and_then(|node| node.inputs_mut().get_mut(index as usize));
			let slot = slot.ok_or(RunnerError::MissingConsumer(consumer))?;
			slot.input = output.into();
		}
		InputConnector::Export { network, index } => {
			let slot = target.networks.get_mut(&network).and_then(|network| network.exports.get_mut(index as usize));
			let slot = slot.ok_or(RunnerError::MissingConsumer(consumer))?;
			slot.target = Some(output.into());
		}
	}

	Ok(())
}

/// A history rewritten into the shape `M` produces, with where each original delta went.
pub struct RewrittenHistory {
	pub deltas: Vec<Delta>,
	/// Each original `Rev` mapped to the last delta of its rewritten chain, or to its parent's mapping when the chain is empty.
	pub revs: HashMap<Rev, Option<Rev>>,
}

/// The original and rewritten states after one delta, which its children continue from.
#[derive(Clone)]
struct Branch {
	original: Registry,
	rewritten: Registry,
	tip: Option<Rev>,
}

/// Rewrite `history` so replaying any delta's ancestors yields `M` of the original at that step: `rewritten(d) = diff(R(parent), M(O(parent) + d))`.
/// Undoing and then editing leaves an abandoned branch, which rewrites like any other since each delta only depends on its ancestors.
/// Fails on a delta that merges branches, or whose parent isn't in `history`.
///
/// A migration reading the graph, asking the compiler, or starting from a network definition needs whole states, so it only runs at the boundaries undo can land on, the end of an interaction,
/// and rewrites everything since the last one as a single chain. Each delta inside maps to the boundary before it, and `kept` names further states to keep.
///
/// Each delta costs one full evaluation of `M` and one full diff, so this is O(history × registry), until the RFC's footprint-scoped
/// evaluation lands. A state is only copied where history branches.
pub(crate) fn rewrite_history(
	migration: &dyn NodeMigration,
	history: &[Delta],
	resolver: &dyn DeclarationResolver,
	host: Option<&dyn MigrationHost>,
	kept: &HashSet<Rev>,
	blobs: &mut MigrationBlobs,
) -> Result<RewrittenHistory, RunnerError> {
	let mut children: HashMap<Option<Rev>, usize> = HashMap::new();
	let mut child_authors: HashMap<Rev, HashSet<_>> = HashMap::new();
	for delta in history {
		*children.entry(delta.parent).or_default() += 1;
		if let Some(parent) = delta.parent {
			child_authors.entry(parent).or_default().insert(delta.author);
		}
	}

	let child_counts = children.clone();
	let whole_states_only = needs_whole_states(migration);
	let is_boundary = |delta: &Delta| {
		let continues_alone = child_counts.get(&Some(delta.id)) == Some(&1) && child_authors.get(&delta.id).is_some_and(|authors| authors.contains(&delta.author));
		delta.is_interaction_end() || !continues_alone || kept.contains(&delta.id)
	};

	let mut branches = HashMap::from([(
		None,
		Branch {
			original: Registry::default(),
			rewritten: Registry::default(),
			tip: None,
		},
	)]);
	let mut deltas: Vec<Delta> = Vec::with_capacity(history.len());
	let mut indices: HashMap<Rev, usize> = HashMap::with_capacity(history.len());
	let mut revs = HashMap::with_capacity(history.len());

	for delta in history {
		if matches!(delta.kind, RegistryDelta::Merge { .. }) {
			return Err(RunnerError::NonLinearHistory(delta.id));
		}

		// The last child takes the parent's state, and every earlier one a copy
		let siblings_left = children.get_mut(&delta.parent).ok_or(RunnerError::NonLinearHistory(delta.id))?;
		*siblings_left -= 1;
		let branch = if *siblings_left == 0 {
			branches.remove(&delta.parent)
		} else {
			branches.get(&delta.parent).cloned()
		};
		let mut branch = branch.ok_or(RunnerError::NonLinearHistory(delta.id))?;

		apply_retired_op(&mut branch.original, delta.kind.clone(), delta.timestamp)?;

		let passes = passes_through(&delta.kind);
		let mut chain = if passes { vec![delta.kind.clone()] } else { Vec::new() };
		if if whole_states_only { is_boundary(delta) } else { !passes } {
			let migrated = apply_migration(migration, &branch.original, resolver, host, blobs)?;
			chain.extend(compute_deltas(&branch.rewritten, &migrated));
		}
		check_single_writes(&chain)?;

		// Every op of the chain shares the original's timestamp, which orders it after everything the original saw
		let chain_start = deltas.len();
		for op in chain {
			let reverse = apply_retired_op(&mut branch.rewritten, op.clone(), delta.timestamp)?;
			let rewritten_delta = Delta::new(branch.tip, delta.author, delta.timestamp, op, reverse);
			branch.tip = Some(rewritten_delta.id);
			indices.insert(rewritten_delta.id, deltas.len());
			deltas.push(rewritten_delta);
		}

		// A dropped delta's interaction end moves to the rewritten delta before it, so undo keeps its checkpoint
		match deltas[chain_start..].last_mut() {
			Some(last) => last.carry_annotations(delta),
			None if delta.is_interaction_end() => {
				if let Some(previous) = branch.tip.and_then(|tip| indices.get(&tip).copied()) {
					deltas[previous].mark_interaction_end(delta.timestamp);
				}
			}
			None => {}
		}

		revs.insert(delta.id, branch.tip);
		if children.contains_key(&Some(delta.id)) {
			branches.insert(Some(delta.id), branch);
		}
	}

	Ok(RewrittenHistory { deltas, revs })
}

/// The registry after `head` and its ancestors in `deltas`, leaving out abandoned branches. Fails on a delta that merges branches.
pub fn registry_at(deltas: &[Delta], head: Option<Rev>) -> Result<Registry, RunnerError> {
	let by_rev: HashMap<Rev, &Delta> = deltas.iter().map(|delta| (delta.id, delta)).collect();

	let mut path = Vec::new();
	let mut cursor = head;
	while let Some(rev) = cursor {
		let Some(delta) = by_rev.get(&rev) else {
			return Err(RunnerError::NonLinearHistory(rev));
		};
		if matches!(delta.kind, RegistryDelta::Merge { .. }) {
			return Err(RunnerError::NonLinearHistory(rev));
		}
		path.push(*delta);
		cursor = delta.parent;
	}

	let mut registry = Registry::default();
	for delta in path.into_iter().rev() {
		apply_retired_op(&mut registry, delta.kind.clone(), delta.timestamp)?;
	}
	Ok(registry)
}

/// A network definition's instance is a node and the network it implements, which a history adds in separate deltas. In between, the network
/// names the definition without an anchor to migrate it, so only states where both or neither exist are migrated.
pub(crate) fn needs_whole_states(migration: &dyn NodeMigration) -> bool {
	migration.footprint().reads_network() || migration.source().is_network()
}

/// Ops `M` can't affect and `compute_deltas` doesn't diff, which therefore keep their original form.
fn passes_through(kind: &RegistryDelta) -> bool {
	matches!(kind, RegistryDelta::RegisterPeer { .. } | RegistryDelta::Other(_))
}

/// Ops of one chain share a timestamp, and last-writer-wins needs a newer stamp to overwrite, so no chain may remove and re-add one node.
fn check_single_writes(chain: &[RegistryDelta]) -> Result<(), RunnerError> {
	let mut removed = HashSet::new();
	let mut added = HashSet::new();

	for op in chain {
		match op {
			RegistryDelta::RemoveNode { id, .. } => removed.insert(*id),
			RegistryDelta::AddNode { id, .. } => added.insert(*id),
			_ => continue,
		};
	}

	match removed.intersection(&added).next() {
		Some(id) => Err(RunnerError::ChainWritesTwice(*id)),
		None => Ok(()),
	}
}
