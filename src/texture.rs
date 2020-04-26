use image::GenericImageView;

pub struct Texture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub sampler: wgpu::Sampler,
}

impl Texture {
	pub fn from_filepath(device: &wgpu::Device, queue: &mut wgpu::Queue, path: &str) -> Result<Self, failure::Error> {
		let bytes = std::fs::read(path)?;
		Texture::from_bytes(device, queue, &bytes[..])
	}
	
	pub fn from_bytes(device: &wgpu::Device, queue: &mut wgpu::Queue, bytes: &[u8]) -> Result<Self, failure::Error> {
		let img = image::load_from_memory(bytes)?;
		Self::from_image(device, queue, &img)
	}

	pub fn from_image(device: &wgpu::Device, queue: &mut wgpu::Queue, img: &image::DynamicImage) -> Result<Self, failure::Error> {
		let rgba = img.as_rgba8().unwrap();
		let dimensions = img.dimensions();
		let size = wgpu::Extent3d {
			width: dimensions.0,
			height: dimensions.1,
			depth: 1,
		};

		let texture = device.create_texture(&wgpu::TextureDescriptor {
			size,
			array_layer_count: 1,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8UnormSrgb,
			usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
		});

		let buffer = device.create_buffer_mapped(rgba.len(), wgpu::BufferUsage::COPY_SRC).fill_from_slice(&rgba);

		let mut encoder = device.create_command_encoder(&Default::default());

		encoder.copy_buffer_to_texture(
			wgpu::BufferCopyView {
				buffer: &buffer,
				offset: 0,
				row_pitch: 4 * dimensions.0,
				image_height: dimensions.1,
			}, 
			wgpu::TextureCopyView {
				texture: &texture,
				mip_level: 0,
				array_layer: 0,
				origin: wgpu::Origin3d::ZERO,
			},
			size,
		);

		let command_buffer = encoder.finish();

		let view = texture.create_default_view();
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			lod_min_clamp: -100.0,
			lod_max_clamp: 100.0,
			compare_function: wgpu::CompareFunction::Always,
		});

		queue.submit(&[command_buffer]);
		
		Ok(Self { texture, view, sampler })
	}
}