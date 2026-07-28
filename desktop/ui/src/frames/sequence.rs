use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
#[cfg(feature = "accelerated_paint")]
use std::time::Instant;

use crate::consts::FRAMES_IN_FLIGHT_LIMIT;

pub(crate) struct SequenceState {
	last_sent: AtomicU64,
	last_acked: AtomicU64,
	ack_lock: Mutex<()>,
	ack_signal: Condvar,
}

impl SequenceState {
	pub(crate) fn new() -> Self {
		Self {
			last_sent: AtomicU64::new(0),
			last_acked: AtomicU64::new(0),
			ack_lock: Mutex::new(()),
			ack_signal: Condvar::new(),
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
		drop(self.ack_lock.lock().unwrap_or_else(PoisonError::into_inner));
		self.ack_signal.notify_all();
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

	#[cfg(feature = "accelerated_paint")]
	pub(crate) fn wait_for_ack(&self) -> bool {
		let deadline = Instant::now() + crate::consts::FRAME_ACK_TIMEOUT;
		let mut guard = self.sequence.ack_lock.lock().unwrap_or_else(PoisonError::into_inner);
		loop {
			if self.sequence.last_acked.load(Ordering::Relaxed) >= self.seq {
				return true;
			}
			let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
				return false;
			};
			guard = self.sequence.ack_signal.wait_timeout(guard, remaining).unwrap_or_else(PoisonError::into_inner).0;
		}
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
