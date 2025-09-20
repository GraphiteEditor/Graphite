use crate::WgpuExecutor;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::ops::Convert;
use core_types::table::Table;
use core_types::transform::Footprint;
use glam::{DAffine2, DVec2, UVec2};
use graphic_types::Vector;
use graphic_types::raster_types::{GPU, Raster};
use rendering::{Render, RenderOutputType, RenderParams};
use wgpu::{CommandEncoderDescriptor, TextureFormat, TextureViewDescriptor};

/// Converts Table<Vector> to Table<Raster<GPU>> by rendering each vector to Vello scene and then to texture
impl<'i> Convert<Table<Raster<GPU>>, &'i WgpuExecutor> for Table<Vector> {
	async fn convert(self, footprint: Footprint, executor: &'i WgpuExecutor) -> Table<Raster<GPU>> {
		// Create render parameters for Vello rendering
		let render_params = RenderParams {
			render_mode: graphic_types::vector_types::vector::style::RenderMode::Normal,
			hide_artboards: false,
			for_export: false,
			render_output_type: RenderOutputType::Vello,
			footprint,
			..Default::default()
		};

		let vector = &self;
		let bounding_box = vector.bounding_box(DAffine2::IDENTITY, true);
		let RenderBoundingBox::Rectangle(rect) = bounding_box else {
			panic!("did not find valid bounding box")
		};

		// Create a Vello scene for this vector
		let mut scene = vello::Scene::new();
		let mut context = crate::RenderContext::default();

		let viewport_bounds = footprint.viewport_bounds_in_local_space();
		log::debug!("viewport bounds: {viewport_bounds:?}");
		log::debug!("vector bounds: {bounding_box:?}");

		let image_bounds = core_types::math::bbox::AxisAlignedBbox { start: rect[0], end: rect[1] };
		let intersection = viewport_bounds.intersect(&image_bounds);

		log::debug!("intersection: {intersection:?}");
		let size = intersection.size();

		// let offset = (intersection.start - image_bounds.start).max(DVec2::ZERO);
		let offset = (intersection.start - image_bounds.start).max(DVec2::ZERO) + image_bounds.start;
		log::debug!("size: {size} offset: {offset}");

		// If the image would not be visible, return an empty image
		if size.x <= 0. || size.y <= 0. {
			return Table::new();
		}

		let scale = footprint.scale();
		log::debug!("scale: {scale:?}");
		let width = (size.x * scale.x) as u32;
		let height = (size.y * scale.y) as u32;

		// Render the scene to a GPU texture
		let resolution = UVec2::new(width, height);
		log::debug!("resolution: {resolution:?}");
		let background = core_types::Color::TRANSPARENT;

		let render_transform = DAffine2::from_scale(scale) * DAffine2::from_translation(-offset);
		log::debug!("render transform: {render_transform:?}");
		// Render the vector to the Vello scene with the row's transform
		vector.render_to_vello(&mut scene, render_transform, &mut context, &render_params);

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
		*(table.get_mut(0).as_mut().unwrap().transform) = DAffine2::from_translation(offset) * DAffine2::from_scale(size);
		texture.destroy();
		table
	}
}
