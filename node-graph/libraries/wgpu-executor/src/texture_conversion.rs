use crate::WgpuExecutor;
use core_types::Color;
use core_types::Ctx;
use core_types::color::SRGBA8;
use core_types::list::{Item, List};
use core_types::ops::Convert;
use core_types::transform::Footprint;
use raster_types::Image;
use raster_types::{CPU, GPU, Raster};
use wgpu::util::{DeviceExt, TextureDataOrder};
use wgpu::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};

/// Uploads CPU image data to a GPU texture
///
/// Creates a new WGPU texture with RGBA8UnormSrgb format and uploads the provided
/// image data. The texture is configured for binding, copying, and source operations.
fn upload_to_texture(device: &wgpu::Device, queue: &wgpu::Queue, image: &Raster<CPU>) -> wgpu::Texture {
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


/// Passthrough conversion for GPU `List`s - no conversion needed
impl<'i> Convert<List<Raster<GPU>>, &'i WgpuExecutor> for List<Raster<GPU>> {
	fn convert(self, _: Footprint, _converter: &'i WgpuExecutor) -> List<Raster<GPU>> {
		self
	}
}

/// Converts a `List<Raster<CPU>>` to `List<Raster<GPU>>` by uploading each image to a texture
impl<'i> Convert<List<Raster<GPU>>, &'i WgpuExecutor> for List<Raster<CPU>> {
	fn convert(self, _: Footprint, executor: &'i WgpuExecutor) -> List<Raster<GPU>> {
		let device = &executor.context().device;
		let queue = executor.context().queue.lock();
		let list = self
			.into_iter()
			.map(|row| {
				let (image, attributes) = row.into_parts();
				let texture = upload_to_texture(device, &queue, &image);

				Item::from_parts(Raster::new_gpu(texture), attributes)
			})
			.collect();

		queue.submit([]);
		list
	}
}

/// Converts single CPU raster to GPU by uploading to texture
impl<'i> Convert<Raster<GPU>, &'i WgpuExecutor> for Raster<CPU> {
	fn convert(self, _: Footprint, executor: &'i WgpuExecutor) -> Raster<GPU> {
		let device = &executor.context().device;
		let queue = executor.context().queue.lock();
		let texture = upload_to_texture(device, &queue, &self);

		queue.submit([]);
		Raster::new_gpu(texture)
	}
}

/// Passthrough conversion for CPU `List`s - no conversion needed
impl<'i> Convert<List<Raster<CPU>>, &'i WgpuExecutor> for List<Raster<CPU>> {
	fn convert(self, _: Footprint, _converter: &'i WgpuExecutor) -> List<Raster<CPU>> {
		self
	}
}

/// Uploads an raster texture from the CPU to the GPU. This is now deprecated and the Convert node should be used in the future.
///
/// Accepts either individual raster data or a `List` of raster elements and converts it to the GPU format using the WgpuExecutor's device and queue.
#[node_macro::node(category(""))]
pub fn upload_texture<'a, T: Convert<List<Raster<GPU>>, &'a WgpuExecutor>>(
	_: impl Ctx,
	#[implementations(List<Raster<CPU>>, List<Raster<GPU>>)] input: T,
	executor: &'a WgpuExecutor,
) -> List<Raster<GPU>> {
	input.convert(Footprint::DEFAULT, executor)
}
