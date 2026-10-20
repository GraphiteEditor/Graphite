use crate::wrapper::Clock;
use std::time::{Duration, Instant};

/// The desktop's monotonic clock, measuring time since the app started it.
pub(crate) struct InstantClock(Instant);

impl InstantClock {
	pub(crate) fn new() -> Self {
		Self(Instant::now())
	}
}

impl Clock for InstantClock {
	fn now(&self) -> Duration {
		self.0.elapsed()
	}
}
