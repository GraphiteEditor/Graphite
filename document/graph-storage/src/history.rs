//! Retired delta history: the durable, append-only DAG of committed deltas.
//!
//! [`History`] owns the deltas in topological order (every parent precedes its children) plus an
//! index from [`Rev`] to position for O(1) lookup. The order is a valid replay order, so it is what
//! gets serialized to the on-disk history file and what [`crate::Session::replay_from_history`]
//! consumes. Appending a commit keeps the order; deltas arriving out of it are put back by
//! a canonical sort.

use std::collections::{HashMap, HashSet};

use crate::{CrdtError, Delta, RegistryDelta, ResourceHash, Rev, TimeStamp, Value};

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

	pub(crate) fn position(&self, rev: Rev) -> Option<usize> {
		self.index.get(&rev).copied()
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

	/// Whether the deltas from `from` on form a single-parent chain off the delta before them, so are already sorted.
	pub(crate) fn extends_canonically(&self, from: usize) -> bool {
		(from..self.deltas.len()).all(|position| {
			let delta = &self.deltas[position];
			delta.parent == position.checked_sub(1).map(|previous| self.deltas[previous].id) && !matches!(delta.kind, RegistryDelta::Merge { .. })
		})
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
		self.resource_hashes.extend(delta.kind.resource_hash());
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

	pub(crate) fn at(&self, position: usize) -> Option<&Delta> {
		self.deltas.get(position)
	}

	/// Re-order `deltas` into the canonical topological order and rebuild the index: parents precede
	/// children, and among deltas whose parents are all emitted the lowest `Rev` goes first. O(V + E).
	/// Peers holding the same deltas thus hold byte-identical history.
	pub(crate) fn canonical_sort(&mut self) {
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

	pub(crate) fn is_ancestor(&self, ancestor: Rev, descendant: Rev) -> bool {
		self.ancestors([descendant]).contains(&ancestor)
	}

	/// `tip`, its first-parent ancestors at distance 1, 2, 4, 8, ..., and the root.
	pub(crate) fn sample_chain(&self, tip: Rev) -> Vec<Rev> {
		let mut samples = vec![tip];
		let mut current = tip;
		let mut distance = 0_usize;
		while let Some(parent) = self.get(current).and_then(|delta| delta.parent) {
			current = parent;
			distance += 1;
			if distance.is_power_of_two() {
				samples.push(current);
			}
		}
		// Copies of one document share the root, which tells a diverged copy from an unrelated document.
		if samples.last() != Some(&current) {
			samples.push(current);
		}
		samples
	}

	/// Every delta not reachable from `known`, in topological order.
	pub(crate) fn deltas_unknown_to(&self, known: impl IntoIterator<Item = Rev>) -> Vec<&Delta> {
		let known = self.ancestors(known);
		self.deltas.iter().filter(|delta| !known.contains(&delta.id)).collect()
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

	/// Record when a retired delta entered history (outside its `Rev`). Returns whether the delta was found.
	pub(crate) fn set_retired_at(&mut self, rev: Rev, wall_ms: u64) -> bool {
		let Some(&position) = self.index.get(&rev) else { return false };
		self.deltas[position].retired_at_ms = wall_ms;
		true
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

/// Recompute every delta's content-addressed `Rev` after delta payloads were rewritten, walking in
/// topological order and remapping parent links (including `Merge` extra parents) along the way.
/// Returns the old → new mapping for remapping cursors (`head`, redo stack, broadcast tip).
pub fn rehash_deltas(deltas: &mut [Delta]) -> HashMap<Rev, Rev> {
	let mut mapping: HashMap<Rev, Rev> = HashMap::new();

	for delta in deltas.iter_mut() {
		let remapped = |rev: Rev| mapping.get(&rev).copied().unwrap_or(rev);

		if let RegistryDelta::Merge { extra_parents } = &mut delta.kind {
			// A merge's `parent` is its lowest, and remapping can change which that is
			let mut parents: Vec<Rev> = delta.parent.into_iter().chain(extra_parents.iter().copied()).map(remapped).collect();
			parents.sort_unstable();
			parents.dedup();
			delta.parent = parents.first().copied();
			*extra_parents = parents.split_first().map(|(_, rest)| rest.to_vec()).unwrap_or_default();
		} else {
			delta.parent = delta.parent.map(remapped);
		}

		let new_id = delta.recomputed_id();
		mapping.insert(delta.id, new_id);
		delta.id = new_id;
	}

	mapping
}
