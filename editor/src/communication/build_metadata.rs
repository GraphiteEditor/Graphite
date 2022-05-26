use serde::{Deserialize, Serialize};

/// Provides metadata about the build environment.
///
/// This data is viewable in the editor via the [`crate::dialog::AboutGraphite`] dialog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildMetadata {
	pub release: String,
	pub timestamp: String,
	pub hash: String,
	pub branch: String,
}

impl Default for BuildMetadata {
	fn default() -> Self {
		Self {
			release: "unknown".to_string(),
			timestamp: "unknown".to_string(),
			hash: "unknown".to_string(),
			branch: "unknown".to_string(),
		}
	}
}

impl BuildMetadata {
	pub fn release_series(&self) -> String {
		format!("Release Series: {}", self.release)
	}

	pub fn commit_info(&self) -> String {
		format!("{}\n{}\n{}", self.commit_timestamp(), self.commit_hash(), self.commit_branch())
	}

	pub fn commit_timestamp(&self) -> String {
		format!("Date: {}", self.timestamp)
	}

	pub fn commit_hash(&self) -> String {
		format!("Hash: {}", self.hash)
	}

	pub fn commit_branch(&self) -> String {
		format!("Branch: {}", self.branch)
	}
}
