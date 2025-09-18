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

impl<'i> Convert<Table<Raster<GPU>>, &'i WgpuExecutor> for Table<Raster<GPU>> {
	async fn convert(self, _: Footprint, _converter: &'i WgpuExecutor) -> Table<Raster<GPU>> {
		self
	}
}
impl<'i> Convert<Table<Raster<GPU>>, &'i WgpuExecutor> for Table<Raster<CPU>> {
	async fn convert(self, _: Footprint, executor: &'i WgpuExecutor) -> Table<Raster<GPU>> {
		let device = &executor.context.device;
		let queue = &executor.context.queue;
		let table = self
			.iter()
			.map(|row| {
				let image = row.element;
				let rgba8_data: Vec<SRGBA8> = image.data.iter().map(|x| (*x).into()).collect();

				let texture = device.create_texture_with_data(
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
						// I don't know what usages are actually necessary
						usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC,
						view_formats: &[],
					},
					TextureDataOrder::LayerMajor,
					bytemuck::cast_slice(rgba8_data.as_slice()),
				);

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
impl<'i> Convert<Table<Raster<CPU>>, &'i WgpuExecutor> for Table<Raster<CPU>> {
	async fn convert(self, _: Footprint, _converter: &'i WgpuExecutor) -> Table<Raster<CPU>> {
		self
	}
}
impl<'i> Convert<Table<Raster<CPU>>, &'i WgpuExecutor> for Table<Raster<GPU>> {
	async fn convert(self, _: Footprint, executor: &'i WgpuExecutor) -> Table<Raster<CPU>> {
		let device = &executor.context.device;
		let queue = &executor.context.queue;

		// Create a single command encoder for all copy operations
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("batch_texture_download_encoder"),
		});

		// Collect all buffer and texture info for batch processing
		let mut buffers_and_info = Vec::new();

		for row in self.iter() {
			let gpu_raster = row.element;
			let texture = gpu_raster.data();

			// Get texture dimensions
			let width = texture.width();
			let height = texture.height();
			let bytes_per_pixel = 4; // RGBA8
			let buffer_size = (width * height * bytes_per_pixel) as u64;

			// Create a buffer to copy texture data to
			let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
				label: Some("texture_download_buffer"),
				size: buffer_size,
				usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
				mapped_at_creation: false,
			});

			// Add copy operation to the batch encoder
			encoder.copy_texture_to_buffer(
				wgpu::TexelCopyTextureInfo {
					texture,
					mip_level: 0,
					origin: wgpu::Origin3d::ZERO,
					aspect: wgpu::TextureAspect::All,
				},
				wgpu::TexelCopyBufferInfo {
					buffer: &output_buffer,
					layout: wgpu::TexelCopyBufferLayout {
						offset: 0,
						bytes_per_row: Some(width * bytes_per_pixel),
						rows_per_image: Some(height),
					},
				},
				Extent3d {
					width,
					height,
					depth_or_array_layers: 1,
				},
			);

			buffers_and_info.push((output_buffer, width, height, *row.transform, *row.alpha_blending, *row.source_node_id));
		}

		// Submit all copy operations in a single batch
		queue.submit([encoder.finish()]);

		// Now async map all buffers and collect futures
		let mut map_futures = Vec::new();
		for (buffer, _width, _height, _transform, _alpha_blending, _source_node_id) in &buffers_and_info {
			let buffer_slice = buffer.slice(..);
			let (sender, receiver) = futures::channel::oneshot::channel();
			buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
				let _ = sender.send(result);
			});
			map_futures.push(receiver);
		}

		// Wait for all mapping operations to complete
		let map_results = futures::future::try_join_all(map_futures).await.map_err(|_| "Failed to receive map result").unwrap();

		// Process all mapped buffers
		let mut table = Vec::new();
		for (i, (buffer, width, height, transform, alpha_blending, source_node_id)) in buffers_and_info.into_iter().enumerate() {
			if let Err(e) = &map_results[i] {
				panic!("Buffer mapping failed: {:?}", e);
			}

			let data = buffer.slice(..).get_mapped_range();
			// Convert bytes directly to Color via SRGBA8
			let cpu_data: Vec<Color> = data
				.chunks_exact(4)
				.map(|chunk| {
					// Create SRGBA8 from bytes, then convert to Color
					Color::from_rgba8_srgb(chunk[0], chunk[1], chunk[2], chunk[3])
				})
				.collect();

			drop(data);
			buffer.unmap();
			let cpu_image = Image {
				data: cpu_data,
				width,
				height,
				base64_string: None,
			};
			let cpu_raster = Raster::new_cpu(cpu_image);

			table.push(TableRow {
				element: cpu_raster,
				transform,
				alpha_blending,
				source_node_id,
			});
		}

		table.into_iter().collect()
	}
}

#[node_macro::node(category(""))]
pub async fn upload_texture<'a: 'n, T: Convert<Table<Raster<GPU>>, &'a WgpuExecutor>>(
	_: impl Ctx,
	#[implementations(Table<Raster<CPU>>, Table<Raster<GPU>>)] input: T,
	executor: &'a WgpuExecutor,
) -> Table<Raster<GPU>> {
	input.convert(Footprint::DEFAULT, executor).await
}
