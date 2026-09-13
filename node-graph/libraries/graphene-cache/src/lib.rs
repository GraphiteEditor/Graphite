use core_types::transform::Footprint;
use glam::DMat2;
use std::sync::{Arc, Mutex};

// =====
// Cache
// =====

/// A small keyed cache backed by a linear `Vec`.
/// It is not intended for many entries, so its `CachePolicy` must evict entries to keep the cache bounded.
pub struct Cache<K, M: CachePolicy<K>> {
	inner: Arc<Mutex<CacheInner<K, M>>>,
	nonce: u64, // Avoid deduplication of cache entries across different brush nodes.
}

impl<K: Copy + PartialEq, M: CachePolicy<K>> Cache<K, M> {
	/// Removes and returns the value stored for `key`.
	/// Returns `None` if the key is absent or the stored value has a different type.
	/// A type mismatch leaves the original value cached.
	pub fn take<S: std::any::Any + Send + Sync>(&self, key: &K) -> Option<S> {
		let mut guard = self.inner.lock().unwrap();
		guard.take::<S>(key)
	}

	/// Clones the value stored for `key` without removing it.
	/// Returns `None` if the key is absent or the stored value has a different type.
	/// Cloning occurs while the cache lock is held.
	pub fn get_cloned<S: std::any::Any + Send + Sync + Clone>(&self, key: &K) -> Option<S> {
		let mut guard = self.inner.lock().unwrap();
		guard.get_cloned(key)
	}

	/// Stores a value for `key`, replacing any existing value with the same key, regardless of its concrete type.
	pub fn store<S: std::any::Any + Send + Sync>(&self, key: &K, value: S) {
		self.inner.lock().unwrap().store(key, Box::new(value));
	}
}

impl<K, M: CachePolicy<K>> Default for Cache<K, M> {
	fn default() -> Self {
		Self {
			inner: Default::default(),
			nonce: core_types::uuid::generate_uuid(),
		}
	}
}

impl<K, M: CachePolicy<K>> Clone for Cache<K, M> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
			nonce: self.nonce,
		}
	}
}

impl<K, M: CachePolicy<K>> PartialEq for Cache<K, M> {
	fn eq(&self, _: &Self) -> bool {
		true
	}
}

impl<K, M: CachePolicy<K>> std::fmt::Debug for Cache<K, M> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Cache").field("entries", &self.inner.lock().unwrap().entries.len()).finish()
	}
}

impl<K, M: CachePolicy<K>> core_types::CacheHash for Cache<K, M> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		state.write_u64(self.nonce);
	}
}

unsafe impl<K: 'static, M: CachePolicy<K> + 'static> dyn_any::StaticType for Cache<K, M> {
	type Static = Cache<K, M>;
}

#[cfg(feature = "serde")]
impl<K, M: CachePolicy<K>> serde::Serialize for Cache<K, M> {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_unit()
	}
}

#[cfg(feature = "serde")]
impl<'de, K, M: CachePolicy<K>> serde::Deserialize<'de> for Cache<K, M> {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		serde::de::IgnoredAny::deserialize(deserializer)?;
		Ok(Self::default())
	}
}

// ===================
// CachePolicy & Entry
// ===================

/// Defines the policy state and lifecycle hooks used to manage cached entries.
pub trait CachePolicy<K>: Sized {
	/// State shared by all entries in one cache.
	type PolicyState: Default;
	/// Policy-specific state stored with each cache entry.
	type EntryState: Default;

	/// Updates entry ordering or policy state before accessing `key`.
	fn touch(key: &K, entries: &mut Vec<Entry<K, Self>>, policy_state: &mut Self::PolicyState);
	/// Removes entries that should no longer be retained.
	fn retire(entries: &mut Vec<Entry<K, Self>>, policy_state: &mut Self::PolicyState);
	/// Updates policy state when an entry is accessed successfully.
	fn on_hit(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState);
	/// Initializes or updates policy state for a stored entry.
	fn on_store(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState);
}

pub struct Entry<K, M: CachePolicy<K>> {
	entry_state: M::EntryState,
	key: K,
	value: BoxedValue,
}

// =================================
// CachePolicy: GenerationalEviction
// =================================

/// Retains recently used key groups and evicts entries that exceed the configured age or group limit.
pub struct GenerationalEviction<const STALE_EPOCHS: u64, const MAX_GROUPS: usize>;

impl<K: CacheKeyGroup, const STALE_EPOCHS: u64, const MAX_GROUPS: usize> CachePolicy<K> for GenerationalEviction<STALE_EPOCHS, MAX_GROUPS> {
	type PolicyState = u64;
	type EntryState = u64;

	fn touch(key: &K, entries: &mut Vec<Entry<K, Self>>, _policy_state: &mut Self::PolicyState) {
		entries.sort_by_key(|entry| entry.key.group() == key.group());
	}

	fn retire(entries: &mut Vec<Entry<K, Self>>, policy_state: &mut Self::PolicyState) {
		if *policy_state == u64::MAX {
			entries.clear();
			*policy_state = 0;
			return;
		}

		entries.retain(|entry| *policy_state - entry.entry_state < STALE_EPOCHS);
		while entries.chunk_by(|a, b| a.key.group() == b.key.group()).count() > MAX_GROUPS {
			let oldest_group = entries[0].key.group();
			let group_len = entries.iter().take_while(|entry| entry.key.group() == oldest_group).count();
			entries.drain(..group_len.max(1));
		}
	}

	fn on_hit(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState) {
		if entry_state == policy_state {
			*policy_state += 1;
		}
		*entry_state = *policy_state;
	}

	fn on_store(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState) {
		*entry_state = *policy_state;
	}
}

/// Provide a method to group entries by key for eviction.
trait CacheKeyGroup {
	type Group: PartialEq;
	fn group(&self) -> Self::Group;
}

impl CacheKeyGroup for Footprint {
	type Group = DMat2;
	fn group(&self) -> Self::Group {
		self.transform.matrix2
	}
}

// ======================================
// CachePolicy: LRU (Least Recently Used)
// ======================================

/// Retains up to `CAPACITY` entries and evicts the least recently used entry when full.
pub struct Lru<const CAPACITY: usize>;

impl<K, const CAPACITY: usize> CachePolicy<K> for Lru<CAPACITY> {
	// Keep tracks the most recent state.
	type PolicyState = u64;
	// Stores entry's recency. Larger is more recent.
	type EntryState = u64;

	fn touch(_key: &K, _entries: &mut Vec<Entry<K, Self>>, _policy_state: &mut Self::PolicyState) {}

	fn retire(entries: &mut Vec<Entry<K, Self>>, policy_state: &mut Self::PolicyState) {
		if *policy_state == u64::MAX {
			entries.clear();
			*policy_state = 0;
			return;
		}
		if entries.len() <= CAPACITY {
			return;
		};
		let Some((oldest_index, _)) = entries.iter().enumerate().min_by_key(|(_, entry)| entry.entry_state) else {
			return;
		};
		entries.swap_remove(oldest_index);
	}

	fn on_hit(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState) {
		*policy_state += 1;
		*entry_state = *policy_state;
	}

	fn on_store(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState) {
		*policy_state += 1;
		*entry_state = *policy_state;
	}
}

// ==========
// CacheInner
// ==========

type BoxedValue = Box<dyn std::any::Any + Send + Sync>;

struct CacheInner<K, E: CachePolicy<K>> {
	policy_state: E::PolicyState,
	entries: Vec<Entry<K, E>>,
}

impl<K, M: CachePolicy<K>> Default for CacheInner<K, M> {
	fn default() -> Self {
		Self {
			policy_state: Default::default(),
			entries: Default::default(),
		}
	}
}

impl<K: Copy + PartialEq, M: CachePolicy<K>> CacheInner<K, M> {
	fn take<S: 'static>(&mut self, key: &K) -> Option<S> {
		M::touch(key, &mut self.entries, &mut self.policy_state);

		let index = self.entries.iter().position(|entry| entry.key == *key);
		let hit = index.map(|index| {
			let entry = self.entries.get(index).unwrap();
			<dyn std::any::Any>::downcast_ref::<S>(entry.value.as_ref())?;

			let mut entry = self.entries.remove(index);
			entry.value.downcast().ok().map(|value| {
				M::on_hit(&mut entry.entry_state, &mut self.policy_state);
				*value
			})
		});

		M::retire(&mut self.entries, &mut self.policy_state);
		hit.flatten()
	}

	fn get_cloned<S: Clone + 'static>(&mut self, key: &K) -> Option<S> {
		M::touch(key, &mut self.entries, &mut self.policy_state);

		let index = self.entries.iter().position(|entry| entry.key == *key);
		let hit = index.map(|index| {
			let entry = self.entries.get_mut(index).unwrap();
			let value = <dyn std::any::Any>::downcast_ref::<S>(entry.value.as_ref())?;
			M::on_hit(&mut entry.entry_state, &mut self.policy_state);
			Some(value.clone())
		});

		M::retire(&mut self.entries, &mut self.policy_state);
		hit.flatten()
	}

	fn store(&mut self, key: &K, value: BoxedValue) {
		M::touch(key, &mut self.entries, &mut self.policy_state);

		self.entries.retain(|entry| entry.key != *key);
		let mut entry = Entry {
			key: *key,
			value,
			entry_state: M::EntryState::default(),
		};

		M::on_store(&mut entry.entry_state, &mut self.policy_state);
		self.entries.push(entry);
		M::retire(&mut self.entries, &mut self.policy_state);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[derive(Copy, Clone, PartialEq, Debug)]
	struct DummyKey(usize);
	#[derive(Clone, PartialEq, Debug)]
	struct DummyValue(usize);

	#[derive(Default)]
	struct CallCounts {
		touch: usize,
		retire: usize,
		on_hit: usize,
		on_store: usize,
	}

	struct TestPolicy;
	impl<K> CachePolicy<K> for TestPolicy {
		type PolicyState = CallCounts;
		type EntryState = ();

		fn touch(_key: &K, _entries: &mut Vec<Entry<K, Self>>, counts: &mut Self::PolicyState) {
			counts.touch += 1;
		}

		fn retire(_entries: &mut Vec<Entry<K, Self>>, counts: &mut Self::PolicyState) {
			counts.retire += 1;
		}

		fn on_hit(_entry_state: &mut Self::EntryState, counts: &mut Self::PolicyState) {
			counts.on_hit += 1;
		}

		fn on_store(_entry_state: &mut Self::EntryState, counts: &mut Self::PolicyState) {
			counts.on_store += 1;
		}
	}

	fn live<K, M: CachePolicy<K>>(cache: &Cache<K, M>) -> usize {
		cache.inner.lock().unwrap().entries.len()
	}

	#[test]
	fn take_removes_entry() {
		let cache = Cache::<DummyKey, TestPolicy>::default();
		let key = DummyKey(0);
		let val = DummyValue(0);
		cache.store(&key, val);
		let taken_val = cache.take::<DummyValue>(&key);

		assert_eq!(taken_val, Some(DummyValue(0)));
		assert_eq!(live(&cache), 0);

		let inner = cache.inner.lock().unwrap();
		assert_eq!(inner.policy_state.touch, 2);
		assert_eq!(inner.policy_state.retire, 2);
		assert_eq!(inner.policy_state.on_store, 1);
		assert_eq!(inner.policy_state.on_hit, 1);
	}

	#[test]
	fn take_type_mismatch_preserves_entry() {
		let cache = Cache::<DummyKey, TestPolicy>::default();
		let key = DummyKey(0);
		cache.store(&key, DummyValue(0));
		let mismatched_val = cache.take::<()>(&key);

		assert_eq!(live(&cache), 1);
		assert!(mismatched_val.is_none());

		let correct_val = cache.take::<DummyValue>(&key);
		assert_eq!(correct_val, Some(DummyValue(0)));

		let inner = cache.inner.lock().unwrap();
		assert_eq!(inner.policy_state.touch, 3);
		assert_eq!(inner.policy_state.retire, 3);
		assert_eq!(inner.policy_state.on_store, 1);
		assert_eq!(inner.policy_state.on_hit, 1);
	}

	#[test]
	fn get_cloned_returns_value_without_removing_entry() {
		let cache = Cache::<DummyKey, TestPolicy>::default();
		let key = DummyKey(0);
		let val = DummyValue(0);
		cache.store(&key, val);
		let cloned_val = cache.get_cloned::<DummyValue>(&key);

		assert_eq!(cloned_val, Some(DummyValue(0)));
		assert_eq!(live(&cache), 1);

		let inner = cache.inner.lock().unwrap();
		assert_eq!(inner.policy_state.touch, 2);
		assert_eq!(inner.policy_state.retire, 2);
		assert_eq!(inner.policy_state.on_store, 1);
		assert_eq!(inner.policy_state.on_hit, 1);
	}

	#[test]
	fn get_cloned_type_mismatch_preserves_entry() {
		let cache = Cache::<DummyKey, TestPolicy>::default();
		let key = DummyKey(0);
		cache.store(&key, DummyValue(0));
		let mismatched_val = cache.get_cloned::<()>(&key);

		assert_eq!(live(&cache), 1);
		assert!(mismatched_val.is_none());

		let correct_val = cache.get_cloned::<DummyValue>(&key);
		assert_eq!(correct_val, Some(DummyValue(0)));

		let inner = cache.inner.lock().unwrap();
		assert_eq!(inner.policy_state.touch, 3);
		assert_eq!(inner.policy_state.retire, 3);
		assert_eq!(inner.policy_state.on_store, 1);
		assert_eq!(inner.policy_state.on_hit, 1);
	}

	mod footprint_generational_eviction {
		use super::*;
		use core_types::transform::RenderQuality;
		use glam::{DAffine2, DVec2, UVec2};

		const STALE_EPOCHS: u64 = 2;
		const MAX_GROUPS: usize = 3;

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

		fn render(cache: &Cache<Footprint, GenerationalEviction<STALE_EPOCHS, MAX_GROUPS>>, footprint: &Footprint) -> bool {
			let hit = cache.take::<DummyValue>(footprint).is_some();
			cache.store(footprint, DummyValue(0));
			hit
		}

		#[test]
		fn continuous_zoom_is_bounded_by_views() {
			let cache = Cache::default();
			for step in 0..100 {
				render(&cache, &view(1. + step as f64 * 0.01, 0., DVec2::ZERO));
			}
			assert!(live(&cache) <= MAX_GROUPS);
		}

		#[test]
		fn continuous_rotation_is_bounded_by_views() {
			let cache = Cache::default();
			for step in 0..100 {
				render(&cache, &view(2., step as f64 * 0.01, DVec2::ZERO));
			}
			assert!(live(&cache) <= MAX_GROUPS);
		}

		#[test]
		fn zooming_reclaims_pan_entries() {
			let cache = Cache::default();
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
			let cache = Cache::default();
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
			let cache = Cache::default();
			for step in 0..100 {
				render(&cache, &thumbnail(1. + step as f64 * 0.001));
			}
			assert!(live(&cache) <= MAX_GROUPS);

			let viewport = view(2., 0., DVec2::ZERO);
			render(&cache, &viewport);
			for step in 0..50 {
				render(&cache, &thumbnail(2. + step as f64 * 0.001));
				assert!(render(&cache, &viewport), "thumbnail churn evicted the viewport entry");
			}
		}

		#[test]
		fn settled_view_retires_stale_entries() {
			let cache = Cache::default();
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

	mod lru {
		use std::array;

		use super::*;

		#[test]
		fn evicts_least_recently_used_entry_when_capacity_is_exceeded() {
			let cache = Cache::<DummyKey, Lru<2>>::default();
			let [(key0, val0), (key1, val1), (key2, val2)] = array::from_fn(|n| (DummyKey(n), DummyValue(n)));
			cache.store(&key0, val0);
			cache.store(&key1, val1);
			let _ = cache.get_cloned::<DummyValue>(&key0);
			cache.store(&key2, val2);

			assert_eq!(live(&cache), 2);
			let inner = cache.inner.lock().unwrap();
			assert!(inner.entries.iter().find(|entry| entry.key == key1).is_none());
		}

		#[test]
		fn get_cloned_refreshes_entry_recency() {
			let cache = Cache::<DummyKey, Lru<2>>::default();
			let [(key0, val0), (key1, val1)] = array::from_fn(|n| (DummyKey(n), DummyValue(n)));
			cache.store(&key0, val0);
			cache.store(&key1, val1);
			let _ = cache.get_cloned::<DummyValue>(&key0);

			let inner = cache.inner.lock().unwrap();
			assert_eq!(inner.entries.iter().find(|entry| entry.key == key0).unwrap().entry_state, inner.policy_state);
		}

		#[test]
		fn storing_existing_key_replaces_value_without_growing_cache() {
			let cache = Cache::<DummyKey, Lru<2>>::default();
			let [(key0, val0), (_, val1)] = array::from_fn(|n| (DummyKey(n), DummyValue(n)));
			cache.store(&key0, val0);

			assert_eq!(live(&cache), 1);
			cache.store(&key0, val1);
			assert_eq!(live(&cache), 1);
			let val = cache.get_cloned::<DummyValue>(&key0).unwrap();
			assert_eq!(val, DummyValue(1));
		}

		#[test]
		fn entry_count_never_exceeds_capacity() {
			let cache = Cache::<DummyKey, Lru<10>>::default();
			for n in 0..10 {
				cache.store(&DummyKey(n), DummyValue(n));
				assert_eq!(live(&cache), n + 1);
			}

			for n in 10..20 {
				cache.store(&DummyKey(n), DummyValue(n));
				assert_eq!(live(&cache), 10);
			}
		}
	}
}
