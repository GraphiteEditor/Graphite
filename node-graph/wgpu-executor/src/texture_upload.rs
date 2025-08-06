use crate::WgpuExecutor;
use graphene_core::color::SRGBA8;
use graphene_core::raster_types::{CPU, GPU, Raster};
use graphene_core::table::{Table, TableRow};
use graphene_core::{Ctx, ExtractFootprint};
use wgpu::util::{DeviceExt, TextureDataOrder};
use wgpu::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};

#[node_macro::node(category(""))]
pub async fn upload_texture<'a: 'n>(_: impl ExtractFootprint + Ctx, input: Table<Raster<CPU>>, executor: &'a WgpuExecutor) -> Table<Raster<GPU>> {
	let device = &executor.context.device;
	let queue = &executor.context.queue;
	let table = input
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
