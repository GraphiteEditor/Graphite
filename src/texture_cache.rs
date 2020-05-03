use std::collections::HashMap;
use super::texture::Texture;

#[derive(Copy, Clone, PartialEq, Debug)]
struct CacheID {
	index: usize,
}

impl CacheID {
	fn new(index: usize) -> Self {
		Self { index }
	}
}

pub struct TextureCache {
	pub textures: Vec<Texture>,
	name_to_id: HashMap<String, CacheID>,
}

impl TextureCache {
	pub fn new() -> Self {
		let textures = Vec::new();
		let name_to_id = HashMap::new();

		Self {
			textures,
			name_to_id,
		}
	}

	#[allow(dead_code)]
	pub fn get(&self, name: &str) -> Option<&Texture> {
		match self.name_to_id.get(name) {
			Some(id) => self.textures.get(id.index),
			None => None,
		}
	}

	#[allow(dead_code)]
	pub fn set(&mut self, name: &str, texture: Texture) {
		match self.name_to_id.get(name) {
			Some(id) => {
				self.textures[id.index] = texture;
			},
			None => {
				let last_index = self.name_to_id.len();
				let id = CacheID::new(last_index);
				self.name_to_id.insert(String::from(name), id);
				self.textures.push(texture);
			}
		}
	}

	#[allow(dead_code)]
	pub fn load(&mut self, device: &wgpu::Device, queue: &mut wgpu::Queue, path: &str) -> std::io::Result<()> {
		if self.name_to_id.get(path).is_none() {
			let texture = Texture::from_filepath(device, queue, "textures/grid.png").unwrap();

			let length = self.name_to_id.len();
			self.name_to_id.insert(String::from(path), CacheID::new(length));
			self.textures.push(texture);
		}

		Ok(())
	}
}