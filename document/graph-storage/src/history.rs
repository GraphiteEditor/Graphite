//! Retired delta history: the durable, append-only DAG of committed deltas.
//!
//! [`History`] owns the deltas in topological order (every parent precedes its children) plus an
//! index from [`Rev`] to position for O(1) lookup. The order is a valid replay order, so it is what
//! gets serialized to the on-disk history file and what [`crate::Session::replay_from_history`]
//! consumes. Appending a commit keeps the order; deltas arriving out of it are put back by
//! a canonical sort.

use std::collections::{HashMap, HashSet};

use crate::{AttributesWrite, CrdtError, Delta, RegistryDelta, ResourceHash, Rev, TimeStamp, Value};

/// Every mutator maintains the indexes, so the tips and named hashes are probes, not scans of a growing history.
#[derive(Clone, Debug, Default)]
pub struct History {
	/// Deltas in topological order. Mutated only via [`push`](Self::push).
	deltas: Vec<Delta>,
	/// `Rev` to its position in `deltas`. Kept in sync with `deltas` by every mutator.
	index: HashMap<Rev, usize>,
	/// Every rev some delta names as a parent.
	referenced: HashSet<Rev>,
	/// Deltas no other delta names as a parent.
	tips: HashSet<Rev>,
	/// Content hashes the deltas name, from resource additions and removals.
	resource_hashes: HashSet<ResourceHash>,
}

impl History {
	pub fn new() -> Self {
		Self::default()
	}

	/// Build from deltas already in topological order (the on-disk load path), indexing them in place.
	pub fn from_ordered(deltas: Vec<Delta>) -> Self {
		let mut history = Self::default();
		deltas.into_iter().for_each(|delta| history.push(delta));
		history
	}

	pub fn get(&self, rev: Rev) -> Option<&Delta> {
		self.index.get(&rev).map(|&position| &self.deltas[position])
	}

	pub fn contains(&self, rev: Rev) -> bool {
		self.index.contains_key(&rev)
	}

	/// The content hashes named by any resource addition or removal in history.
	pub fn resource_hashes(&self) -> &HashSet<ResourceHash> {
		&self.resource_hashes
	}

	pub fn len(&self) -> usize {
		self.deltas.len()
	}

	pub fn is_empty(&self) -> bool {
		self.deltas.is_empty()
	}

	/// Append a delta after its parents, keeping `deltas` and `index` in sync. A duplicate `Rev`
	/// (idempotent re-apply) overwrites the existing entry in place rather than appending, so the
	/// order and index are unchanged.
	pub fn push(&mut self, delta: Delta) {
		match &delta.kind {
			RegistryDelta::AddResource { entry, .. } => self.resource_hashes.extend(entry.hash),
			RegistryDelta::RemoveResource { snapshot, .. } => self.resource_hashes.extend(snapshot.hash),
			_ => {}
		}
		if let Some(&position) = self.index.get(&delta.id) {
			self.deltas[position] = delta;
			return;
		}
		for parent in delta.all_parents() {
			self.referenced.insert(parent);
			self.tips.remove(&parent);
		}
		if !self.referenced.contains(&delta.id) {
			self.tips.insert(delta.id);
		}
		self.index.insert(delta.id, self.deltas.len());
		self.deltas.push(delta);
	}

	/// Deltas in topological order (a valid replay order).
	pub fn iter(&self) -> impl Iterator<Item = &Delta> + '_ {
		self.deltas.iter()
	}

	/// Absorb `incoming` (dedup by `Rev`) and canonically re-sort the whole combined history.
	///
	/// The sort is deterministic (topological, ties broken by `Rev`), so two peers that absorb the same
	/// delta set produce byte-identical history, not merely two different valid orderings. This is the
	/// history-convergence mechanism: arrival order is erased. Callers update the registry separately
	/// (LWW apply is commutative, so the registry converges regardless of order).
	pub fn merge(&mut self, incoming: impl IntoIterator<Item = Delta>) {
		for delta in incoming {
			self.push(delta);
		}
		self.canonical_sort();
	}

	/// Re-order `deltas` into the canonical topological order and rebuild the index: parents precede
	/// children, and among deltas whose parents are all emitted the lowest `Rev` goes first. O(V + E).
	fn canonical_sort(&mut self) {
		// Unsatisfied in-history parent count per delta, plus reverse edges to decrement as parents emit.
		let mut pending_parents: HashMap<Rev, usize> = HashMap::with_capacity(self.deltas.len());
		let mut children: HashMap<Rev, Vec<Rev>> = HashMap::new();
		for delta in &self.deltas {
			let in_history_parents = delta.all_parents().filter(|parent| self.index.contains_key(parent)).count();
			pending_parents.insert(delta.id, in_history_parents);
			for parent in delta.all_parents() {
				if self.index.contains_key(&parent) {
					children.entry(parent).or_default().push(delta.id);
				}
			}
		}

		// Ready set as a min-heap on `Rev` (via `Reverse`) so ties resolve deterministically.
		let mut ready: std::collections::BinaryHeap<std::cmp::Reverse<Rev>> = pending_parents.iter().filter(|(_, count)| **count == 0).map(|(rev, _)| std::cmp::Reverse(*rev)).collect();

		let mut order: Vec<Rev> = Vec::with_capacity(self.deltas.len());
		while let Some(std::cmp::Reverse(rev)) = ready.pop() {
			order.push(rev);
			for child in children.get(&rev).into_iter().flatten() {
				let count = pending_parents.get_mut(child).expect("child is in history");
				*count -= 1;
				if *count == 0 {
					ready.push(std::cmp::Reverse(*child));
				}
			}
		}

		// `order` is a permutation of the existing revs, so reorder `deltas` to match and rebuild the index.
		let mut by_rev: HashMap<Rev, Delta> = self.deltas.drain(..).map(|delta| (delta.id, delta)).collect();
		self.index.clear();
		for (position, rev) in order.iter().enumerate() {
			if let Some(delta) = by_rev.remove(rev) {
				self.index.insert(*rev, position);
				self.deltas.push(delta);
			}
		}
	}

	/// `roots` and everything reachable from them through all parent links. Unknown roots are skipped.
	pub(crate) fn ancestors(&self, roots: impl IntoIterator<Item = Rev>) -> HashSet<Rev> {
		let mut seen = HashSet::new();
		let mut stack: Vec<Rev> = roots.into_iter().filter(|rev| self.contains(*rev)).collect();
		while let Some(rev) = stack.pop() {
			if seen.insert(rev)
				&& let Some(delta) = self.get(rev)
			{
				stack.extend(delta.all_parents());
			}
		}
		seen
	}

	/// The current tips: revs that no other delta lists as a parent (the divergent heads). A linear
	/// history has exactly one tip; concurrent branches have several. Sorted ascending for determinism.
	pub fn tips(&self) -> Vec<Rev> {
		let mut tips: Vec<Rev> = self.tips.iter().copied().collect();
		tips.sort_unstable();
		tips
	}

	/// Mark a retired delta as the end of a user interaction. Mutates only the delta's attributes
	/// (excluded from its `Rev`), so the index stays valid. Returns whether the delta was found.
	pub fn mark_interaction_end(&mut self, rev: Rev, timestamp: TimeStamp) -> bool {
		match self.index.get(&rev) {
			Some(&position) => {
				self.deltas[position].mark_interaction_end(timestamp);
				true
			}
			None => false,
		}
	}

	/// Set a local annotation attribute (e.g. a commit message) on a retired delta in place. Excluded
	/// from the delta's `Rev`, so identity and the index are unchanged. Returns whether the delta was found.
	pub fn annotate(&mut self, rev: Rev, key: &str, value: Value, timestamp: TimeStamp) -> bool {
		match self.index.get(&rev) {
			Some(&position) => {
				self.deltas[position].attributes.set(key, value, timestamp);
				true
			}
			None => false,
		}
	}

	/// Test-only mutable access to the first stored delta, for corrupting it to exercise `verify`.
	#[cfg(test)]
	pub(crate) fn first_mut(&mut self) -> Option<&mut Delta> {
		self.deltas.first_mut()
	}

	/// Verify the two stored invariants for history loaded from an untrusted source: every delta's
	/// content-addressed `id` matches its recomputed hash, and the deltas are in topological order
	/// (each delta's in-history parents precede it). Returns the first violation found.
	pub fn verify(&self) -> Result<(), CrdtError> {
		let mut seen: std::collections::HashSet<Rev> = std::collections::HashSet::with_capacity(self.deltas.len());
		for delta in &self.deltas {
			let expected = delta.recomputed_id();
			if delta.id != expected {
				return Err(CrdtError::RevMismatch { stored: delta.id, expected });
			}
			for parent in delta.all_parents() {
				if self.index.contains_key(&parent) && !seen.contains(&parent) {
					return Err(CrdtError::NotFoundInHistory(parent));
				}
			}
			seen.insert(delta.id);
		}
		Ok(())
	}
}
