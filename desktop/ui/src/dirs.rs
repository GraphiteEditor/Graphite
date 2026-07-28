use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
const APP_DIRECTORY_NAME: &str = "graphite";
#[cfg(not(target_os = "linux"))]
const APP_DIRECTORY_NAME: &str = "Graphite";

pub(crate) fn app_tmp_dir() -> PathBuf {
	let path = std::env::temp_dir().join(APP_DIRECTORY_NAME);
	if let Err(e) = fs::create_dir_all(&path) {
		tracing::error!("Failed to create temp directory at {path:?}: {e}");
	}
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
