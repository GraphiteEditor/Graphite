//! Loose-folder backend.

use crate::{ByteHolder, Container, ContainerError, MmappedBytes, Result, validate_path, validate_prefix};
use mmap_io::mmap::{MemoryMappedFile, MmapMode};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct FolderBackend {
	root: PathBuf,
}

impl FolderBackend {
	/// Open an existing folder. Errors if `root` is not a directory.
	pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
		let root = root.into();
		if !root.is_dir() {
			return Err(ContainerError::NotFound(root.display().to_string()));
		}
		Ok(Self { root })
	}

	/// Create the folder if it does not exist, then open it.
	pub fn create(root: impl Into<PathBuf>) -> Result<Self> {
		let root = root.into();
		fs::create_dir_all(&root)?;
		Ok(Self { root })
	}

	pub fn root(&self) -> &std::path::Path {
		&self.root
	}

	fn resolve(&self, path: &str) -> Result<PathBuf> {
		validate_path(path)?;
		self.reject_symlinked_components(path)?;
		Ok(self.root.join(path))
	}

	/// Reject any existing component along `root/relative` that is a symlink. `validate_path`/`validate_prefix`
	/// block `..` and absolute paths, but a symlink stored under the root could still point outside it, so every
	/// path that gets joined onto the root must pass through here before it is opened or traversed.
	fn reject_symlinked_components(&self, relative: &str) -> Result<()> {
		let mut partial = self.root.clone();
		for component in Path::new(relative).components() {
			partial.push(component);
			if let Ok(metadata) = fs::symlink_metadata(&partial)
				&& metadata.file_type().is_symlink()
			{
				return Err(ContainerError::InvalidPath(relative.to_string()));
			}
		}
		Ok(())
	}

	fn list_filtered(&self, prefix: &str, want_files: bool) -> Result<Vec<String>> {
		validate_prefix(prefix)?;
		let base = if prefix.is_empty() || prefix == "." {
			self.root.clone()
		} else {
			self.reject_symlinked_components(prefix)?;
			self.root.join(prefix)
		};

		// A missing prefix has no entries; a prefix that names a file is a misuse.
		if base.is_file() {
			return Err(ContainerError::NotADirectory(prefix.to_string()));
		}
		if !base.is_dir() {
			return Ok(Vec::new());
		}

		let mut results = Vec::new();
		for entry in fs::read_dir(&base)? {
			let entry = entry?;

			// `file_type` does not follow symlinks, unlike `is_file`/`is_dir`. Skip symlink entries so a
			// listing never advertises a path that `resolve` would then reject as a container escape.
			let Ok(file_type) = entry.file_type() else { continue };
			let matches = if want_files { file_type.is_file() } else { file_type.is_dir() };
			if !matches {
				continue;
			}

			let path = entry.path();
			let relative = path.strip_prefix(&self.root).map_err(|_| ContainerError::Backend("path escaped root".into()))?;
			results.push(relative.to_string_lossy().replace('\\', "/"));
		}
		Ok(results)
	}
}

impl Container for FolderBackend {
	fn read(&self, path: &str) -> Result<ByteHolder> {
		let full = self.resolve(path)?;
		if !full.is_file() {
			return Err(ContainerError::NotFound(path.to_string()));
		}

		// Mmapping a zero-length file is platform-dependent and often fails, so serve empty files as owned
		// bytes and reserve mmap for files that actually have content.
		if fs::metadata(&full).map(|metadata| metadata.len() == 0).unwrap_or(false) {
			return Ok(ByteHolder::Owned(Vec::new()));
		}

		Ok(ByteHolder::Mmapped(MmappedBytes::new(open_mmap(&full)?)?))
	}

	fn write(&self, path: &str, bytes: &[u8]) -> Result<()> {
		let full = self.resolve(path)?;
		if let Some(parent) = full.parent() {
			fs::create_dir_all(parent)?;
		}
		fs::write(&full, bytes)?;
		Ok(())
	}

	fn append(&self, path: &str, bytes: &[u8]) -> Result<()> {
		let full = self.resolve(path)?;
		if let Some(parent) = full.parent() {
			fs::create_dir_all(parent)?;
		}
		let mut file = OpenOptions::new().create(true).append(true).open(&full)?;
		file.write_all(bytes)?;
		Ok(())
	}

	fn write_sized(&self, path: &str, size: usize, fill: &mut dyn FnMut(&mut [u8]) -> Result<()>) -> Result<()> {
		if size == 0 {
			return self.write(path, &[]);
		}

		let full = self.resolve(path)?;
		if let Some(parent) = full.parent() {
			fs::create_dir_all(parent)?;
		}

		let file = MemoryMappedFile::create_rw(&full, size as u64).map_err(|error| ContainerError::Backend(format!("create_rw {full:?} failed: {error}")))?;

		// `create_rw` materializes the full-size file before `fill` runs, so remove it on failure rather
		// than leave a zeroed or half-written remnant.
		let result = (|| {
			let mut slice = file
				.as_slice_mut(0, size as u64)
				.map_err(|error| ContainerError::Backend(format!("as_slice_mut {full:?} failed: {error}")))?;
			fill(slice.as_mut())?;
			drop(slice);
			file.flush().map_err(|error| ContainerError::Backend(format!("flush {full:?} failed: {error}")))
		})();

		if result.is_err() {
			drop(file);
			let _ = fs::remove_file(&full);
		}
		result
	}

	fn list(&self, prefix: &str) -> Result<Vec<String>> {
		self.list_filtered(prefix, true)
	}

	fn list_dirs(&self, prefix: &str) -> Result<Vec<String>> {
		self.list_filtered(prefix, false)
	}

	fn exists(&self, path: &str) -> bool {
		match self.resolve(path) {
			Ok(full) => full.is_file(),
			Err(_) => false,
		}
	}

	fn remove(&self, path: &str) -> Result<()> {
		let full = self.resolve(path)?;
		// Idempotent: a missing file is not an error. Directories are left alone.
		if full.is_file() {
			fs::remove_file(full)?;
		}
		Ok(())
	}
}

/// Open a memory-mapped read-only view of `path`, trying huge pages first. Callers must ensure `path`
/// is non-empty, since mmapping a zero-length file is platform-dependent.
fn open_mmap(path: &Path) -> Result<MemoryMappedFile> {
	// Huge pages are unavailable on many systems, so falling back to a plain read-only mapping is routine.
	match MemoryMappedFile::builder(path).mode(MmapMode::ReadOnly).huge_pages(true).open() {
		Ok(file) => Ok(file),
		Err(_) => MemoryMappedFile::open_ro(path).map_err(|error| ContainerError::Backend(format!("mmap of {path:?} failed: {error}"))),
	}
}
