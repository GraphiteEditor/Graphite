use glam::UVec2;
use std::collections::VecDeque;
use std::sync::Arc;

struct CachedTexture {
	texture: Arc<wgpu::Texture>,
	size: UVec2,
	bytes: u64,
}

pub(crate) struct TextureCache {
	/// Always sorted oldest-first by insertion/last-use order.
	textures: VecDeque<CachedTexture>,
	max_free_bytes: u64,
}

impl TextureCache {
	pub fn new(max_free_bytes: u64) -> Self {
		Self {
			textures: VecDeque::new(),
			max_free_bytes,
		}
	}

	pub fn request_texture(&mut self, device: &wgpu::Device, size: UVec2) -> Arc<wgpu::Texture> {
		let size = size.max(UVec2::ONE);

		if let Some(pos) = self.textures.iter().position(|e| e.size == size && Arc::strong_count(&e.texture) == 1) {
			let entry = self.textures.remove(pos).unwrap();
			let texture = entry.texture.clone();
			self.textures.push_back(entry);
			return texture;
		}

		let incoming_bytes = size.x as u64 * size.y as u64 * 4;
		self.evict_until_fits(incoming_bytes);

		let texture = Arc::new(device.create_texture(&wgpu::TextureDescriptor {
			label: Some(&format!("cached_texture_{}x{}", size.x, size.y)),
			size: wgpu::Extent3d {
				width: size.x,
				height: size.y,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: &[],
		}));

		self.textures.push_back(CachedTexture {
			texture: texture.clone(),
			size,
			bytes: incoming_bytes,
		});

		texture
	}

	fn total_free_bytes(&self) -> u64 {
		self.textures.iter().filter(|e| Arc::strong_count(&e.texture) == 1).map(|e| e.bytes).sum()
	}

	fn evict_until_fits(&mut self, incoming_bytes: u64) {
		let mut free_bytes = self.total_free_bytes();
		let max_free_bytes = self.max_free_bytes;

		if free_bytes + incoming_bytes <= max_free_bytes {
			return;
		}

		self.textures.retain(|entry| {
			if free_bytes + incoming_bytes <= max_free_bytes {
				return true;
			}
			if Arc::strong_count(&entry.texture) == 1 {
				free_bytes -= entry.bytes;
				false
			} else {
				true
			}
		});
	}
}
