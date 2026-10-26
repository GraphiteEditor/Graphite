use crate::{Buffer, WgpuExecutor};
use core_types::Color;
use core_types::color::SRGBA8;
use core_types::list::{Item, List};
use core_types::ops::{Convert, ConvertAsync};
use core_types::transform::Footprint;
use half::f16;
use raster_types::Image;
use raster_types::{CPU, GPU, Raster, Storage};
use wgpu::Extent3d;

/// Uploads CPU image data to a GPU texture
fn upload_to_texture(executor: &WgpuExecutor, queue: &wgpu::Queue, image: &Raster<CPU>) -> Raster<GPU> {
	if image.is_empty() {
		return Raster::default();
	}

	let format = executor.raster_format();
	let texture = executor.request_texture_with_format(glam::UVec2::new(image.width, image.height), format);
	let half_floats: Vec<[f16; 4]>;
	let (bytes, bytes_per_pixel) = if format == crate::FALLBACK_RASTER_FORMAT {
		half_floats = image.data.iter().map(|color| color.to_vec4().to_array().map(f16::from_f32)).collect();
		(bytemuck::cast_slice(half_floats.as_slice()), 8)
	} else {
		(bytemuck::cast_slice(image.data.as_slice()), 16)
	};
	queue.write_texture(
		texture.as_image_copy(),
		bytes,
		wgpu::TexelCopyBufferLayout {
			offset: 0,
			bytes_per_row: Some(bytes_per_pixel * image.width),
			rows_per_image: Some(image.height),
		},
		Extent3d {
			width: image.width,
			height: image.height,
			depth_or_array_layers: 1,
		},
	);
	Raster::new_gpu(texture)
}

/// Converts a Raster<GPU> texture to Raster<CPU> by downloading the underlying texture data.
///
/// Assumptions:
/// - 2D texture, mip level 0
/// - 4 bytes-per-pixel RGBA8, or `RASTER_FORMAT` or `FALLBACK_RASTER_FORMAT`
/// - Texture has COPY_SRC usage
struct RasterGpuToRasterCpuConverter {
	buffer: Buffer,
	width: u32,
	height: u32,
	format: wgpu::TextureFormat,
	unpadded_bytes_per_row: u32,
	padded_bytes_per_row: u32,
	_source: raster_types::Texture,
}
impl RasterGpuToRasterCpuConverter {
	fn new(executor: &WgpuExecutor, encoder: &mut wgpu::CommandEncoder, data_gpu: Raster<GPU>) -> Option<Self> {
		let texture = data_gpu.texture()?;
		let width = texture.width();
		let height = texture.height();
		let format = texture.format();
		let bytes_per_pixel = match format {
			crate::RASTER_FORMAT => 16,
			crate::FALLBACK_RASTER_FORMAT => 8,
			_ => 4,
		};
		let unpadded_bytes_per_row = width * bytes_per_pixel;
		let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
		let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
		let buffer_size = padded_bytes_per_row as u64 * height as u64;

		let buffer = executor.create_buffer(&wgpu::BufferDescriptor {
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

		Some(Self {
			buffer,
			width,
			height,
			format,
			unpadded_bytes_per_row,
			padded_bytes_per_row,
			// Keep source texture alive
			_source: texture.clone(),
		})
	}

	async fn convert(self, device: &wgpu::Device) -> Result<Raster<CPU>, wgpu::BufferAsyncError> {
		let buffer_slice = self.buffer.slice(..);
		let (sender, receiver) = futures::channel::oneshot::channel();
		buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
			let _ = sender.send(result);
		});

		#[cfg(not(target_family = "wasm"))]
		let map_result = {
			let _ = device.poll(wgpu::wgt::PollType::wait_indefinitely());
			receiver.await
		};

		// A WebGL2 fence only signals once control returns to the browser's event loop, so a wait can't block until the map completes
		#[cfg(target_family = "wasm")]
		let map_result = {
			let mut receiver = receiver;
			loop {
				let _ = device.poll(wgpu::wgt::PollType::Poll);
				match receiver.try_recv() {
					Ok(Some(result)) => break Ok(result),
					Ok(None) => {
						if !yield_to_event_loop().await {
							break Err(futures::channel::oneshot::Canceled);
						}
					}
					Err(canceled) => break Err(canceled),
				}
			}
		};

		map_result.expect("Failed to receive map result")?;

		let view = buffer_slice.get_mapped_range().expect("Failed to get mapped range");

		let row_stride = self.padded_bytes_per_row as usize;
		let row_bytes = self.unpadded_bytes_per_row as usize;
		let mut cpu_data: Vec<Color> = Vec::with_capacity((self.width * self.height) as usize);
		for row in 0..self.height as usize {
			let start = row * row_stride;
			let row_slice = &view[start..start + row_bytes];
			match self.format {
				crate::RASTER_FORMAT => cpu_data.extend(bytemuck::pod_collect_to_vec::<u8, Color>(row_slice)),
				crate::FALLBACK_RASTER_FORMAT => {
					let pixels = bytemuck::pod_collect_to_vec::<u8, [f16; 4]>(row_slice);
					cpu_data.extend(pixels.iter().map(|pixel| {
						let [red, green, blue, alpha] = pixel.map(f16::to_f32);
						Color::from_rgbaf32_unchecked(red, green, blue, alpha)
					}));
				}
				_ => {
					for px in row_slice.chunks_exact(4) {
						cpu_data.push(SRGBA8::new(px[0], px[1], px[2], px[3]).into());
					}
				}
			}
		}

		drop(view);
		self.buffer.unmap();
		let cpu_image = Image {
			data: cpu_data,
			width: self.width,
			height: self.height,
		};

		Ok(Raster::new_cpu(cpu_image))
	}
}

/// Resumes in a later task of the browser's event loop, returning false if one couldn't be scheduled.
#[cfg(target_family = "wasm")]
async fn yield_to_event_loop() -> bool {
	use futures::task::AtomicWaker;
	use std::sync::Arc;
	use std::sync::atomic::{AtomicBool, Ordering};
	use web_sys::wasm_bindgen::JsCast;
	use web_sys::wasm_bindgen::closure::Closure;

	#[derive(Default)]
	struct Timeout {
		fired: AtomicBool,
		waker: AtomicWaker,
	}
	let timeout = Arc::new(Timeout::default());

	// Only the shared flag is held across the wait below, keeping the future `Send` as `ConvertAsync` requires
	let scheduled = {
		let timeout = Arc::clone(&timeout);
		let callback = Closure::once_into_js(move || {
			timeout.fired.store(true, Ordering::Release);
			timeout.waker.wake();
		});
		web_sys::window().is_some_and(|window| window.set_timeout_with_callback(callback.unchecked_ref()).is_ok())
	};
	if !scheduled {
		return false;
	}

	std::future::poll_fn(|context| {
		timeout.waker.register(context.waker());
		if timeout.fired.load(Ordering::Acquire) {
			std::task::Poll::Ready(true)
		} else {
			std::task::Poll::Pending
		}
	})
	.await
}

/// Passthrough conversion for GPU `List`s - no conversion needed
impl Convert<List<Raster<GPU>>, WgpuExecutor> for List<Raster<GPU>> {
	fn convert(self, _: Footprint, _converter: WgpuExecutor) -> List<Raster<GPU>> {
		self
	}
}

/// Converts a `List<Raster<CPU>>` to `List<Raster<GPU>>` by uploading each image to a texture
impl Convert<List<Raster<GPU>>, WgpuExecutor> for List<Raster<CPU>> {
	fn convert(self, _: Footprint, executor: WgpuExecutor) -> List<Raster<GPU>> {
		let queue = executor.context().queue.lock();
		let list = self
			.into_iter()
			.map(|row| {
				let (image, attributes) = row.into_parts();
				Item::from_parts(upload_to_texture(&executor, &queue, &image), attributes)
			})
			.collect();

		queue.submit([]);
		list
	}
}

/// Converts single CPU raster to GPU by uploading to texture
impl Convert<Raster<GPU>, WgpuExecutor> for Raster<CPU> {
	fn convert(self, _: Footprint, executor: WgpuExecutor) -> Raster<GPU> {
		let queue = executor.context().queue.lock();
		let raster = upload_to_texture(&executor, &queue, &self);

		queue.submit([]);
		raster
	}
}

/// Passthrough conversion for CPU `List`s - no conversion needed
impl Convert<List<Raster<CPU>>, WgpuExecutor> for List<Raster<CPU>> {
	fn convert(self, _: Footprint, _converter: WgpuExecutor) -> List<Raster<CPU>> {
		self
	}
}

/// Converts a `List<Raster<GPU>>` to `List<Raster<CPU>>` by downloading texture data in one go then asynchronously maps all buffers and processes the results.
impl ConvertAsync<List<Raster<CPU>>, WgpuExecutor> for List<Raster<GPU>> {
	async fn convert(self, _: Footprint, executor: WgpuExecutor) -> List<Raster<CPU>> {
		let device = &executor.context().device;
		let queue = &executor.context().queue;

		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("batch_texture_download_encoder"),
		});

		let mut converters = Vec::new();
		let mut rows_meta = Vec::new();

		for row in self {
			let (element, attributes) = row.into_parts();
			converters.push(RasterGpuToRasterCpuConverter::new(&executor, &mut encoder, element));
			rows_meta.push(Item::from_parts((), attributes));
		}

		queue.submit([encoder.finish()]);

		let mut map_futures = Vec::new();
		for converter in converters {
			map_futures.push(async move {
				match converter {
					Some(converter) => converter.convert(device).await,
					None => Ok(Raster::default()),
				}
			});
		}

		let map_results = futures::future::try_join_all(map_futures)
			.await
			.map_err(|_| "Failed to receive map result")
			.expect("Buffer mapping communication failed");

		map_results
			.into_iter()
			.zip(rows_meta)
			.map(|(element, row)| {
				let (_, attributes) = row.into_parts();
				Item::from_parts(element, attributes)
			})
			.collect()
	}
}

/// Converts single GPU raster to CPU by downloading texture data
impl ConvertAsync<Raster<CPU>, WgpuExecutor> for Raster<GPU> {
	async fn convert(self, _: Footprint, executor: WgpuExecutor) -> Raster<CPU> {
		let device = &executor.context().device;
		let queue = &executor.context().queue;

		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("single_texture_download_encoder"),
		});

		let converter = RasterGpuToRasterCpuConverter::new(&executor, &mut encoder, self);

		queue.submit([encoder.finish()]);

		let Some(converter) = converter else { return Raster::default() };
		converter.convert(device).await.expect("Failed to download texture data")
	}
}
