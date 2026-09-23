use crate::backends::memory::MemoryBackend;
use crate::{AnyContainer, Result};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentKey(pub u64);

#[cfg(not(target_family = "wasm"))]
pub type StoreFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;
#[cfg(target_family = "wasm")]
pub type StoreFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + 'a>>;

pub trait DocumentStore: Send + Sync {
	fn list(&self) -> StoreFuture<'_, Vec<DocumentKey>>;
	fn open(&self, key: DocumentKey) -> StoreFuture<'_, AnyContainer>;
	fn remove(&self, key: DocumentKey) -> StoreFuture<'_, ()>;
}

fn directory_name(key: DocumentKey) -> String {
	format!("{:016x}", key.0)
}

fn parse_directory_name(name: &str) -> Option<DocumentKey> {
	(name.len() == 16 && name.bytes().all(|byte| byte.is_ascii_hexdigit()))
		.then(|| u64::from_str_radix(name, 16).ok().map(DocumentKey))
		.flatten()
}

#[derive(Default)]
pub struct MemoryStore {
	documents: Mutex<HashMap<DocumentKey, MemoryBackend>>,
}

impl DocumentStore for MemoryStore {
	fn list(&self) -> StoreFuture<'_, Vec<DocumentKey>> {
		let mut keys: Vec<_> = self.documents.lock().unwrap().keys().copied().collect();
		keys.sort();
		Box::pin(async move { Ok(keys) })
	}

	fn open(&self, key: DocumentKey) -> StoreFuture<'_, AnyContainer> {
		let backend = self.documents.lock().unwrap().entry(key).or_default().clone();
		Box::pin(async move { Ok(AnyContainer::Memory(backend)) })
	}

	fn remove(&self, key: DocumentKey) -> StoreFuture<'_, ()> {
		self.documents.lock().unwrap().remove(&key);
		Box::pin(async { Ok(()) })
	}
}

#[cfg(not(target_family = "wasm"))]
pub struct FolderStore {
	root: std::path::PathBuf,
}

#[cfg(not(target_family = "wasm"))]
impl FolderStore {
	pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
		Self { root: root.into() }
	}
}

#[cfg(not(target_family = "wasm"))]
impl DocumentStore for FolderStore {
	fn list(&self) -> StoreFuture<'_, Vec<DocumentKey>> {
		Box::pin(async {
			let entries = match std::fs::read_dir(&self.root) {
				Ok(entries) => entries,
				Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
				Err(error) => return Err(error.into()),
			};

			let mut keys: Vec<_> = entries
				.filter_map(|entry| entry.ok())
				.filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
				.filter_map(|entry| parse_directory_name(&entry.file_name().to_string_lossy()))
				.collect();
			keys.sort();
			Ok(keys)
		})
	}

	fn open(&self, key: DocumentKey) -> StoreFuture<'_, AnyContainer> {
		Box::pin(async move {
			let backend = crate::backends::folder::FolderBackend::create(self.root.join(directory_name(key)))?;
			Ok(AnyContainer::Folder(backend))
		})
	}

	fn remove(&self, key: DocumentKey) -> StoreFuture<'_, ()> {
		Box::pin(async move {
			match std::fs::remove_dir_all(self.root.join(directory_name(key))) {
				Ok(()) => Ok(()),
				Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
				Err(error) => Err(error.into()),
			}
		})
	}
}

#[cfg(target_family = "wasm")]
pub struct OpfsStore {
	root: String,
}

#[cfg(target_family = "wasm")]
impl OpfsStore {
	pub fn new(root: impl Into<String>) -> Self {
		Self {
			root: root.into().trim_matches('/').to_string(),
		}
	}

	fn document_path(&self, key: DocumentKey) -> String {
		let name = directory_name(key);
		if self.root.is_empty() { name } else { format!("{}/{name}", self.root) }
	}
}

#[cfg(target_family = "wasm")]
impl DocumentStore for OpfsStore {
	fn list(&self) -> StoreFuture<'_, Vec<DocumentKey>> {
		Box::pin(async {
			let paths = crate::backends::opfs::list_directories(&self.root).await?;
			let mut keys: Vec<_> = paths.iter().filter_map(|path| parse_directory_name(path.rsplit('/').next()?)).collect();
			keys.sort();
			Ok(keys)
		})
	}

	fn open(&self, key: DocumentKey) -> StoreFuture<'_, AnyContainer> {
		Box::pin(async move {
			let backend = crate::backends::opfs::OpfsBackend::open(&self.document_path(key)).await?;
			Ok(AnyContainer::Opfs(backend))
		})
	}

	fn remove(&self, key: DocumentKey) -> StoreFuture<'_, ()> {
		Box::pin(async move { crate::backends::opfs::remove_directory(&self.document_path(key)).await })
	}
}
