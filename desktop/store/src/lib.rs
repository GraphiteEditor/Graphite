use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub type Hash = [u8; 32];
pub type Blob = Arc<[u8]>;

pub trait Store {
	fn write(&mut self, data: Blob) -> Hash;
	fn read(&mut self, hash: Hash) -> Option<Blob>;
	fn delete(&mut self, hash: Hash);
	fn contains(&self, hash: Hash) -> bool;
}

pub struct SimpleFSStore {
	root: PathBuf,
}

impl SimpleFSStore {
	pub fn new(root: PathBuf) -> Self {
		Self { root }
	}

	fn hash_to_path(&self, hash: Hash) -> PathBuf {
		let hex = hash.iter().map(|b| format!("{b:02x}")).collect::<String>();
		self.root.join(&hex[..2]).join(&hex[2..4]).join(&hex[4..])
	}
}

impl Store for SimpleFSStore {
	fn write(&mut self, data: Blob) -> Hash {
		let bytes = data.as_ref();
		let hash: Hash = blake3::hash(bytes).into();
		let path = self.hash_to_path(hash);

		if path.exists() {
			return hash;
		}

		if let Some(parent) = path.parent() {
			fs::create_dir_all(parent).ok();
		}

		if let Ok(mut file) = File::create(&path) {
			file.write_all(bytes).ok();
		}

		hash
	}

	fn read(&mut self, hash: Hash) -> Option<Blob> {
		let path = self.hash_to_path(hash);
		fs::read(path).ok().map(Arc::from)
	}

	fn delete(&mut self, hash: Hash) {
		let path = self.hash_to_path(hash);
		fs::remove_file(path).ok();
	}

	fn contains(&self, hash: Hash) -> bool {
		self.hash_to_path(hash).exists()
	}
}

struct CacheEntry {
	data: Blob,
	last_used: Instant,
}

pub struct InMemoryCacheStore<S: Store> {
	backing: S,
	cache: HashMap<Hash, CacheEntry>,
	current_size: usize,
	max_size: usize,
}

impl<S: Store> InMemoryCacheStore<S> {
	pub fn new(backing: S, max_size: usize) -> Self {
		Self {
			backing,
			cache: HashMap::new(),
			current_size: 0,
			max_size,
		}
	}

	fn evict_until_fits(&mut self, needed: usize) {
		while self.current_size + needed > self.max_size && !self.cache.is_empty() {
			let oldest_hash = self.cache.iter().min_by_key(|(_, entry)| entry.last_used).map(|(hash, _)| *hash);

			if let Some(hash) = oldest_hash
				&& let Some(entry) = self.cache.remove(&hash)
			{
				self.current_size -= entry.data.len();
			}
		}
	}

	fn insert_to_cache(&mut self, hash: Hash, data: Blob) {
		let size = data.len();

		if size > self.max_size {
			return;
		}

		self.evict_until_fits(size);

		if let std::collections::hash_map::Entry::Vacant(e) = self.cache.entry(hash) {
			e.insert(CacheEntry { data, last_used: Instant::now() });
			self.current_size += size;
		}
	}
}

impl<S: Store> Store for InMemoryCacheStore<S> {
	fn write(&mut self, data: Blob) -> Hash {
		let hash = self.backing.write(data.clone());
		self.insert_to_cache(hash, data);
		hash
	}

	fn read(&mut self, hash: Hash) -> Option<Blob> {
		if let Some(entry) = self.cache.get_mut(&hash) {
			entry.last_used = Instant::now();
			return Some(entry.data.clone());
		}

		if let Some(data) = self.backing.read(hash) {
			self.insert_to_cache(hash, data.clone());
			Some(data)
		} else {
			None
		}
	}

	fn delete(&mut self, hash: Hash) {
		self.backing.delete(hash);
		if let Some(entry) = self.cache.remove(&hash) {
			self.current_size -= entry.data.len();
		}
	}

	fn contains(&self, hash: Hash) -> bool {
		if self.cache.contains_key(&hash) {
			return true;
		}
		self.backing.contains(hash)
	}
}
