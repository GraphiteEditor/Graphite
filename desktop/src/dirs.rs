use std::fs::create_dir_all;
use std::path::PathBuf;

use crate::consts::{APP_DIRECTORY_NAME, APP_DOCUMENTS_DIRECTORY_NAME};

pub(crate) fn ensure_dir_exists(path: &PathBuf) {
	if !path.exists() {
		create_dir_all(path).unwrap_or_else(|_| panic!("Failed to create directory at {path:?}"));
	}
}

pub(crate) fn app_data_dir() -> PathBuf {
	let path = dirs::data_dir().expect("Failed to get data directory").join(APP_DIRECTORY_NAME);
	ensure_dir_exists(&path);
	path
}

pub(crate) fn app_autosave_documents_dir() -> PathBuf {
	let path = app_data_dir().join(APP_DOCUMENTS_DIRECTORY_NAME);
	ensure_dir_exists(&path);
	path
}
