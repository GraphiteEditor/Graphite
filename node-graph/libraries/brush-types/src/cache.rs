//! Opaque render state cached per footprint.
//!
//! ```ignore
//! let state: SomeState = cache.take(ctx.footprint()).unwrap_or_default();
//! // ...render, freely mutating the state
//! cache.store(ctx.footprint(), state);
//! ```

use core_types::transform::Footprint;
use glam::DMat2;
use std::sync::{Arc, Mutex};

const STALE_EPOCHS: u64 = 2;
const MAX_VIEWS: usize = 3;

#[derive(Clone)]
pub struct BrushCache {
	state: Arc<Mutex<State>>,
	nonce: u64, // Avoid deduplication of cache entries across different brush nodes.
}

impl Default for BrushCache {
	fn default() -> Self {
		Self {
			state: Default::default(),
			nonce: core_types::uuid::generate_uuid(),
		}
	}
}

impl BrushCache {
	pub fn take<S: std::any::Any + Send + Sync>(&self, footprint: &Footprint) -> Option<S> {
		let state = self.state.lock().unwrap().take(footprint);
		state.and_then(|state| state.downcast().ok()).map(|state| *state)
	}

	pub fn store<S: std::any::Any + Send + Sync>(&self, footprint: &Footprint, state: S) {
		self.state.lock().unwrap().store(footprint, Box::new(state));
	}
}

impl PartialEq for BrushCache {
	fn eq(&self, _: &Self) -> bool {
		true
	}
}

impl std::fmt::Debug for BrushCache {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("BrushCache").field("slots", &self.state.lock().unwrap().slots.len()).finish()
	}
}

impl core_types::CacheHash for BrushCache {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		state.write_u64(self.nonce);
	}
}

unsafe impl dyn_any::StaticType for BrushCache {
	type Static = BrushCache;
}

#[cfg(feature = "serde")]
impl serde::Serialize for BrushCache {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_unit()
	}
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BrushCache {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		serde::de::IgnoredAny::deserialize(deserializer)?;
		Ok(Self::default())
	}
}

type BoxedData = Box<dyn std::any::Any + Send + Sync>;

#[derive(Default)]
struct State {
	epoch: u64,
	slots: Vec<Slot>,
}

struct Slot {
	footprint: Footprint,
	epoch: u64,
	data: BoxedData,
}

impl Slot {
	fn view(&self) -> DMat2 {
		self.footprint.transform.matrix2
	}
}

impl State {
	fn take(&mut self, footprint: &Footprint) -> Option<BoxedData> {
		self.touch(footprint.transform.matrix2);
		let index = self.slots.iter().position(|slot| slot.footprint == *footprint);
		let hit = index.map(|index| {
			let slot = self.slots.remove(index);
			if slot.epoch == self.epoch {
				self.epoch += 1;
			}
			slot.data
		});
		self.retire();
		hit
	}

	fn store(&mut self, footprint: &Footprint, data: BoxedData) {
		self.touch(footprint.transform.matrix2);
		self.slots.retain(|slot| slot.footprint != *footprint);
		self.slots.push(Slot {
			footprint: *footprint,
			epoch: self.epoch,
			data,
		});
		self.retire();
	}

	fn touch(&mut self, view: DMat2) {
		self.slots.sort_by_key(|slot| slot.view() == view);
	}

	fn retire(&mut self) {
		let epoch = self.epoch;
		self.slots.retain(|slot| epoch - slot.epoch < STALE_EPOCHS);
		while self.slots.chunk_by(|a, b| a.view() == b.view()).count() > MAX_VIEWS {
			let front = self.slots[0].view();
			let group = self.slots.iter().take_while(|slot| slot.view() == front).count();
			self.slots.drain(..group.max(1));
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::transform::RenderQuality;
	use glam::{DAffine2, DVec2, UVec2};

	struct Dummy;

	fn view(zoom: f64, rotation: f64, pan: DVec2) -> Footprint {
		Footprint {
			transform: DAffine2::from_scale_angle_translation(DVec2::splat(zoom), rotation, pan),
			resolution: UVec2::new(1920, 1080),
			quality: RenderQuality::Full,
		}
	}

	fn thumbnail(zoom: f64) -> Footprint {
		Footprint {
			resolution: UVec2::new(150, 150),
			..view(zoom, 0., DVec2::ZERO)
		}
	}

	fn live(cache: &BrushCache) -> usize {
		cache.state.lock().unwrap().slots.len()
	}

	fn render(cache: &BrushCache, footprint: &Footprint) -> bool {
		let hit = cache.take::<Dummy>(footprint).is_some();
		cache.store(footprint, Dummy);
		hit
	}

	#[test]
	fn continuous_zoom_is_bounded_by_views() {
		let cache = BrushCache::default();
		for step in 0..100 {
			render(&cache, &view(1. + step as f64 * 0.01, 0., DVec2::ZERO));
		}
		assert!(live(&cache) <= MAX_VIEWS);
	}

	#[test]
	fn continuous_rotation_is_bounded_by_views() {
		let cache = BrushCache::default();
		for step in 0..100 {
			render(&cache, &view(2., step as f64 * 0.01, DVec2::ZERO));
		}
		assert!(live(&cache) <= MAX_VIEWS);
	}

	#[test]
	fn zooming_reclaims_pan_slots() {
		let cache = BrushCache::default();
		for step in 0..30 {
			render(&cache, &view(1., 0., DVec2::splat(step as f64 * 100.)));
		}
		for step in 1..=3 {
			render(&cache, &view(1. + step as f64, 0., DVec2::ZERO));
		}
		assert_eq!(live(&cache), 3);
	}

	#[test]
	fn frames_may_hold_many_footprints_per_view() {
		let cache = BrushCache::default();
		let footprints: Vec<_> = (0..5).map(|step| view(1., 0., DVec2::splat(step as f64 * 100.))).collect();
		for frame in 0..10 {
			for footprint in &footprints {
				assert_eq!(render(&cache, footprint), frame > 0, "footprint evicted while its frame still renders it");
			}
		}
		assert_eq!(live(&cache), 5);
	}

	#[test]
	fn thumbnail_drift_is_bounded_and_keeps_the_view() {
		let cache = BrushCache::default();
		for step in 0..100 {
			render(&cache, &thumbnail(1. + step as f64 * 0.001));
		}
		assert!(live(&cache) <= MAX_VIEWS);

		let viewport = view(2., 0., DVec2::ZERO);
		render(&cache, &viewport);
		for step in 0..50 {
			render(&cache, &thumbnail(2. + step as f64 * 0.001));
			assert!(render(&cache, &viewport), "thumbnail churn evicted the viewport slot");
		}
	}

	#[test]
	fn settled_view_retires_stale_slots() {
		let cache = BrushCache::default();
		for step in 0..3 {
			render(&cache, &view(1. + step as f64, 0., DVec2::ZERO));
		}
		assert_eq!(live(&cache), 3);
		for _ in 0..STALE_EPOCHS {
			render(&cache, &view(1., 0., DVec2::ZERO));
		}
		assert_eq!(live(&cache), 1);
	}
}
