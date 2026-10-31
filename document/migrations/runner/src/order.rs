//! Orders registered node migrations by the declarations they read and write, and runs every pending one over a history.

use crate::node::{apply_migration, rewrite_history};
use crate::{RewrittenHistory, RunnerError};
use document_graph_storage::{Delta, Implementation, Node, Registry, RegistryDelta, Rev};
use migration_core::{Declaration, DeclarationResolver, NodeMigration};
use std::collections::{BTreeSet, HashMap};

/// Order `migrations` so each runs after the one producing its source and before the one consuming its target, and pinned
/// declarations exist in exactly the version the body names. Unrelated migrations order by source declaration, so the result
/// doesn't depend on registration order. Kahn's algorithm, O(n²) in the number of migrations to find edges.
pub fn order_migrations(migrations: &[&'static dyn NodeMigration]) -> Result<Vec<&'static dyn NodeMigration>, RunnerError> {
	let mut migrations = migrations.to_vec();
	migrations.sort_by_key(|migration| (migration.source(), migration.target()));

	for pair in migrations.windows(2) {
		if pair[0].source() == pair[1].source() {
			return Err(RunnerError::DuplicateMigration(pair[0].source()));
		}
	}
	if let Some(migration) = migrations.iter().find(|migration| Some(migration.target().version) != migration.source().version.checked_add(1)) {
		return Err(RunnerError::InvalidStep {
			from: migration.source(),
			to: migration.target(),
		});
	}

	let consumer = |declaration: Declaration| migrations.iter().position(|migration| migration.source() == declaration);

	// `successors[a]` holds every migration that must run after `a`
	let mut successors = vec![Vec::new(); migrations.len()];
	for (index, migration) in migrations.iter().enumerate() {
		if let Some(next) = consumer(migration.target()) {
			successors[index].push(next);
		}
		for pin in migration.pins() {
			// Two migrations can converge on one declaration, and the pin has to see the nodes of both
			for (before, producer) in migrations.iter().enumerate() {
				if producer.target() == *pin {
					successors[before].push(index);
				}
			}
			if let Some(after) = consumer(*pin) {
				successors[index].push(after);
			}
		}
	}

	let mut predecessor_counts = vec![0; migrations.len()];
	for next in successors.iter().flatten() {
		predecessor_counts[*next] += 1;
	}

	// Indices follow the sort above, so the smallest ready index is the smallest source
	let mut ready: BTreeSet<usize> = (0..migrations.len()).filter(|index| predecessor_counts[*index] == 0).collect();
	let mut ordered = Vec::with_capacity(migrations.len());
	while let Some(index) = ready.pop_first() {
		ordered.push(migrations[index]);
		for next in &successors[index] {
			predecessor_counts[*next] -= 1;
			if predecessor_counts[*next] == 0 {
				ready.insert(*next);
			}
		}
	}

	if ordered.len() < migrations.len() {
		let cycle = (0..migrations.len()).filter(|index| predecessor_counts[*index] > 0).map(|index| migrations[index].source()).collect();
		return Err(RunnerError::OrderCycle(cycle));
	}

	Ok(ordered)
}

/// Rewrite `history` through every migration whose source declaration it references, in dependency order. A migration is pending
/// even when no live node uses its source, since a removed node keeps its old shape in history and undo can bring it back.
pub fn migrate_history(migrations: &[&'static dyn NodeMigration], history: Vec<Delta>, resolver: &dyn DeclarationResolver) -> Result<RewrittenHistory, RunnerError> {
	let mut revs: HashMap<Rev, Option<Rev>> = history.iter().map(|delta| (delta.id, Some(delta.id))).collect();
	let mut deltas = history;

	for migration in order_migrations(migrations)? {
		if !deltas.iter().any(|delta| references(&delta.kind, migration.source(), resolver)) {
			continue;
		}

		let step = rewrite_history(migration, &deltas, resolver)?;
		for rev in revs.values_mut() {
			*rev = rev.and_then(|rev| step.revs.get(&rev).copied().flatten());
		}
		deltas = step.deltas;
	}

	Ok(RewrittenHistory { deltas, revs })
}

/// `M` of every migration on one state, for a document whose history can't be rewritten. Undo can't cross the result.
pub fn migrate_registry(migrations: &[&'static dyn NodeMigration], registry: &Registry, resolver: &dyn DeclarationResolver) -> Result<Registry, RunnerError> {
	let mut migrated = registry.clone();
	for migration in order_migrations(migrations)? {
		migrated = apply_migration(migration, &migrated, resolver)?;
	}
	Ok(migrated)
}

/// Whether `kind` stores a node implementing `declaration`.
pub fn references(kind: &RegistryDelta, declaration: Declaration, resolver: &dyn DeclarationResolver) -> bool {
	let is_declaration = |implementation: &Implementation| matches!(implementation, Implementation::ProtoNode(resource) if resolver.declaration(*resource) == Some(declaration));
	let implements = |node: &Node| is_declaration(node.implementation());

	match kind {
		RegistryDelta::AddNode { node, .. } => implements(node),
		RegistryDelta::RemoveNode { snapshot, .. } => implements(snapshot),
		RegistryDelta::SetNodeImplementation { implementation, .. } => is_declaration(implementation),
		_ => false,
	}
}
