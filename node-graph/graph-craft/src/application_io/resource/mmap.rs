use graphene_application_io::{LoadResource, Resource, ResourceFuture, ResourceHash, ResourceStorage};
use mmap_io::mmap::{MemoryMappedFile, MmapMode};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct MmapResourceStorage {
	root: PathBuf,
	cache: RwLock<HashMap<ResourceHash, Resource>>,
}

impl MmapResourceStorage {
	pub fn new(root: impl Into<PathBuf>) -> std::io::Result<Self> {
		let root = root.into();
		fs::create_dir_all(&root)?;
		Ok(Self {
			root,
			cache: RwLock::new(HashMap::new()),
		})
	}

	fn path_for(&self, hash: &ResourceHash) -> PathBuf {
		let hash = String::from(hash);
		let mut path = self.root.clone();
		path.push(&hash[..2]);
		path.push(&hash[2..]);
		path
	}

	fn open_mmap(path: &Path) -> Option<MemoryMappedFile> {
		match MemoryMappedFile::builder(path).mode(MmapMode::ReadOnly).huge_pages(true).open() {
			Ok(file) => Some(file),
			Err(error) => {
				log::warn!("Failed to mmap {path:?} retrying without huge pages: {error}");

				match MemoryMappedFile::open_ro(path) {
					Ok(file) => Some(file),
					Err(error) => {
						log::error!("Failed to mmap {path:?}: {error}");
						None
					}
				}
			}
		}
	}

	fn lookup(&self, hash: &ResourceHash) -> Option<Resource> {
		if let Some(resource) = self.cache.read().unwrap_or_else(|poisoned| poisoned.into_inner()).get(hash) {
			return Some(resource.clone());
		}

		let path = self.path_for(hash);
		let mmap = Self::open_mmap(&path)?;
		let resource = Resource::new(MmappedBytes(mmap));

		self.cache.write().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(*hash, resource.clone());
		Some(resource)
	}
}

impl LoadResource for MmapResourceStorage {
	fn load(&self, hash: ResourceHash) -> ResourceFuture {
		let result = self.lookup(&hash);
		Box::pin(async move { result })
	}
}

impl ResourceStorage for MmapResourceStorage {
	fn store(&mut self, data: &[u8]) -> ResourceHash {
		let hash = ResourceHash::from(data);
		let path = self.path_for(&hash);

		if path.exists() {
			return hash;
		}

		let Some(parent) = path.parent() else {
			log::error!("Resource path {path:?} has no parent directory");
			return hash;
		};
		if let Err(error) = fs::create_dir_all(parent) {
			log::error!("Failed to create resource subdirectory {parent:?}: {error}");
			return hash;
		}

		let tmp = parent.join(format!(
			"tmp.{}.{}.{}",
			path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
			std::process::id(),
			std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0),
		));

		let write_result = (|| -> std::io::Result<()> {
			let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
			file.write_all(data)?;
			file.sync_all()?;
			fs::rename(&tmp, &path)
		})();

		if let Err(error) = write_result {
			let _ = fs::remove_file(&tmp);
			if !path.exists() {
				log::error!("Failed to write resource to {path:?}: {error}");
			}
		}

		hash
	}

	fn contains(&mut self, hash: &ResourceHash) -> bool {
		self.cache.get_mut().unwrap_or_else(|poisoned| poisoned.into_inner()).contains_key(hash) || self.path_for(hash).exists()
	}

	fn garbage_collect(&mut self, used: &[ResourceHash]) {
		let used_set: std::collections::HashSet<ResourceHash> = used.iter().cloned().collect();
		self.cache.get_mut().unwrap_or_else(|poisoned| poisoned.into_inner()).retain(|hash, _| used_set.contains(hash));

		let Ok(top_entries) = fs::read_dir(&self.root) else { return };
		for top_entry in top_entries.flatten() {
			let top_path = top_entry.path();
			if !top_path.is_dir() {
				continue;
			}
			let Ok(entries) = fs::read_dir(&top_path) else { continue };
			for entry in entries.flatten() {
				let path = entry.path();
				let Some(prefix) = top_path.file_name().and_then(|n| n.to_str()) else { continue };
				let Some(suffix) = path.file_name().and_then(|n| n.to_str()) else { continue };
				if suffix.starts_with("tmp.") {
					continue;
				}
				let hex = format!("{prefix}{suffix}");
				let Ok(hash) = ResourceHash::try_from(hex.as_str()) else { continue };
				if !used_set.contains(&hash)
					&& let Err(error) = fs::remove_file(&path)
				{
					log::error!("Failed to remove unused resource {path:?}: {error}");
				}
			}

			let _ = fs::remove_dir(&top_path);
		}
	}
}

struct MmappedBytes(MemoryMappedFile);
impl AsRef<[u8]> for MmappedBytes {
	fn as_ref(&self) -> &[u8] {
		let len = self.0.len();
		match self.0.as_slice(0, len) {
			Ok(slice) => slice,
			Err(error) => {
				log::error!("Failed to obtain mmap slice: {error}");
				&[]
			}
		}
	}
}
