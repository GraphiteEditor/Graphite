use crate::WgpuExecutor;
use graphene_core::Color;
use graphene_core::Ctx;
use graphene_core::color::SRGBA8;
use graphene_core::ops::Convert;
use graphene_core::raster::Image;
use graphene_core::raster_types::{CPU, GPU, Raster};
use graphene_core::table::{Table, TableRow};
use graphene_core::transform::Footprint;
use wgpu::util::{DeviceExt, TextureDataOrder};
use wgpu::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};

/// Uploads CPU image data to a GPU texture
///
/// Creates a new WGPU texture with RGBA8UnormSrgb format and uploads the provided
/// image data. The texture is configured for binding, copying, and source operations.
fn upload_to_texture(device: &std::sync::Arc<wgpu::Device>, queue: &std::sync::Arc<wgpu::Queue>, image: &Raster<CPU>) -> wgpu::Texture {
	let rgba8_data: Vec<SRGBA8> = image.data.iter().map(|x| (*x).into()).collect();

	device.create_texture_with_data(
		queue,
		&TextureDescriptor {
			label: Some("upload_texture node texture"),
			size: Extent3d {
				width: image.width,
				height: image.height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: TextureDimension::D2,
			format: TextureFormat::Rgba8UnormSrgb,
			usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC,
			view_formats: &[],
		},
		TextureDataOrder::LayerMajor,
		bytemuck::cast_slice(rgba8_data.as_slice()),
	)
}

/// Converts a Raster<GPU> texture to Raster<CPU> by downloading the underlying texture data.
///
/// Assumptions:
/// - 2D texture, mip level 0
/// - 4 bytes-per-pixel RGBA8
/// - Texture has COPY_SRC usage
struct RasterGpuToRasterCpuConverter {
	buffer: wgpu::Buffer,
	width: u32,
	height: u32,
	unpadded_bytes_per_row: u32,
	padded_bytes_per_row: u32,
}
impl RasterGpuToRasterCpuConverter {
	fn new(device: &std::sync::Arc<wgpu::Device>, encoder: &mut wgpu::CommandEncoder, data_gpu: Raster<GPU>) -> Self {
		let texture = data_gpu.data();
		let width = texture.width();
		let height = texture.height();
		let bytes_per_pixel = 4; // RGBA8
		let unpadded_bytes_per_row = width * bytes_per_pixel;
		let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
		let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
		let buffer_size = padded_bytes_per_row as u64 * height as u64;

		let buffer = device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("texture_download_buffer"),
			size: buffer_size,
			usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
			mapped_at_creation: false,
		});

		encoder.copy_texture_to_buffer(
			wgpu::TexelCopyTextureInfo {
				texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			wgpu::TexelCopyBufferInfo {
				buffer: &buffer,
				layout: wgpu::TexelCopyBufferLayout {
					offset: 0,
					bytes_per_row: Some(padded_bytes_per_row),
					rows_per_image: Some(height),
				},
			},
			Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
		);

		Self {
			buffer,
			width,
			height,
			unpadded_bytes_per_row,
			padded_bytes_per_row,
		}
	}

	async fn convert(self) -> Result<Raster<CPU>, wgpu::BufferAsyncError> {
		let buffer_slice = self.buffer.slice(..);
		let (sender, receiver) = futures::channel::oneshot::channel();
		buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
			let _ = sender.send(result);
		});
		receiver.await.expect("Failed to receive map result")?;

		let view = buffer_slice.get_mapped_range();

		let row_stride = self.padded_bytes_per_row as usize;
		let row_bytes = self.unpadded_bytes_per_row as usize;
		let mut cpu_data: Vec<Color> = Vec::with_capacity((self.width * self.height) as usize);
		for row in 0..self.height as usize {
			let start = row * row_stride;
			let row_slice = &view[start..start + row_bytes];
			for px in row_slice.chunks_exact(4) {
				cpu_data.push(Color::from_rgba8_srgb(px[0], px[1], px[2], px[3]));
			}
		}

		drop(view);
		self.buffer.unmap();
		let cpu_image = Image {
			data: cpu_data,
			width: self.width,
			height: self.height,
			base64_string: None,
		};

		Ok(Raster::new_cpu(cpu_image))
	}
}

/// Passthrough conversion for GPU tables - no conversion needed
impl<'i> Convert<Table<Raster<GPU>>, &'i WgpuExecutor> for Table<Raster<GPU>> {
	async fn convert(self, _: Footprint, _converter: &'i WgpuExecutor) -> Table<Raster<GPU>> {
		self
	}
}

/// Converts CPU raster table to GPU by uploading each image to a texture
impl<'i> Convert<Table<Raster<GPU>>, &'i WgpuExecutor> for Table<Raster<CPU>> {
	async fn convert(self, _: Footprint, executor: &'i WgpuExecutor) -> Table<Raster<GPU>> {
		let device = &executor.context.device;
		let queue = &executor.context.queue;
		let table = self
			.iter()
			.map(|row| {
				let image = row.element;
				let texture = upload_to_texture(device, queue, image);

				TableRow {
					element: Raster::new_gpu(texture),
					transform: *row.transform,
					alpha_blending: *row.alpha_blending,
					source_node_id: *row.source_node_id,
				}
			})
			.collect();

		queue.submit([]);
		table
	}
}

/// Converts single CPU raster to GPU by uploading to texture
impl<'i> Convert<Raster<GPU>, &'i WgpuExecutor> for Raster<CPU> {
	async fn convert(self, _: Footprint, executor: &'i WgpuExecutor) -> Raster<GPU> {
		let device = &executor.context.device;
		let queue = &executor.context.queue;
		let texture = upload_to_texture(device, queue, &self);

		queue.submit([]);
		Raster::new_gpu(texture)
	}
}

/// Passthrough conversion for CPU tables - no conversion needed
impl<'i> Convert<Table<Raster<CPU>>, &'i WgpuExecutor> for Table<Raster<CPU>> {
	async fn convert(self, _: Footprint, _converter: &'i WgpuExecutor) -> Table<Raster<CPU>> {
		self
	}
}

/// Converts GPU raster table to CPU by downloading texture data in one go
///
/// then asynchronously maps all buffers and processes the results.
impl<'i> Convert<Table<Raster<CPU>>, &'i WgpuExecutor> for Table<Raster<GPU>> {
	async fn convert(self, _: Footprint, executor: &'i WgpuExecutor) -> Table<Raster<CPU>> {
		let device = &executor.context.device;
		let queue = &executor.context.queue;

		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("batch_texture_download_encoder"),
		});

		let mut converters = Vec::new();
		let mut rows_meta = Vec::new();

		for row in self {
			let gpu_raster = row.element;
			converters.push(RasterGpuToRasterCpuConverter::new(device, &mut encoder, gpu_raster));
			rows_meta.push(TableRow {
				element: (),
				transform: row.transform,
				alpha_blending: row.alpha_blending,
				source_node_id: row.source_node_id,
			});
		}

		queue.submit([encoder.finish()]);

		let mut map_futures = Vec::new();
		for converter in converters {
			map_futures.push(converter.convert());
		}

		let map_results = futures::future::try_join_all(map_futures)
			.await
			.map_err(|_| "Failed to receive map result")
			.expect("Buffer mapping communication failed");

		map_results
			.into_iter()
			.zip(rows_meta.into_iter())
			.map(|(element, row)| TableRow {
				element,
				transform: row.transform,
				alpha_blending: row.alpha_blending,
				source_node_id: row.source_node_id,
			})
			.collect()
	}
}

/// Converts single GPU raster to CPU by downloading texture data
impl<'i> Convert<Raster<CPU>, &'i WgpuExecutor> for Raster<GPU> {
	async fn convert(self, _: Footprint, executor: &'i WgpuExecutor) -> Raster<CPU> {
		let device = &executor.context.device;
		let queue = &executor.context.queue;

		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("single_texture_download_encoder"),
		});

		let converter = RasterGpuToRasterCpuConverter::new(device, &mut encoder, self);

		queue.submit([encoder.finish()]);

		converter.convert().await.expect("Failed to download texture data")
	}
}

/// Node for uploading textures from CPU to GPU. This Is now deprecated and
/// we should use the Convert node in the future.
///
/// Accepts either individual rasters or tables of rasters and converts them
/// to GPU format using the WgpuExecutor's device and queue.
#[node_macro::node(category(""))]
pub async fn upload_texture<'a: 'n, T: Convert<Table<Raster<GPU>>, &'a WgpuExecutor>>(
	_: impl Ctx,
	#[implementations(Table<Raster<CPU>>, Table<Raster<GPU>>)] input: T,
	executor: &'a WgpuExecutor,
) -> Table<Raster<GPU>> {
	input.convert(Footprint::DEFAULT, executor).await
}
