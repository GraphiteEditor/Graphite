use std::collections::HashMap;

#[derive(Copy, Clone, PartialEq, Debug)]
struct CacheID {
	index: usize,
}

impl CacheID {
	fn new(index: usize) -> Self {
		Self { index }
	}
}

pub struct ResourceCache<T> {
	pub resources: Vec<T>,
	name_to_id: HashMap<String, CacheID>,
}

impl<T> ResourceCache<T> {
	pub fn new() -> Self {
		let resources = Vec::new();
		let name_to_id = HashMap::new();

		Self {
			resources,
			name_to_id,
		}
	}

	#[allow(dead_code)]
	pub fn get(&self, name: &str) -> Option<&T> {
		match self.name_to_id.get(name) {
			Some(id) => self.resources.get(id.index),
			None => None,
		}
	}

	#[allow(dead_code)]
	pub fn set(&mut self, name: &str, resource: T) {
		match self.name_to_id.get(name) {
			Some(id) => {
				self.resources[id.index] = resource;
			},
			None => {
				let last_index = self.name_to_id.len();
				let id = CacheID::new(last_index);
				self.name_to_id.insert(String::from(name), id);
				self.resources.push(resource);
			},
		}
	}
}