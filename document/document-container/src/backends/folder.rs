//! Loose-folder backend.

use crate::{ByteHolder, Container, ContainerError, MmappedBytes, Result, validate_path};
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
		Ok(self.root.join(path))
	}

	fn list_filtered(&self, prefix: &str, want_files: bool) -> Result<Vec<String>> {
		validate_path(prefix)?;
		let base = if prefix.is_empty() { self.root.clone() } else { self.root.join(prefix) };
		if !base.is_dir() {
			return Ok(Vec::new());
		}

		let mut results = Vec::new();
		for entry in fs::read_dir(&base)? {
			let entry = entry?;
			let path = entry.path();
			let matches = if want_files { path.is_file() } else { path.is_dir() };
			if !matches {
				continue;
			}
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

		let file = open_mmap(&full)?;
		Ok(ByteHolder::Mmapped(MmappedBytes::new(file)))
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

		// `create_rw` materializes the full-size file before `fill` runs, so on any failure we remove
		// the partial file rather than leave a zeroed/half-written remnant behind.
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
		if !full.is_file() {
			return Err(ContainerError::NotFound(path.to_string()));
		}
		fs::remove_file(full)?;
		Ok(())
	}
}

/// Open a memory-mapped read-only view of `path`, trying huge pages first.
fn open_mmap(path: &Path) -> Result<MemoryMappedFile> {
	// The huge-pages builder rejects zero-length files with `ResizeFailed`, so skip it for empty files
	// and go straight to the plain mapping rather than logging a misleading huge-pages warning.
	let empty = fs::metadata(path).map(|metadata| metadata.len() == 0).unwrap_or(false);
	if empty {
		return MemoryMappedFile::open_ro(path).map_err(|error| ContainerError::Backend(format!("mmap of {path:?} failed: {error}")));
	}

	match MemoryMappedFile::builder(path).mode(MmapMode::ReadOnly).huge_pages(true).open() {
		Ok(file) => Ok(file),
		Err(error) => {
			log::warn!("Failed to mmap {path:?} with huge pages, retrying without: {error}");
			MemoryMappedFile::open_ro(path).map_err(|error| ContainerError::Backend(format!("mmap of {path:?} failed: {error}")))
		}
	}
}
