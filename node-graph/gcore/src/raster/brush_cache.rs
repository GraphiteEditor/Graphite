use crate::Color;
use crate::graphene_core::raster::image::ImageFrameTable;
use crate::raster::Image;
use crate::vector::brush_stroke::BrushStroke;
use crate::vector::brush_stroke::BrushStyle;
use core::hash::Hash;
use dyn_any::DynAny;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, DynAny, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct BrushCacheImpl {
	// The full previous input that was cached.
	prev_input: Vec<BrushStroke>,

	// The strokes that have been fully processed and blended into the background.
	#[cfg_attr(feature = "serde", serde(deserialize_with = "crate::graphene_core::raster::image::migrate_image_frame"))]
	background: ImageFrameTable<Color>,
	#[cfg_attr(feature = "serde", serde(deserialize_with = "crate::graphene_core::raster::image::migrate_image_frame"))]
	blended_image: ImageFrameTable<Color>,
	#[cfg_attr(feature = "serde", serde(deserialize_with = "crate::graphene_core::raster::image::migrate_image_frame"))]
	last_stroke_texture: ImageFrameTable<Color>,

	// A cache for brush textures.
	#[cfg_attr(feature = "serde", serde(skip))]
	brush_texture_cache: HashMap<BrushStyle, Image<Color>>,
}

impl BrushCacheImpl {
	fn compute_brush_plan(&mut self, mut background: ImageFrameTable<Color>, input: &[BrushStroke]) -> BrushPlan {
		// Do background invalidation.
		if background.one_instance_ref().instance != self.background.one_instance_ref().instance {
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
		background = core::mem::take(&mut self.blended_image);

		// Check if the first non-blended stroke is an extension of the last one.
		let mut first_stroke_texture = ImageFrameTable::one_empty_image();
		let mut first_stroke_point_skip = 0;
		let strokes = input[num_blended_strokes..].to_vec();
		if !strokes.is_empty() && self.prev_input.len() > num_blended_strokes {
			let last_stroke = &self.prev_input[num_blended_strokes];
			let same_style = strokes[0].style == last_stroke.style;
			let prev_points = last_stroke.compute_blit_points();
			let new_points = strokes[0].compute_blit_points();
			let is_point_prefix = new_points.get(..prev_points.len()) == Some(&prev_points);
			if same_style && is_point_prefix {
				first_stroke_texture = core::mem::take(&mut self.last_stroke_texture);
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

	pub fn cache_results(&mut self, input: Vec<BrushStroke>, blended_image: ImageFrameTable<Color>, last_stroke_texture: ImageFrameTable<Color>) {
		self.prev_input = input;
		self.blended_image = blended_image;
		self.last_stroke_texture = last_stroke_texture;
	}
}

impl Hash for BrushCacheImpl {
	// Zero hash.
	fn hash<H: core::hash::Hasher>(&self, _state: &mut H) {}
}

#[derive(Clone, Debug, Default)]
pub struct BrushPlan {
	pub strokes: Vec<BrushStroke>,
	pub background: ImageFrameTable<Color>,
	pub first_stroke_texture: ImageFrameTable<Color>,
	pub first_stroke_point_skip: usize,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, DynAny)]
pub struct BrushCache {
	inner: Arc<Mutex<BrushCacheImpl>>,
	proto: bool,
}

impl Default for BrushCache {
	fn default() -> Self {
		Self::new_proto()
	}
}

// A bit of a cursed implementation to work around the current node system.
// The original object is a 'prototype' that when cloned gives you a independent
// new object. Any further clones however are all the same underlying cache object.
impl Clone for BrushCache {
	fn clone(&self) -> Self {
		if self.proto {
			let inner_val = self.inner.lock().unwrap();
			Self {
				inner: Arc::new(Mutex::new(inner_val.clone())),
				proto: false,
			}
		} else {
			Self {
				inner: Arc::clone(&self.inner),
				proto: false,
			}
		}
	}
}

impl PartialEq for BrushCache {
	fn eq(&self, other: &Self) -> bool {
		if Arc::ptr_eq(&self.inner, &other.inner) {
			return true;
		}

		let s = self.inner.lock().unwrap();
		let o = other.inner.lock().unwrap();

		*s == *o
	}
}

impl Hash for BrushCache {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.inner.lock().unwrap().hash(state);
	}
}

impl BrushCache {
	pub fn new_proto() -> Self {
		Self {
			inner: Default::default(),
			proto: true,
		}
	}

	pub fn compute_brush_plan(&self, background: ImageFrameTable<Color>, input: &[BrushStroke]) -> BrushPlan {
		let mut inner = self.inner.lock().unwrap();
		inner.compute_brush_plan(background, input)
	}

	pub fn cache_results(&self, input: Vec<BrushStroke>, blended_image: ImageFrameTable<Color>, last_stroke_texture: ImageFrameTable<Color>) {
		let mut inner = self.inner.lock().unwrap();
		inner.cache_results(input, blended_image, last_stroke_texture)
	}

	pub fn get_cached_brush(&self, style: &BrushStyle) -> Option<Image<Color>> {
		let inner = self.inner.lock().unwrap();
		inner.brush_texture_cache.get(style).cloned()
	}

	pub fn store_brush(&self, style: BrushStyle, brush: Image<Color>) {
		let mut inner = self.inner.lock().unwrap();
		inner.brush_texture_cache.insert(style, brush);
	}
}
