use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use graphene_application_io::resource::*;
#[cfg(not(target_family = "wasm"))]
pub mod mmap;
#[cfg(not(target_family = "wasm"))]
pub use mmap::MmapResourceStorage;
#[cfg(target_family = "wasm")]
pub mod opfs;
#[cfg(target_family = "wasm")]
pub use opfs::OpfsResourceStorage;

#[derive(Debug, Default)]
pub struct HashMapResourceStorage {
	resources: Mutex<HashMap<ResourceHash, Resource>>,
}

impl HashMapResourceStorage {
	pub fn new() -> Self {
		Self::default()
	}
}

impl LoadResource for HashMapResourceStorage {
	fn load(&self, hash: ResourceHash) -> ResourceFuture<'_> {
		let result = self.resources.lock().unwrap().get(&hash).cloned();
		Box::pin(async move { result })
	}
}

impl ResourceStorage for HashMapResourceStorage {
	fn store(&self, data: &[u8]) -> ResourceHash {
		let hash = ResourceHash::from(data);
		self.resources.lock().unwrap().insert(hash, Resource::new(Arc::<[u8]>::from(data)));
		hash
	}

	fn contains(&self, hash: &ResourceHash) -> bool {
		self.resources.lock().unwrap().contains_key(hash)
	}

	fn garbage_collect(&self, used: &[ResourceHash]) {
		let used_set: std::collections::HashSet<&ResourceHash> = used.iter().collect();
		self.resources.lock().unwrap().retain(|hash, _| used_set.contains(hash));
	}
}
