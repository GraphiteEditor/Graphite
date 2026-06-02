//! In-memory backend. Useful for tests and as the deserialize target for archive codecs.

use crate::{ByteHolder, Container, ContainerError, Result, validate_path, with_trailing_slash};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

#[derive(Default)]
pub struct MemoryBackend {
	files: Mutex<HashMap<String, Vec<u8>>>,
}

impl MemoryBackend {
	pub fn new() -> Self {
		Self::default()
	}
}

impl Container for MemoryBackend {
	fn read(&self, path: &str) -> Result<ByteHolder> {
		validate_path(path)?;
		self.files
			.lock()
			.unwrap()
			.get(path)
			.map(|bytes| ByteHolder::Owned(bytes.clone()))
			.ok_or_else(|| ContainerError::NotFound(path.to_string()))
	}

	fn write(&self, path: &str, bytes: &[u8]) -> Result<()> {
		validate_path(path)?;
		self.files.lock().unwrap().insert(path.to_string(), bytes.to_vec());
		Ok(())
	}

	fn append(&self, path: &str, bytes: &[u8]) -> Result<()> {
		validate_path(path)?;
		self.files.lock().unwrap().entry(path.to_string()).or_default().extend_from_slice(bytes);
		Ok(())
	}

	fn list(&self, prefix: &str) -> Result<Vec<String>> {
		let normalized = with_trailing_slash(prefix);
		let files = self.files.lock().unwrap();
		let results = files.keys().filter(|path| path.starts_with(&normalized) && !path[normalized.len()..].contains('/')).cloned().collect();
		Ok(results)
	}

	fn list_dirs(&self, prefix: &str) -> Result<Vec<String>> {
		let normalized = with_trailing_slash(prefix);
		let files = self.files.lock().unwrap();
		let mut seen = HashSet::new();
		let mut results = Vec::new();
		for path in files.keys() {
			if !path.starts_with(&normalized) {
				continue;
			}
			let remainder = &path[normalized.len()..];
			if let Some((segment, _)) = remainder.split_once('/') {
				let dir = format!("{normalized}{segment}");
				if seen.insert(dir.clone()) {
					results.push(dir);
				}
			}
		}
		Ok(results)
	}

	fn exists(&self, path: &str) -> bool {
		validate_path(path).is_ok() && self.files.lock().unwrap().contains_key(path)
	}

	fn remove(&self, path: &str) -> Result<()> {
		validate_path(path)?;
		self.files.lock().unwrap().remove(path).map(|_| ()).ok_or_else(|| ContainerError::NotFound(path.to_string()))
	}
}
