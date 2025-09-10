use std::path::PathBuf;

use crate::dirs::{ensure_dir_exists, graphite_data_dir};

static CEF_DIR_NAME: &str = "browser";

pub(crate) fn cef_data_dir() -> PathBuf {
	let path = graphite_data_dir().join(CEF_DIR_NAME);
	ensure_dir_exists(&path);
	path
}

pub(crate) fn cef_cache_dir() -> PathBuf {
	let path = cef_data_dir().join("cache");
	ensure_dir_exists(&path);
	path
}
