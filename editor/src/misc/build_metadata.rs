//! Provides metadata about the build environment.
//!
//! This data is viewable in the editor via the [AboutGraphite](crate::dialog::AboutGraphite) dialog.

pub fn release_series() -> String {
	format!("Release Series: {}", env!("GRAPHITE_RELEASE_SERIES"))
}

pub fn commit_info() -> String {
	format!("{}\n{}\n{}", commit_timestamp(), commit_hash(), commit_branch())
}

pub fn commit_info_localized(localized_commit_date: &str) -> String {
	format!("{}\n{}\n{}", commit_timestamp_localized(localized_commit_date), commit_hash(), commit_branch())
}

pub fn commit_timestamp() -> String {
	format!("Date: {}", env!("GRAPHITE_GIT_COMMIT_DATE"))
}

pub fn commit_timestamp_localized(localized_commit_date: &str) -> String {
	format!("Date: {}", localized_commit_date)
}

pub fn commit_hash() -> String {
	format!("Hash: {}", &env!("GRAPHITE_GIT_COMMIT_HASH")[..8])
}

pub fn commit_branch() -> String {
	format!("Branch: {}", env!("GRAPHITE_GIT_COMMIT_BRANCH"))
}
