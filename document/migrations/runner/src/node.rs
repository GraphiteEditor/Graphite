//! The node tier: `M` as a function on registry states, and the history rewrite derived from it.

use crate::RunnerError;
use document_graph_storage::delta::compute_deltas;
use document_graph_storage::{Delta, Implementation, Node, NodeId, Registry, RegistryDelta, ResourceHash, ResourceId, Rev, apply_retired_op};
use migration_core::{AnchorContext, Declaration, DeclarationResolver, Edits, InputConnector, InputsEdit, NodeMigration, OutputConnector};
use std::collections::{HashMap, HashSet};

/// A [`DeclarationResolver`] over a fixed set of declarations, whose resource IDs derive from `identifier@version`.
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
}

/// `M`: run `migration` on every anchor in `source` and apply the edits. Every run reads `source`, so no run observes another's edits.
pub fn apply_migration(migration: &dyn NodeMigration, source: &Registry, resolver: &dyn DeclarationResolver) -> Result<Registry, RunnerError> {
	let mut target = source.clone();

	let mut runs = Vec::new();
	for anchor in anchors(migration.source(), source, resolver) {
		let Some(context) = AnchorContext::new(source, anchor, migration.source(), migration.target(), resolver) else {
			continue;
		};
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

	let target_implementation = Implementation::ProtoNode(resolver.resource(migration.target()));
	for (anchor, _) in &runs {
		if let Some(node) = target.node_instances.get_mut(anchor) {
			*node.implementation_mut() = target_implementation.clone();
		}
	}

	Ok(target)
}

/// The live nodes of `declaration`, in ID order.
fn anchors(declaration: Declaration, registry: &Registry, resolver: &dyn DeclarationResolver) -> Vec<NodeId> {
	let mut anchors: Vec<NodeId> = registry
		.node_instances
		.iter()
		.filter(|(_, node)| matches!(node.implementation(), Implementation::ProtoNode(resource) if resolver.declaration(*resource) == Some(declaration)))
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

/// Rewrite a linear `history` so replaying it yields `M` of the original at every step: `rewritten(d) = diff(R(d), M(O(d) + d))`.
///
/// Each delta costs one full evaluation of `M` and one full diff, so this is O(history × registry). The incremental, footprint-scoped
/// evaluation the RFC describes comes later.
pub fn rewrite_history(migration: &dyn NodeMigration, history: &[Delta], resolver: &dyn DeclarationResolver) -> Result<RewrittenHistory, RunnerError> {
	let mut original = Registry::default();
	let mut rewritten = Registry::default();

	let mut deltas: Vec<Delta> = Vec::with_capacity(history.len());
	let mut revs = HashMap::with_capacity(history.len());
	let mut previous_original = None;
	let mut tip = None;

	for delta in history {
		if delta.parent != previous_original || matches!(delta.kind, RegistryDelta::Merge { .. }) {
			return Err(RunnerError::NonLinearHistory(delta.id));
		}
		previous_original = Some(delta.id);

		apply_retired_op(&mut original, delta.kind.clone(), delta.timestamp)?;

		let chain = if passes_through(&delta.kind) {
			vec![delta.kind.clone()]
		} else {
			let migrated = apply_migration(migration, &original, resolver)?;
			compute_deltas(&rewritten, &migrated)
		};
		check_single_writes(&chain)?;

		// Every op of the chain shares the original's timestamp, which orders it after everything the original saw
		let chain_start = deltas.len();
		for op in chain {
			let reverse = apply_retired_op(&mut rewritten, op.clone(), delta.timestamp)?;
			let rewritten_delta = Delta::new(tip, delta.author, delta.timestamp, op, reverse);
			tip = Some(rewritten_delta.id);
			deltas.push(rewritten_delta);
		}

		// A dropped delta's interaction end moves to the rewritten delta before it, so undo keeps its checkpoint
		match deltas[chain_start..].last_mut() {
			Some(last) => last.carry_annotations(delta),
			None if delta.is_interaction_end() => {
				if let Some(previous) = deltas.last_mut() {
					previous.mark_interaction_end(delta.timestamp);
				}
			}
			None => {}
		}

		revs.insert(delta.id, tip);
	}

	Ok(RewrittenHistory { deltas, revs })
}

/// Ops `M` can't affect and `compute_deltas` doesn't diff, which therefore keep their original form.
fn passes_through(kind: &RegistryDelta) -> bool {
	matches!(kind, RegistryDelta::RegisterPeer { .. } | RegistryDelta::Other(_))
}

/// Ops of one chain share a timestamp, and last-writer-wins needs a newer stamp to overwrite, so no chain may write one entity twice.
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
