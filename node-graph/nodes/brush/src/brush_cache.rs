use crate::brush_stroke::BrushStroke;
use crate::brush_stroke::BrushStyle;
use core_types::ATTR_TRANSFORM;
use core_types::graphene_hash::CacheHashWrapper;
use core_types::table::TableRow;
use dyn_any::DynAny;
use raster_types::CPU;
use raster_types::Raster;
use std::collections::HashMap;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

// TODO: This is a temporary hack, be sure to not reuse this when the brush system is replaced/rewritten.
static NEXT_BRUSH_CACHE_IMPL_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct BrushCacheImpl {
	#[cfg_attr(feature = "serde", serde(default = "new_unique_id"))]
	unique_id: u64,
	// The full previous input that was cached.
	#[cfg_attr(feature = "serde", serde(default))]
	prev_input: Vec<BrushStroke>,

	// The strokes that have been fully processed and blended into the background.
	#[cfg_attr(feature = "serde", serde(default, deserialize_with = "raster_types::image::migrate_image_frame_row"))]
	background: TableRow<Raster<CPU>>,
	#[cfg_attr(feature = "serde", serde(default, deserialize_with = "raster_types::image::migrate_image_frame_row"))]
	blended_image: TableRow<Raster<CPU>>,
	#[cfg_attr(feature = "serde", serde(default, deserialize_with = "raster_types::image::migrate_image_frame_row"))]
	last_stroke_texture: TableRow<Raster<CPU>>,

	// A cache for brush textures.
	#[cfg_attr(feature = "serde", serde(skip))]
	brush_texture_cache: HashMap<CacheHashWrapper<BrushStyle>, Raster<CPU>>,
}

impl BrushCacheImpl {
	fn compute_brush_plan(&mut self, mut background: TableRow<Raster<CPU>>, input: &[BrushStroke]) -> BrushPlan {
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
		let mut first_stroke_texture = TableRow::new_from_element(Raster::<CPU>::default()).with_attribute(ATTR_TRANSFORM, glam::DAffine2::ZERO);
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

	pub fn cache_results(&mut self, input: Vec<BrushStroke>, blended_image: TableRow<Raster<CPU>>, last_stroke_texture: TableRow<Raster<CPU>>) {
		self.prev_input = input;
		self.blended_image = blended_image;
		self.last_stroke_texture = last_stroke_texture;
	}
}

impl Default for BrushCacheImpl {
	fn default() -> Self {
		Self {
			unique_id: new_unique_id(),
			prev_input: Vec::new(),
			background: Default::default(),
			blended_image: Default::default(),
			last_stroke_texture: Default::default(),
			brush_texture_cache: HashMap::new(),
		}
	}
}

impl PartialEq for BrushCacheImpl {
	fn eq(&self, other: &Self) -> bool {
		self.unique_id == other.unique_id
	}
}

impl Hash for BrushCacheImpl {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.unique_id.hash(state);
	}
}

fn new_unique_id() -> u64 {
	NEXT_BRUSH_CACHE_IMPL_ID.fetch_add(1, Ordering::SeqCst)
}

#[derive(Clone, Debug, Default)]
pub struct BrushPlan {
	pub strokes: Vec<BrushStroke>,
	pub background: TableRow<Raster<CPU>>,
	pub first_stroke_texture: TableRow<Raster<CPU>>,
	pub first_stroke_point_skip: usize,
}

#[derive(Debug, Default, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushCache(Arc<Mutex<BrushCacheImpl>>);

// A bit of a cursed implementation to work around the current node system.
// The original object is a 'prototype' that when cloned gives you a independent
// new object. Any further clones however are all the same underlying cache object.
impl Clone for BrushCache {
	fn clone(&self) -> Self {
		Self(Arc::new(Mutex::new(self.0.lock().unwrap().clone())))
	}
}

impl PartialEq for BrushCache {
	fn eq(&self, other: &Self) -> bool {
		if Arc::ptr_eq(&self.0, &other.0) {
			return true;
		}

		let s = self.0.lock().unwrap();
		let o = other.0.lock().unwrap();

		*s == *o
	}
}

impl Hash for BrushCache {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.0.lock().unwrap().hash(state);
	}
}

impl graphene_hash::CacheHash for BrushCache {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(&self.0.lock().unwrap().unique_id, state);
	}
}

impl BrushCache {
	pub fn compute_brush_plan(&self, background: TableRow<Raster<CPU>>, input: &[BrushStroke]) -> BrushPlan {
		let mut inner = self.0.lock().unwrap();
		inner.compute_brush_plan(background, input)
	}

	pub fn cache_results(&self, input: Vec<BrushStroke>, blended_image: TableRow<Raster<CPU>>, last_stroke_texture: TableRow<Raster<CPU>>) {
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
