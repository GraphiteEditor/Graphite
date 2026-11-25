use crate::WgpuExecutor;
use core_types::ops::ConvertAsync;
use core_types::transform::Footprint;
use raster_types::{CPU, Raster, Texture};
use rendering::CanvasImage;
use rendering::deferred_image::{DeferredImage, DeferredImagePlacement, EffectMap, EffectMapPlacement, MAX_DEFERRED_IMAGE_PIXELS, substitute_deferred_image, substitute_effect_map};

impl WgpuExecutor {
	/// Renders the maps of effects' coverages that an SVG of the view `footprint` takes, with where its filters place each.
	/// Maps out of view, or that fail to render, keep their placeholders.
	pub fn render_effect_maps(&self, effect_maps: &[EffectMap], footprint: Footprint) -> Vec<(EffectMapPlacement, Texture)> {
		let max_pixels = MAX_DEFERRED_IMAGE_PIXELS.min(self.context().device.limits().max_texture_dimension_2d);

		effect_maps
			.iter()
			.filter_map(|map| {
				let layout = map.layout(footprint.transform, footprint.resolution.as_dvec2(), max_pixels)?;
				match self.render_vello(layout.size, layout.document_to_pixels, &mut |scene| map.draw(scene, &layout)) {
					Ok(texture) => Some((layout.placement, texture)),
					Err(error) => {
						log::error!("Failed to render a map of an effect's coverage for an SVG filter: {error}");
						None
					}
				}
			})
			.collect()
	}

	/// Reads back the maps of effects' coverages, putting each in `svg` in place of its placeholder.
	pub async fn resolve_effect_maps(&self, svg: &mut String, maps: Vec<(EffectMapPlacement, Texture)>) {
		for (placement, texture) in maps {
			let map: Raster<CPU> = Raster::new_gpu(texture).convert(Footprint::BOUNDLESS, self.clone()).await;
			substitute_effect_map(svg, &placement, &map);
		}
	}

	/// Renders images of the content an SVG or PDF of the view `footprint` defers to the GPU, with where each is drawn.
	/// Content out of view, or that fails to render, gets no image.
	pub fn render_deferred_images(&self, deferred_images: &[DeferredImage], footprint: Footprint) -> Vec<(DeferredImagePlacement, Texture)> {
		let max_pixels = MAX_DEFERRED_IMAGE_PIXELS.min(self.context().device.limits().max_texture_dimension_2d);

		deferred_images
			.iter()
			.filter_map(|deferred| {
				let layout = deferred.layout(footprint.transform, footprint.resolution.as_dvec2(), max_pixels)?;
				match self.render_vello(layout.size, layout.document_to_pixels, &mut |scene| deferred.draw(scene, &layout)) {
					Ok(texture) => Some((layout.placement, texture)),
					Err(error) => {
						log::error!("Failed to render an image of content with effects SVG can't express: {error}");
						None
					}
				}
			})
			.collect()
	}

	/// Reads back the images of deferred content, putting each in `svg` in place of the markup standing in for its content.
	pub async fn resolve_deferred_svg_images(&self, svg: &mut String, image_data: &mut Vec<(u64, CanvasImage)>, images: Vec<(DeferredImagePlacement, Texture)>) {
		for (placement, image) in self.read_back_deferred_images(images).await {
			substitute_deferred_image(svg, image_data, &placement, &image);
		}
	}

	/// Reads back the images of deferred content into CPU memory, each with where it goes.
	pub async fn read_back_deferred_images(&self, images: Vec<(DeferredImagePlacement, Texture)>) -> Vec<(DeferredImagePlacement, Raster<CPU>)> {
		let mut read_back = Vec::with_capacity(images.len());
		for (placement, texture) in images {
			read_back.push((placement, Raster::new_gpu(texture).convert(Footprint::BOUNDLESS, self.clone()).await));
		}
		read_back
	}
}
