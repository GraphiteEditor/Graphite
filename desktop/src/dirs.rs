use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::consts::{APP_DIRECTORY_NAME, APP_DOCUMENTS_DIRECTORY_NAME};

pub(crate) fn ensure_dir_exists(path: &PathBuf) {
	if !path.exists() {
		fs::create_dir_all(path).unwrap_or_else(|_| panic!("Failed to create directory at {path:?}"));
	}
}

fn clear_dir(path: &PathBuf) {
	let Ok(entries) = fs::read_dir(path) else {
		tracing::error!("Failed to read directory at {path:?}");
		return;
	};
	for entry in entries.flatten() {
		let entry_path = entry.path();
		if entry_path.is_dir() {
			if let Err(e) = fs::remove_dir_all(&entry_path) {
				tracing::error!("Failed to remove directory at {:?}: {}", entry_path, e);
			}
		} else if entry_path.is_file() {
			if let Err(e) = fs::remove_file(&entry_path) {
				tracing::error!("Failed to remove file at {:?}: {}", entry_path, e);
			}
		}
	}
}

pub(crate) fn app_data_dir() -> PathBuf {
	let path = dirs::data_dir().expect("Failed to get data directory").join(APP_DIRECTORY_NAME);
	ensure_dir_exists(&path);
	path
}

fn app_tmp_dir() -> PathBuf {
	let path = std::env::temp_dir().join(APP_DIRECTORY_NAME);
	ensure_dir_exists(&path);
	path
}

pub(crate) fn app_tmp_dir_cleanup() {
	clear_dir(&app_tmp_dir());
}

pub(crate) fn app_autosave_documents_dir() -> PathBuf {
	let path = app_data_dir().join(APP_DOCUMENTS_DIRECTORY_NAME);
	ensure_dir_exists(&path);
	path
}

/// Temporary directory that is automatically deleted when dropped.
pub struct TempDir {
	path: PathBuf,
}

impl TempDir {
	pub fn new() -> io::Result<Self> {
		Self::new_with_parent(app_tmp_dir())
	}

	pub fn new_with_parent(parent: impl AsRef<Path>) -> io::Result<Self> {
		let random_suffix = format!("{:032x}", rand::random::<u128>());
		let name = format!("{}_{}", std::process::id(), random_suffix);
		let path = parent.as_ref().join(name);
		fs::create_dir_all(&path)?;
		Ok(Self { path })
	}
}

impl Drop for TempDir {
	fn drop(&mut self) {
		let result = fs::remove_dir_all(&self.path);
		if let Err(e) = result {
			tracing::error!("Failed to remove temporary directory at {:?}: {}", self.path, e);
		}
	}
}

impl AsRef<Path> for TempDir {
	fn as_ref(&self) -> &Path {
		&self.path
	}
}

// TODO: Eventually remove this cleanup code for the old "browser" CEF directory
pub(crate) fn delete_old_cef_browser_directory() {
	let old_browser_dir = crate::dirs::app_data_dir().join("browser");
	if old_browser_dir.is_dir() {
		let _ = std::fs::remove_dir_all(&old_browser_dir);
	}
}
