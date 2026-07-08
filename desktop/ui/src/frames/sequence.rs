use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::consts::FRAMES_IN_FLIGHT_LIMIT;

pub(crate) struct SequenceState {
	last_sent: AtomicU64,
	last_acked: AtomicU64,
}

impl SequenceState {
	pub(crate) fn new() -> Self {
		Self {
			last_sent: AtomicU64::new(0),
			last_acked: AtomicU64::new(0),
		}
	}

	pub(crate) fn claim(self: &Arc<Self>) -> Option<FrameSequenceClaim> {
		let last_sent = self.last_sent.load(Ordering::Relaxed);
		let last_acked = self.last_acked.load(Ordering::Relaxed);
		if last_sent.saturating_sub(last_acked) >= FRAMES_IN_FLIGHT_LIMIT {
			return None;
		}
		let seq = last_sent + 1;
		self.last_sent.store(seq, Ordering::Relaxed);
		Some(FrameSequenceClaim {
			seq,
			sequence: self.clone(),
			commited: false,
		})
	}

	pub(crate) fn ack(&self, seq: u64) {
		self.last_acked.fetch_max(seq, Ordering::Relaxed);
	}
}

pub(crate) struct FrameSequenceClaim {
	seq: u64,
	commited: bool,
	sequence: Arc<SequenceState>,
}

impl FrameSequenceClaim {
	pub(crate) fn seq(&self) -> u64 {
		self.seq
	}

	pub(crate) fn commit(mut self) {
		self.commited = true;
	}
}

impl Drop for FrameSequenceClaim {
	fn drop(&mut self) {
		// Roll back the claim if it was never committed to free the sequence number
		if !self.commited {
			let _ = self.sequence.last_sent.compare_exchange(self.seq, self.seq - 1, Ordering::Relaxed, Ordering::Relaxed);
		}
	}
}
