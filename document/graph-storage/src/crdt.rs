use crate::{Attributes, Delta, PeerId, Prior, RegistryDelta, Rev, TimeStamp, Value, attr, compute_rev};

impl Delta {
	pub fn new(parent: Option<Rev>, author: PeerId, timestamp: TimeStamp, kind: RegistryDelta, reverse: Vec<Prior>) -> Self {
		let id = compute_rev(parent, author, timestamp, &kind);
		Self {
			id,
			parent,
			author,
			timestamp,
			kind,
			reverse,
			attributes: Attributes::default(),
			retired_at_ms: 0,
		}
	}

	/// Build a merge delta joining `tips` into one node. See [`RegistryDelta::Merge`] for the semantics.
	pub fn merge(tips: impl IntoIterator<Item = Rev>, author: PeerId, timestamp: TimeStamp) -> Self {
		let mut parents: Vec<Rev> = tips.into_iter().collect();
		parents.sort_unstable();
		parents.dedup();
		let parent = parents.first().copied();
		let extra_parents = parents.split_first().map(|(_, rest)| rest.to_vec()).unwrap_or_default();
		let kind = RegistryDelta::Merge { extra_parents };
		let id = compute_rev(parent, author, timestamp, &kind);
		Self {
			id,
			parent,
			author,
			timestamp,
			reverse: Vec::new(),
			kind,
			attributes: Attributes::default(),
			retired_at_ms: 0,
		}
	}

	/// Every parent: the primary `parent` (absent for the root) plus a merge's `extra_parents`.
	pub fn all_parents(&self) -> impl Iterator<Item = Rev> + '_ {
		let extras = match &self.kind {
			RegistryDelta::Merge { extra_parents } => extra_parents.as_slice(),
			_ => &[],
		};
		self.parent.into_iter().chain(extras.iter().copied())
	}

	/// Mark this delta as the last op of a user interaction, so the undo cursor treats it as a checkpoint.
	pub fn mark_interaction_end(&mut self, timestamp: TimeStamp) {
		self.attributes.set(attr::delta::INTERACTION_END, Value::Bool(true), timestamp);
	}

	pub fn is_interaction_end(&self) -> bool {
		self.attributes.get(attr::delta::INTERACTION_END).is_some_and(|marker| marker.value == Value::Bool(true))
	}

	/// Take over `original`'s annotations and retirement time, for a delta that replaces it in rewritten history.
	pub fn carry_annotations(&mut self, original: &Delta) {
		self.attributes = original.attributes.clone();
		self.retired_at_ms = original.retired_at_ms;
	}

	/// When the delta entered history in wall-clock milliseconds, if the peer that retired it recorded it.
	pub fn retired_at(&self) -> Option<u64> {
		(self.retired_at_ms != 0).then_some(self.retired_at_ms)
	}

	/// The content-addressed `Rev` this delta's identity fields hash to. Equals `id` for a delta built
	/// via `new`/`merge`; differs only if `id` was tampered with or the hash derivation changed.
	pub fn recomputed_id(&self) -> Rev {
		compute_rev(self.parent, self.author, self.timestamp, &self.kind)
	}

	/// Whether `id` matches the recomputed content hash. `Delta` deserializes without checking this
	/// (the hash is not cheap over a large history); callers verify explicitly when they don't trust
	/// the source via [`Session::verify_history`].
	pub fn has_valid_id(&self) -> bool {
		self.id == self.recomputed_id()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn unrecorded_retired_at_costs_one_byte_and_round_trips() {
		let base = Delta::new(None, PeerId(1), TimeStamp { counter: 1, peer: PeerId(1) }, RegistryDelta::Other(Value::None), Vec::new());
		let mut stamped = base.clone();
		stamped.retired_at_ms = 1_790_000_000_000;

		let bare_base = postcard::to_allocvec(&base).expect("encode");
		let bare_stamped = postcard::to_allocvec(&stamped).expect("encode");
		// Only the integer's varint differs: one byte for zero, six for today's milliseconds, no tag byte in either.
		assert_eq!(bare_stamped.len() - bare_base.len(), postcard::to_allocvec(&1_790_000_000_000u64).unwrap().len() - 1);

		let decoded: Delta = postcard::from_bytes(&bare_stamped).expect("decode");
		assert_eq!(decoded.retired_at(), Some(1_790_000_000_000));
		let decoded: Delta = postcard::from_bytes(&bare_base).expect("decode");
		assert_eq!(decoded.retired_at(), None);
	}
}
