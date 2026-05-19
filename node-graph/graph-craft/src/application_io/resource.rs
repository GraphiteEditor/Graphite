#[cfg(not(target_family = "wasm"))]
pub mod mmap;
#[cfg(target_family = "wasm")]
pub mod opfs;

use graphene_application_io::{Resource, ResourceFuture, ResourceHash, ResourceStorage, Resources};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct HashMapResourceStorage {
	resources: Mutex<HashMap<ResourceHash, Resource>>,
}

impl HashMapResourceStorage {
	pub fn new() -> Self {
		Self::default()
	}
}

impl Resources for HashMapResourceStorage {
	fn load(&self, hash: ResourceHash) -> ResourceFuture {
		let result = self.resources.lock().unwrap().get(&hash).cloned();
		Box::pin(async move { result })
	}
}

impl ResourceStorage for HashMapResourceStorage {
	fn write(&mut self, data: &[u8]) -> ResourceHash {
		let hash = ResourceHash::from(data);
		self.resources.get_mut().unwrap().insert(hash, Resource::new(Arc::<[u8]>::from(data)));
		hash
	}

	fn contains(&mut self, hash: &ResourceHash) -> bool {
		self.resources.get_mut().unwrap().contains_key(hash)
	}

	fn garbage_collect(&mut self, used: &[ResourceHash]) {
		let used_set: std::collections::HashSet<&ResourceHash> = used.iter().collect();
		self.resources.get_mut().unwrap().retain(|hash, _| used_set.contains(hash));
	}
}
