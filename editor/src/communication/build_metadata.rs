use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Provides metadata about the build environment.
///
/// This data is viewable in the editor via the [`crate::dialog::AboutGraphite`] dialog.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

impl Display for BuildMetadata {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!(
			"Release Series: {}\n\nDate: {}\nHash:{}\nBranch: {}",
			self.release, self.timestamp, self.hash, self.branch
		))
	}
}
