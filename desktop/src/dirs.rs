use std::fs::create_dir_all;
use std::path::PathBuf;

static APP_NAME: &str = "graphite-desktop";

pub(crate) fn ensure_dir_exists(path: &PathBuf) {
	if !path.exists() {
		create_dir_all(path).unwrap_or_else(|_| panic!("Failed to create directory at {path:?}"));
	}
}

pub(crate) fn graphite_data_dir() -> PathBuf {
	let path = dirs::data_dir().expect("Failed to get data directory").join(APP_NAME);
	ensure_dir_exists(&path);
	path
}
