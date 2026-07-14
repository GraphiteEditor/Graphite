use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrationError {
	#[error("failed to decode payload: {0}")]
	Decode(String),
	#[error("failed to encode payload: {0}")]
	Encode(String),
	#[error("migration invariant violated: {0}")]
	Invariant(String),
}
