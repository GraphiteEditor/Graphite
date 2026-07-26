use super::*;

/// One entity whose stored form may differ from the working storage registry after a mutation.
///
/// A record is a scope marker, not a payload: staging re-derives the entity from the live runtime
/// and diffs it against the working registry, so recording an unchanged entity is harmless and
/// presence decides the operation (runtime-only stages an add, registry-only a removal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedChange {
	/// A node's storage form (inputs, implementation, or attributes derived from its metadata) may have changed.
	Node { network_path: Vec<NodeId>, node_id: NodeId },
	/// A single input slot's value or wiring changed, the dominant record while dragging or painting.
	NodeInput { network_path: Vec<NodeId>, node_id: NodeId, input_index: usize },
	/// A network's storage form (export slots or network-level attributes) may have changed.
	Network { network_path: Vec<NodeId> },
}

/// The set of entities mutated since the last storage staging, letting staging derive history
/// deltas from just those entities instead of diffing the whole document.
///
/// Deliberately absent from serialization and cloning: wholesale interface replacement (document
/// open, undo/redo snapshot install, transaction abort) yields a fresh `Desynced` journal, which
/// obligates the next staging to full-diff before resuming incremental tracking.
#[derive(Debug, Clone)]
pub enum DeltaJournal {
	/// Every persistent mutation since the last staging is captured in `changes`.
	Tracking { changes: Vec<RecordedChange> },
	/// The changed-entity set is unknown; the next staging must fall back to a full diff.
	Desynced { reason: &'static str },
}

impl Default for DeltaJournal {
	fn default() -> Self {
		DeltaJournal::Desynced { reason: "fresh interface" }
	}
}

impl DeltaJournal {
	pub fn is_tracking(&self) -> bool {
		matches!(self, DeltaJournal::Tracking { .. })
	}

	fn push(&mut self, change: RecordedChange) {
		let DeltaJournal::Tracking { changes } = self else { return };

		// A whole-node record subsumes that node's per-input records, in both directions
		match &change {
			RecordedChange::Node { network_path, node_id } => {
				changes.retain(
					|existing| !matches!(existing, RecordedChange::NodeInput { network_path: existing_path, node_id: existing_node, .. } if existing_path == network_path && existing_node == node_id),
				);
			}
			RecordedChange::NodeInput { network_path, node_id, .. } => {
				let subsumed = changes
					.iter()
					.any(|existing| matches!(existing, RecordedChange::Node { network_path: existing_path, node_id: existing_node } if existing_path == network_path && existing_node == node_id));
				if subsumed {
					return;
				}
			}
			RecordedChange::Network { .. } => {}
		}

		if !changes.contains(&change) {
			changes.push(change);
		}
	}
}

impl NodeNetworkInterface {
	/// Records that a node's stored form may have changed and marks the transaction as modified.
	/// Mutation methods call this in place of a bare `transaction_modified`.
	pub(crate) fn record_node_change(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		self.journal_node_change(node_id, network_path);
		self.transaction_modified();
	}

	/// Records that a single input slot changed, routing export slots to their network's record,
	/// and marks the transaction as modified.
	pub(crate) fn record_input_change(&mut self, connector: &InputConnector, network_path: &[NodeId]) {
		match connector {
			InputConnector::Node { node_id, input_index } => {
				self.journal.push(RecordedChange::NodeInput {
					network_path: network_path.to_vec(),
					node_id: *node_id,
					input_index: *input_index,
				});
				self.transaction_modified();
			}
			InputConnector::Export(_) => self.record_network_change(network_path),
		}
	}

	/// Records that a network's stored form may have changed and marks the transaction as modified.
	pub(crate) fn record_network_change(&mut self, network_path: &[NodeId]) {
		self.journal_network_change(network_path);
		self.transaction_modified();
	}

	/// Records a node change without touching the transaction status, for setters that
	/// deliberately do not participate in the undo system (document upgrade scripts).
	pub(crate) fn journal_node_change(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		self.journal.push(RecordedChange::Node {
			network_path: network_path.to_vec(),
			node_id: *node_id,
		});
	}

	/// Records a network change without touching the transaction status.
	pub(crate) fn journal_network_change(&mut self, network_path: &[NodeId]) {
		self.journal.push(RecordedChange::Network { network_path: network_path.to_vec() });
	}

	/// Declares the changed-entity set unknown, forcing the next storage staging to run a full
	/// diff. Called by every access path that can mutate the document without itemized recording.
	pub(crate) fn mark_journal_desynced(&mut self, reason: &'static str) {
		self.journal = DeltaJournal::Desynced { reason };
	}

	/// The pending change records, for staging and tests to inspect.
	pub fn journal(&self) -> &DeltaJournal {
		&self.journal
	}

	/// Takes the accumulated journal for staging, leaving an empty tracking journal behind.
	/// The caller owns the consequences: a taken `Desynced` journal obligates a full diff.
	pub fn take_journal(&mut self) -> DeltaJournal {
		std::mem::replace(&mut self.journal, DeltaJournal::Tracking { changes: Vec::new() })
	}
}
