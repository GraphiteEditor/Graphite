use crate::WgpuExecutor;
use graphene_core::color::SRGBA8;
use graphene_core::instances::Instance;
use graphene_core::raster_types::{CPU, GPU, Raster, RasterDataTable};
use graphene_core::{Ctx, ExtractFootprint};
use wgpu::util::{DeviceExt, TextureDataOrder};
use wgpu::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};

#[node_macro::node(category(""))]
pub async fn upload_texture<'a: 'n>(_: impl ExtractFootprint + Ctx, input: RasterDataTable<CPU>, executor: &'a WgpuExecutor) -> RasterDataTable<GPU> {
	let device = &executor.context.device;
	let queue = &executor.context.queue;
	let instances = input
		.instance_ref_iter()
		.map(|instance| {
			let image = instance.instance;
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

			Instance {
				instance: Raster::new_gpu(texture.into()),
				transform: *instance.transform,
				alpha_blending: *instance.alpha_blending,
				source_node_id: *instance.source_node_id,
			}
		})
		.collect();

	queue.submit([]);
	instances
}
