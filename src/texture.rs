use image::GenericImageView;

pub struct Texture {
	pub texture: wgpu::Texture,
	pub texture_view: wgpu::TextureView,
	pub sampler: wgpu::Sampler,
}

impl Texture {
	pub fn from_filepath(device: &wgpu::Device, queue: &mut wgpu::Queue, path: &str) -> Result<Self, failure::Error> {
		// Read the raw bytes from the specified file
		let bytes = std::fs::read(path)?;
		
		// Construct and return a Texture from the bytes
		Texture::from_bytes(device, queue, &bytes[..])
	}
	
	pub fn from_bytes(device: &wgpu::Device, queue: &mut wgpu::Queue, bytes: &[u8]) -> Result<Self, failure::Error> {
		// Create an image with the Image library
		let image = image::load_from_memory(bytes)?;

		// Construct and return a Texture from the Image
		Self::from_image(device, queue, &image)
	}

	pub fn from_image(device: &wgpu::Device, queue: &mut wgpu::Queue, image: &image::DynamicImage) -> Result<Self, failure::Error> {
		// Get data from image
		let rgba = image.as_rgba8().unwrap();
		let dimensions = image.dimensions();
		let size = wgpu::Extent3d {
			width: dimensions.0,
			height: dimensions.1,
			depth: 1,
		};

		// Create a buffer on the GPU and load it with the image pixel data
		let buffer = device.create_buffer_with_data(&rgba, wgpu::BufferUsage::COPY_SRC);

		// Create an empty texture on the GPU of the correct size for the buffer
		let texture = device.create_texture(&wgpu::TextureDescriptor {
			label: None,
			size,
			array_layer_count: 1,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8UnormSrgb,
			usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
		});

		// Use a command encoder to transfer the pixel data buffer into the texture
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
		encoder.copy_buffer_to_texture(
			wgpu::BufferCopyView {
				buffer: &buffer,
				offset: 0,
				bytes_per_row: 4 * dimensions.0,
				rows_per_image: dimensions.1,
			}, 
			wgpu::TextureCopyView {
				texture: &texture,
				mip_level: 0,
				array_layer: 0,
				origin: wgpu::Origin3d::ZERO,
			},
			size,
		);

		// Finishing the encoding yields the resulting command buffer that is submitted to the GPU's command queue
		let command_buffer = encoder.finish();
		queue.submit(&[command_buffer]);

		// Create the TextureView for this texture
		let view = texture.create_default_view();

		// Create the Sampler for this texture
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			lod_min_clamp: -100.0,
			lod_max_clamp: 100.0,
			compare: wgpu::CompareFunction::Always,
		});
		
		Ok(Self { texture, texture_view: view, sampler })
	}
}