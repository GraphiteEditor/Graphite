use crate::WgpuExecutor;
use core_types::ops::ConvertAsync;
use core_types::transform::Footprint;
use raster_types::{CPU, Raster, Texture};
use rendering::deferred_image::{EffectMap, EffectMapPlacement, MAX_DEFERRED_IMAGE_PIXELS, substitute_effect_map};

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
}
