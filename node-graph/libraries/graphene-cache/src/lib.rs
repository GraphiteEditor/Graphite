use core_types::transform::Footprint;
use glam::DMat2;
use std::{
	any::Any,
	sync::{Arc, Mutex},
};

// ===========
// CacheHandle
// ===========

#[derive(Clone)]
pub struct CacheHandle {
	slot: Arc<Mutex<Option<Box<dyn Any + Send + Sync>>>>,
	nonce: u64, // Avoid deduplication of cache instances across different nodes.
}

impl CacheHandle {
	pub fn get<T: 'static + Any + Send + Sync + Default + Clone>(&self) -> Result<T, CacheTypeError> {
		let mut guard = self.slot.lock().unwrap();
		if guard.is_none() {
			*guard = Some(Box::new(T::default()));
		}

		match guard.as_ref().unwrap().downcast_ref::<T>() {
			Some(cache) => Ok(cache.clone()),
			None => Err(CacheTypeError),
		}
	}
}

impl PartialEq for CacheHandle {
	fn eq(&self, _: &Self) -> bool {
		true
	}
}

impl Default for CacheHandle {
	fn default() -> Self {
		Self {
			slot: Default::default(),
			nonce: core_types::uuid::generate_uuid(),
		}
	}
}

impl core_types::CacheHash for CacheHandle {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		state.write_u64(self.nonce);
	}
}

unsafe impl dyn_any::StaticType for CacheHandle {
	type Static = CacheHandle;
}

#[cfg(feature = "serde")]
impl serde::Serialize for CacheHandle {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_unit()
	}
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for CacheHandle {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		serde::de::IgnoredAny::deserialize(deserializer)?;
		Ok(Self::default())
	}
}

impl std::fmt::Debug for CacheHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CacheHandle")
			.field("initialized", &self.slot.lock().unwrap().is_some())
			.field("nonce", &self.nonce)
			.finish()
	}
}

#[derive(Debug)]
pub struct CacheTypeError;

// =====
// Cache
// =====

/// A keyed cache backed by a linear `Vec`.
pub struct Cache<K, V, P: CachePolicy<K, V>> {
	state: Arc<Mutex<CacheState<K, V, P>>>,
}

impl<K: Copy + PartialEq, V, P: CachePolicy<K, V>> Cache<K, V, P> {
	/// Removes and returns the value stored for `key`.
	pub fn take(&self, key: &K) -> Option<V> {
		let mut guard = self.state.lock().unwrap();
		guard.take(key)
	}

	/// Clones the value stored for `key` without removing it.
	pub fn get_cloned(&self, key: &K) -> Option<V>
	where
		V: Clone,
	{
		let mut guard = self.state.lock().unwrap();
		guard.get_cloned(key)
	}

	/// Stores a value for `key`, replacing any existing value with the same key.
	pub fn store(&self, key: &K, value: V) {
		self.state.lock().unwrap().store(key, value);
	}
}

impl<K, V, P: CachePolicy<K, V>> Default for Cache<K, V, P> {
	fn default() -> Self {
		Self { state: Default::default() }
	}
}

impl<K, V, P: CachePolicy<K, V>> Clone for Cache<K, V, P> {
	fn clone(&self) -> Self {
		Self { state: self.state.clone() }
	}
}

impl<K, V, P: CachePolicy<K, V>> std::fmt::Debug for Cache<K, V, P> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Cache").field("entries", &self.state.lock().unwrap().entries.len()).finish()
	}
}

// ===================
// CachePolicy & Entry
// ===================

/// Defines the policy state and lifecycle hooks used to manage cached entries.
pub trait CachePolicy<K, V>: Sized {
	/// State shared by all entries in one cache.
	type PolicyState: Default;
	/// Policy-specific state stored with each cache entry.
	type EntryState: Default;

	/// Updates entry ordering or policy state before operating on key.
	fn touch(key: &K, entries: &mut Vec<Entry<K, V, Self>>, policy_state: &mut Self::PolicyState);
	/// Removes entries that should no longer be retained.
	fn retire(entries: &mut Vec<Entry<K, V, Self>>, policy_state: &mut Self::PolicyState);
	/// Updates policy state when an entry is accessed successfully.
	fn hit(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState);
	/// Initializes or updates policy state for a stored entry.
	fn store(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState);
}

pub struct Entry<K, V, P: CachePolicy<K, V>> {
	entry_state: P::EntryState,
	key: K,
	value: V,
}

// =================================
// CachePolicy: GenerationalEviction
// =================================

/// Retains recently used key groups and evicts entries that exceed the configured age or group limit.
pub struct GenerationalEviction<const STALE_EPOCHS: u64, const MAX_GROUPS: usize>;

impl<K: CacheKeyGroup, const STALE_EPOCHS: u64, const MAX_GROUPS: usize, V> CachePolicy<K, V> for GenerationalEviction<STALE_EPOCHS, MAX_GROUPS> {
	type PolicyState = u64;
	type EntryState = u64;

	fn touch(key: &K, entries: &mut Vec<Entry<K, V, Self>>, _policy_state: &mut Self::PolicyState) {
		entries.sort_by_key(|entry| entry.key.group() == key.group());
	}

	fn retire(entries: &mut Vec<Entry<K, V, Self>>, policy_state: &mut Self::PolicyState) {
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

	fn hit(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState) {
		if entry_state == policy_state {
			*policy_state += 1;
		}
		*entry_state = *policy_state;
	}

	fn store(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState) {
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

/// Retains up to `CAPACITY` entries and evicts the least recently used entry when capacity is exceeded.
pub struct Lru<const CAPACITY: usize>;

impl<K, const CAPACITY: usize, V> CachePolicy<K, V> for Lru<CAPACITY> {
	// Keep tracks the most recent state.
	type PolicyState = u64;
	// Stores entry's recency. Larger is more recent.
	type EntryState = u64;

	fn touch(_key: &K, _entries: &mut Vec<Entry<K, V, Self>>, _policy_state: &mut Self::PolicyState) {}

	fn retire(entries: &mut Vec<Entry<K, V, Self>>, policy_state: &mut Self::PolicyState) {
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

	fn hit(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState) {
		*policy_state += 1;
		*entry_state = *policy_state;
	}

	fn store(entry_state: &mut Self::EntryState, policy_state: &mut Self::PolicyState) {
		*policy_state += 1;
		*entry_state = *policy_state;
	}
}

// ==========
// CacheState
// ==========

struct CacheState<K, V, P: CachePolicy<K, V>> {
	policy_state: P::PolicyState,
	entries: Vec<Entry<K, V, P>>,
}

impl<K, V, P: CachePolicy<K, V>> Default for CacheState<K, V, P> {
	fn default() -> Self {
		Self {
			policy_state: Default::default(),
			entries: Default::default(),
		}
	}
}

impl<K: Copy + PartialEq, V, P: CachePolicy<K, V>> CacheState<K, V, P> {
	fn take(&mut self, key: &K) -> Option<V> {
		P::touch(key, &mut self.entries, &mut self.policy_state);

		let index = self.entries.iter().position(|entry| entry.key == *key);
		let hit = index.map(|index| {
			let mut entry = self.entries.remove(index);
			P::hit(&mut entry.entry_state, &mut self.policy_state);
			entry.value
		});

		P::retire(&mut self.entries, &mut self.policy_state);
		hit
	}

	fn get_cloned(&mut self, key: &K) -> Option<V>
	where
		V: Clone,
	{
		P::touch(key, &mut self.entries, &mut self.policy_state);

		let index = self.entries.iter().position(|entry| entry.key == *key);
		let hit = index.map(|index| {
			let entry = self.entries.get_mut(index).unwrap();
			P::hit(&mut entry.entry_state, &mut self.policy_state);
			Some(entry.value.clone())
		});

		P::retire(&mut self.entries, &mut self.policy_state);
		hit.flatten()
	}

	fn store(&mut self, key: &K, value: V) {
		P::touch(key, &mut self.entries, &mut self.policy_state);

		self.entries.retain(|entry| entry.key != *key);
		let mut entry = Entry {
			key: *key,
			value,
			entry_state: P::EntryState::default(),
		};

		P::store(&mut entry.entry_state, &mut self.policy_state);
		self.entries.push(entry);
		P::retire(&mut self.entries, &mut self.policy_state);
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
		hit: usize,
		store: usize,
	}

	struct TestPolicy;
	impl<K, V> CachePolicy<K, V> for TestPolicy {
		type PolicyState = CallCounts;
		type EntryState = ();

		fn touch(_key: &K, _entries: &mut Vec<Entry<K, V, Self>>, counts: &mut Self::PolicyState) {
			counts.touch += 1;
		}

		fn retire(_entries: &mut Vec<Entry<K, V, Self>>, counts: &mut Self::PolicyState) {
			counts.retire += 1;
		}

		fn hit(_entry_state: &mut Self::EntryState, counts: &mut Self::PolicyState) {
			counts.hit += 1;
		}

		fn store(_entry_state: &mut Self::EntryState, counts: &mut Self::PolicyState) {
			counts.store += 1;
		}
	}

	type DummyCacheType = Cache<DummyKey, DummyValue, TestPolicy>;

	fn live<K, V, P: CachePolicy<K, V>>(cache: &Cache<K, V, P>) -> usize {
		cache.state.lock().unwrap().entries.len()
	}

	#[test]
	fn get_cache_from_handle() {
		let cache_handle = CacheHandle::default();
		let cache1 = cache_handle.get::<DummyCacheType>();
		assert!(cache1.is_ok());

		let cache2 = cache_handle.clone().get::<DummyCacheType>();
		assert!(cache2.is_ok());
		assert!(Arc::ptr_eq(&cache1.unwrap().state, &cache2.unwrap().state));
	}

	#[test]
	fn get_cache_fails_with_wrong_cache_type() {
		type WrongCacheType = ();
		let cache_handle = CacheHandle::default();
		let cache = cache_handle.get::<DummyCacheType>();
		assert!(cache.is_ok());

		let wrong_cache = cache_handle.get::<WrongCacheType>();
		assert!(wrong_cache.is_err());

		// Validate if the first cache is still available from the handle
		let cache = cache_handle.get::<DummyCacheType>();
		assert!(cache.is_ok());
	}

	#[test]
	fn take_removes_entry() {
		let cache = Cache::<DummyKey, DummyValue, TestPolicy>::default();
		let key = DummyKey(0);
		let val = DummyValue(0);
		cache.store(&key, val);
		let taken_val = cache.take(&key);

		assert_eq!(taken_val, Some(DummyValue(0)));
		assert_eq!(live(&cache), 0);

		let state = cache.state.lock().unwrap();
		assert_eq!(state.policy_state.touch, 2);
		assert_eq!(state.policy_state.retire, 2);
		assert_eq!(state.policy_state.store, 1);
		assert_eq!(state.policy_state.hit, 1);
	}

	#[test]
	fn get_cloned_returns_value_without_removing_entry() {
		let cache = Cache::<DummyKey, DummyValue, TestPolicy>::default();
		let key = DummyKey(0);
		let val = DummyValue(0);
		cache.store(&key, val);
		let cloned_val = cache.get_cloned(&key);

		assert_eq!(cloned_val, Some(DummyValue(0)));
		assert_eq!(live(&cache), 1);

		let state = cache.state.lock().unwrap();
		assert_eq!(state.policy_state.touch, 2);
		assert_eq!(state.policy_state.retire, 2);
		assert_eq!(state.policy_state.store, 1);
		assert_eq!(state.policy_state.hit, 1);
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

		fn render(cache: &Cache<Footprint, DummyValue, GenerationalEviction<STALE_EPOCHS, MAX_GROUPS>>, footprint: &Footprint) -> bool {
			let hit = cache.take(footprint).is_some();
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
			let cache = Cache::<DummyKey, DummyValue, Lru<2>>::default();
			let [(key0, val0), (key1, val1), (key2, val2)] = array::from_fn(|n| (DummyKey(n), DummyValue(n)));
			cache.store(&key0, val0);
			cache.store(&key1, val1);
			let _ = cache.get_cloned(&key0);
			cache.store(&key2, val2);

			assert_eq!(live(&cache), 2);
			let state = cache.state.lock().unwrap();
			assert!(state.entries.iter().find(|entry| entry.key == key1).is_none());
		}

		#[test]
		fn get_cloned_refreshes_entry_recency() {
			let cache = Cache::<DummyKey, DummyValue, Lru<2>>::default();
			let [(key0, val0), (key1, val1)] = array::from_fn(|n| (DummyKey(n), DummyValue(n)));
			cache.store(&key0, val0);
			cache.store(&key1, val1);
			let _ = cache.get_cloned(&key0);

			let state = cache.state.lock().unwrap();
			assert_eq!(state.entries.iter().find(|entry| entry.key == key0).unwrap().entry_state, state.policy_state);
		}

		#[test]
		fn storing_existing_key_replaces_value_without_growing_cache() {
			let cache = Cache::<DummyKey, DummyValue, Lru<2>>::default();
			let [(key0, val0), (_, val1)] = array::from_fn(|n| (DummyKey(n), DummyValue(n)));
			cache.store(&key0, val0);

			assert_eq!(live(&cache), 1);
			cache.store(&key0, val1);
			assert_eq!(live(&cache), 1);
			let val = cache.get_cloned(&key0).unwrap();
			assert_eq!(val, DummyValue(1));
		}

		#[test]
		fn entry_count_never_exceeds_capacity() {
			let cache = Cache::<DummyKey, DummyValue, Lru<10>>::default();
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
