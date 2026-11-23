use super::EditorDelta;
use std::sync::atomic::{AtomicU64, Ordering};

static REVISION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Default)]
pub(crate) struct DeltaLog(Vec<EditorDelta>);

impl DeltaLog {
	pub(crate) fn push(&mut self, delta: EditorDelta) {
		REVISION.fetch_add(1, Ordering::Relaxed);
		self.0.push(delta);
	}

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

pub(crate) fn revision() -> u64 {
	REVISION.load(Ordering::Relaxed)
}
