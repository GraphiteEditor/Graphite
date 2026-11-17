use super::EditorDelta;
use std::sync::atomic::{AtomicU64, Ordering};

/// Counts every write recorded by any interface, so a value derived from an interface can tell whether it is still current.
static REVISION: AtomicU64 = AtomicU64::new(0);

/// What the writes since the last drain changed, in write order.
#[derive(Debug, Default)]
pub(crate) struct DeltaLog(Vec<EditorDelta>);

impl DeltaLog {
	pub(crate) fn push(&mut self, delta: EditorDelta) {
		REVISION.fetch_add(1, Ordering::Relaxed);
		self.0.push(delta);
	}

	/// Drops what was recorded after the first `length` writes, which still count as writes.
	pub(crate) fn truncate(&mut self, length: usize) {
		self.0.truncate(length);
	}

	pub(crate) fn clear(&mut self) {
		self.0.clear();
	}

	pub(crate) fn take(&mut self) -> Vec<EditorDelta> {
		std::mem::take(&mut self.0)
	}
}

impl std::ops::Deref for DeltaLog {
	type Target = [EditorDelta];

	fn deref(&self) -> &[EditorDelta] {
		&self.0
	}
}

/// How many writes all interfaces have recorded so far.
pub(crate) fn revision() -> u64 {
	REVISION.load(Ordering::Relaxed)
}
