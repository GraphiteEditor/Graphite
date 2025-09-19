use crate::WgpuExecutor;
use glam::DAffine2;
use graphene_core::Graphic;
use graphene_core::ops::Convert;
use graphene_core::raster_types::{GPU, Raster};
use graphene_core::table::Table;
use graphene_core::transform::Footprint;
use graphene_core::vector::Vector;
use graphene_svg_renderer::{Render, RenderOutputType, RenderParams};
use wgpu::{CommandEncoderDescriptor, TextureFormat, TextureViewDescriptor};

/// Converts Table<Vector> to Table<Raster<GPU>> by rendering each vector to Vello scene and then to texture
impl<'i> Convert<Table<Raster<GPU>>, &'i WgpuExecutor> for Table<Vector> {
	async fn convert(self, footprint: Footprint, executor: &'i WgpuExecutor) -> Table<Raster<GPU>> {
		// Create render parameters for Vello rendering
		let render_params = RenderParams {
			render_mode: graphene_core::vector::style::RenderMode::Normal,
			hide_artboards: false,
			for_export: false,
			render_output_type: RenderOutputType::Vello,
			footprint,
			..Default::default()
		};
		log::debug!("rasterizing vector data with footprint {footprint:?}");

		let vector = &self;
		log::debug!("{vector:?}");

		// Create a Vello scene for this vector
		let mut scene = vello::Scene::new();
		let mut context = crate::RenderContext::default();

		// Render the vector to the Vello scene with the row's transform
		vector.render_to_vello(&mut scene, footprint.transform, &mut context, &render_params);

		// Render the scene to a GPU texture
		let resolution = footprint.resolution;
		let background = graphene_core::Color::TRANSPARENT;

		// Use async rendering to get the texture
		let texture = executor
			.render_vello_scene_to_texture(&scene, resolution, &context, background)
			.await
			.expect("Failed to render Vello scene to texture");

		let device = &executor.context.device;
		let queue = &executor.context.queue;
		let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
		let blitter = wgpu::util::TextureBlitter::new(device, TextureFormat::Rgba8UnormSrgb);
		let view = texture.create_view(&TextureViewDescriptor::default());
		let new_texture = device.create_texture(&wgpu::wgt::TextureDescriptor {
			label: None,
			size: wgpu::Extent3d {
				width: texture.width(),
				height: texture.height(),
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: TextureFormat::Rgba8UnormSrgb,
			view_formats: &[],
		});
		let new_view = new_texture.create_view(&TextureViewDescriptor::default());

		blitter.copy(device, &mut encoder, &view, &new_view);
		let command_buffer = encoder.finish();
		queue.submit([command_buffer]);

		let mut table = Table::new_from_element(Raster::new_gpu(new_texture));
		*(table.get_mut(0).as_mut().unwrap().transform) = footprint.transform.inverse() * DAffine2::from_scale((texture.width() as f64, texture.height() as f64).into());
		table
	}
}
