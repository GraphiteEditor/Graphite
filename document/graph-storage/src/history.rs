//! Retired delta history: the durable, append-only DAG of committed deltas.
//!
//! [`History`] owns the deltas in topological order (every parent precedes its children) plus an
//! index from [`Rev`] to position for O(1) lookup. The order is a valid replay order, so it is what
//! gets serialized to the on-disk history file and what [`crate::Session::replay_from_history`]
//! consumes. Retired commits have a single writer in every regime (solo editing, or leader-ordered
//! collaboration where the leader serializes retired commits), so appending preserves the order by
//! construction. The only operation that introduces out-of-order deltas is [`merge`](History::merge),
//! which re-sorts the combined set into the canonical order to restore the invariant.

use std::collections::{HashMap, HashSet};

use crate::{AttributesWrite, CrdtError, Delta, Rev, TimeStamp};

#[derive(Clone, Debug, Default)]
pub struct History {
	/// Deltas in topological order. Mutated only via [`push`](Self::push).
	deltas: Vec<Delta>,
	/// `Rev` to its position in `deltas`. Kept in sync with `deltas` by every mutator.
	index: HashMap<Rev, usize>,
}

impl History {
	pub fn new() -> Self {
		Self::default()
	}

	/// Build from deltas already in topological order (the on-disk load path), indexing them in place.
	pub fn from_ordered(deltas: Vec<Delta>) -> Self {
		let index = deltas.iter().enumerate().map(|(position, delta)| (delta.id, position)).collect();
		Self { deltas, index }
	}

	pub fn get(&self, rev: Rev) -> Option<&Delta> {
		self.index.get(&rev).map(|&position| &self.deltas[position])
	}

	pub fn contains(&self, rev: Rev) -> bool {
		self.index.contains_key(&rev)
	}

	/// Whether history holds a delta authored at `timestamp`.
	pub fn contains_timestamp(&self, timestamp: TimeStamp) -> bool {
		self.deltas.iter().any(|delta| delta.timestamp == timestamp)
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
		if let Some(&position) = self.index.get(&delta.id) {
			self.deltas[position] = delta;
			return;
		}
		self.index.insert(delta.id, self.deltas.len());
		self.deltas.push(delta);
	}

	/// Deltas in topological order (a valid replay order).
	pub fn iter(&self) -> impl Iterator<Item = &Delta> + '_ {
		self.deltas.iter()
	}

	/// Re-order `deltas` into the canonical topological order and rebuild the index: parents precede
	/// children, and among deltas whose parents are all emitted the lowest `Rev` goes first. O(V + E).
	///
	/// Deterministic: two peers that absorb the same delta set end up with byte-identical history, not two
	/// different valid orderings. Arrival order is erased.
	pub fn canonical_sort(&mut self) {
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
	pub fn ancestors(&self, roots: impl IntoIterator<Item = Rev>) -> HashSet<Rev> {
		let mut seen = HashSet::new();
		let mut stack: Vec<Rev> = roots.into_iter().filter(|rev| self.contains(*rev)).collect();
		while let Some(rev) = stack.pop() {
			if !seen.insert(rev) {
				continue;
			}
			if let Some(delta) = self.get(rev) {
				stack.extend(delta.all_parents());
			}
		}
		seen
	}

	pub fn is_ancestor(&self, ancestor: Rev, descendant: Rev) -> bool {
		self.ancestors([descendant]).contains(&ancestor)
	}

	/// `tip` plus the revs at first-parent distance 1, 2, 4, 8, ... behind it. Sent to a remote peer
	/// so it can locate the divergence point within a factor of two of the true distance.
	pub fn sample_chain(&self, tip: Rev) -> Vec<Rev> {
		let mut samples = vec![tip];
		let mut current = tip;
		let mut distance = 0;
		let mut next_sample_distance = 1;

		while let Some(parent) = self.get(current).and_then(|delta| delta.parent) {
			current = parent;
			distance += 1;
			if distance == next_sample_distance {
				samples.push(current);
				next_sample_distance *= 2;
			}
		}

		samples
	}

	/// Every delta not reachable from the `known` revs the remote peer reported, in topological order.
	pub fn deltas_unknown_to(&self, known: impl IntoIterator<Item = Rev>) -> Vec<&Delta> {
		let known = self.ancestors(known);
		self.deltas.iter().filter(|delta| !known.contains(&delta.id)).collect()
	}

	/// The current tips: revs that no other delta lists as a parent (the divergent heads). A linear
	/// history has exactly one tip; concurrent branches have several. Sorted ascending for determinism.
	pub fn tips(&self) -> Vec<Rev> {
		let referenced: std::collections::HashSet<Rev> = self.deltas.iter().flat_map(|delta| delta.all_parents()).collect();
		let mut tips: Vec<Rev> = self.deltas.iter().map(|delta| delta.id).filter(|rev| !referenced.contains(rev)).collect();
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
	pub fn annotate(&mut self, rev: Rev, key: &str, value: serde_json::Value, timestamp: TimeStamp) -> bool {
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
