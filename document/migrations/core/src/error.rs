use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrationError {
	#[error("failed to decode payload: {0}")]
	Decode(String),
	#[error("failed to encode payload: {0}")]
	Encode(String),
	/// The source state violates its own declaration's contract.
	#[error("migration invariant violated: {0}")]
	Invariant(String),
	/// The target version can't express a reachable source state.
	#[error("source state is unrepresentable in the target version: {0}")]
	Unrepresentable(String),
	#[error("input slot {index} is out of range for {len} slots")]
	SlotOutOfRange { index: u32, len: usize },
	#[error("{0} does not read an output of the anchor")]
	OutsideRegion(String),
}

impl MigrationError {
	pub fn invariant(message: impl Into<String>) -> Self {
		Self::Invariant(message.into())
	}

	pub fn unrepresentable(message: impl Into<String>) -> Self {
		Self::Unrepresentable(message.into())
	}
}
