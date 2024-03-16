use core::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TimeInfo {
	timestamp: Duration,
	prev_timestamp: Option<Duration>,
}

impl TimeInfo {
	pub fn frame_duration(&self) -> Option<Duration> {
		self.prev_timestamp.map(|prev| self.timestamp - prev)
	}

	pub fn advance_timestamp(&mut self, next_timestamp: Duration) {
		debug_assert!(next_timestamp >= self.timestamp);

		self.prev_timestamp = Some(self.timestamp);
		self.timestamp = next_timestamp;
	}
}
