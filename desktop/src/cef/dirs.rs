use std::path::PathBuf;

use crate::dirs::{app_data_dir, ensure_dir_exists};

static CEF_DIR_NAME: &str = "browser";

pub(crate) fn delete_instance_dirs() {
	let cef_dir = app_data_dir().join(CEF_DIR_NAME);
	if let Ok(entries) = std::fs::read_dir(&cef_dir) {
		for entry in entries.flatten() {
			let path = entry.path();
			if path.is_dir() {
				let _ = std::fs::remove_dir_all(&path);
			}
		}
	}
}

pub(crate) fn create_instance_dir() -> PathBuf {
	let instance_id: String = (0..32).map(|_| format!("{:x}", rand::random::<u8>() % 16)).collect();
	let path = app_data_dir().join(CEF_DIR_NAME).join(instance_id);
	ensure_dir_exists(&path);
	path
}
