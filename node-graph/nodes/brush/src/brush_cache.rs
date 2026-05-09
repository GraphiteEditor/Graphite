use crate::brush_stroke::BrushStroke;
use crate::brush_stroke::BrushStyle;
use core_types::ATTR_TRANSFORM;
use core_types::graphene_hash::CacheHashWrapper;
use core_types::list::Item;
use raster_types::CPU;
use raster_types::Raster;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
struct BrushCacheImpl {
	// The full previous input that was cached.
	prev_input: Vec<BrushStroke>,

	// The strokes that have been fully processed and blended into the background.
	background: Item<Raster<CPU>>,
	blended_image: Item<Raster<CPU>>,
	last_stroke_texture: Item<Raster<CPU>>,

	// A cache for brush textures.
	brush_texture_cache: HashMap<CacheHashWrapper<BrushStyle>, Raster<CPU>>,
}

impl BrushCacheImpl {
	fn compute_brush_plan(&mut self, mut background: Item<Raster<CPU>>, input: &[BrushStroke]) -> BrushPlan {
		// Do background invalidation.
		if background != self.background {
			self.background = background.clone();
			return BrushPlan {
				strokes: input.to_vec(),
				background,
				..Default::default()
			};
		}

		// Do blended_image invalidation.
		let blended_strokes = &self.prev_input[..self.prev_input.len().saturating_sub(1)];
		let num_blended_strokes = blended_strokes.len();
		if input.get(..num_blended_strokes) != Some(blended_strokes) {
			return BrushPlan {
				strokes: input.to_vec(),
				background,
				..Default::default()
			};
		}

		// Take our previous blended image (and invalidate the cache).
		// Since we're about to replace our cache anyway, this saves a clone.
		background = std::mem::take(&mut self.blended_image);

		// Check if the first non-blended stroke is an extension of the last one.
		// Transform is set to ZERO (not the default IDENTITY) as a sentinel to mark this item as uninitialized.
		let mut first_stroke_texture = Item::new_from_element(Raster::<CPU>::default()).with_attribute(ATTR_TRANSFORM, glam::DAffine2::ZERO);
		let mut first_stroke_point_skip = 0;
		let strokes = input[num_blended_strokes..].to_vec();
		if !strokes.is_empty() && self.prev_input.len() > num_blended_strokes {
			let last_stroke = &self.prev_input[num_blended_strokes];
			let same_style = strokes[0].style == last_stroke.style;
			let prev_points = last_stroke.compute_blit_points();
			let new_points = strokes[0].compute_blit_points();
			let is_point_prefix = new_points.get(..prev_points.len()) == Some(&prev_points);
			if same_style && is_point_prefix {
				first_stroke_texture = std::mem::take(&mut self.last_stroke_texture);
				first_stroke_point_skip = prev_points.len();
			}
		}

		self.prev_input = Vec::new();
		BrushPlan {
			strokes,
			background,
			first_stroke_texture,
			first_stroke_point_skip,
		}
	}

	pub fn cache_results(&mut self, input: Vec<BrushStroke>, blended_image: Item<Raster<CPU>>, last_stroke_texture: Item<Raster<CPU>>) {
		self.prev_input = input;
		self.blended_image = blended_image;
		self.last_stroke_texture = last_stroke_texture;
	}
}

#[derive(Clone, Debug, Default)]
pub struct BrushPlan {
	pub strokes: Vec<BrushStroke>,
	pub background: Item<Raster<CPU>>,
	pub first_stroke_texture: Item<Raster<CPU>>,
	pub first_stroke_point_skip: usize,
}

#[derive(Debug, Default, Clone)]
pub struct BrushCache(Arc<Mutex<BrushCacheImpl>>);

impl BrushCache {
	pub fn compute_brush_plan(&self, background: Item<Raster<CPU>>, input: &[BrushStroke]) -> BrushPlan {
		let mut inner = self.0.lock().unwrap();
		inner.compute_brush_plan(background, input)
	}

	pub fn cache_results(&self, input: Vec<BrushStroke>, blended_image: Item<Raster<CPU>>, last_stroke_texture: Item<Raster<CPU>>) {
		let mut inner = self.0.lock().unwrap();
		inner.cache_results(input, blended_image, last_stroke_texture)
	}

	pub fn get_cached_brush(&self, style: &BrushStyle) -> Option<Raster<CPU>> {
		let inner = self.0.lock().unwrap();
		inner.brush_texture_cache.get(&CacheHashWrapper(style.clone())).cloned()
	}

	pub fn store_brush(&self, style: BrushStyle, brush: Raster<CPU>) {
		let mut inner = self.0.lock().unwrap();
		inner.brush_texture_cache.insert(CacheHashWrapper(style), brush);
	}
}
