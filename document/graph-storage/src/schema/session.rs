use super::{Attributes, PeerId, RegistryDelta, TimeStamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One live op in the hot zone. Carries only enough to drive live LWW; no parents (transient),
/// no Rev (not content-addressed in the durable DAG). GC'd at retirement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HotOp {
	pub op: RegistryDelta,
	pub timestamp: TimeStamp,
	/// Position in its author's run. See [`HotSequence`].
	pub sequence: HotSequence,
	/// Shared with every peer, unlike `Delta::attributes`. Dropped at retirement.
	#[serde(default)]
	pub attributes: Attributes,
}

/// Position in one author's run of hot ops, counting from 1 with no gaps. Kept apart from the Lamport
/// counter, which skips on observing a higher remote timestamp and so cannot bound a contiguous prefix.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HotSequence(pub u64);

/// One hot op's author and position in its run, as the retired marks name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HotOpId {
	pub peer: PeerId,
	pub sequence: HotSequence,
}

/// Which hot ops are retired or taken back, as each author's gap-free prefix plus runs past it while ops are in flight.
/// Persisted, so a late copy of such an op is dropped.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetiredMarks {
	pub retired_up_to: HashMap<PeerId, HotSequence>,
	/// retired ops past their author's prefix, as inclusive runs, one per gap in flight.
	pub retired_runs: HashMap<PeerId, Vec<(HotSequence, HotSequence)>>,
	/// Which covered ops their author took back rather than retired, as inclusive runs.
	#[serde(default)]
	pub taken_back: HashMap<PeerId, Vec<(HotSequence, HotSequence)>>,
}
