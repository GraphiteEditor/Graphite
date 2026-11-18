use graphene_hash::CacheHash;
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hasher};
use std::ops::Deref;
use std::sync::{Arc, OnceLock};

pub trait Keyed: Clone {
	type Key: Ord + Clone + Debug;

	fn key(&self) -> Self::Key;
}

#[derive(Clone, Debug)]
pub struct ReplicatedMap<T: Keyed> {
	items: Arc<Vec<T>>,
	hash: Arc<OnceLock<u64>>,
}

impl<T: Keyed> Default for ReplicatedMap<T> {
	fn default() -> Self {
		Self {
			items: Arc::default(),
			hash: Arc::default(),
		}
	}
}

impl<T: Keyed + PartialEq> PartialEq for ReplicatedMap<T> {
	fn eq(&self, other: &Self) -> bool {
		self.items == other.items
	}
}

impl<T: Keyed + CacheHash> CacheHash for ReplicatedMap<T> {
	fn cache_hash<H: Hasher>(&self, state: &mut H) {
		let hash = self.hash.get_or_init(|| {
			let mut hasher = DefaultHasher::new();
			self.items.cache_hash(&mut hasher);
			hasher.finish()
		});
		state.write_u64(*hash);
	}
}

impl<T: Keyed> ReplicatedMap<T> {
	pub fn get(&self, key: &T::Key) -> Option<&T> {
		self.position(key).ok().map(|index| &self.items[index])
	}

	pub fn iter(&self) -> std::slice::Iter<'_, T> {
		self.items.iter()
	}

	pub fn len(&self) -> usize {
		self.items.len()
	}

	pub fn is_empty(&self) -> bool {
		self.items.is_empty()
	}

	pub fn edit(self) -> ReplicatedMapEdit<T> {
		ReplicatedMapEdit { map: self, deltas: Vec::new() }
	}

	pub fn apply(&mut self, deltas: impl IntoIterator<Item = ReplicatedMapDelta<T>>) {
		for delta in deltas {
			match delta {
				ReplicatedMapDelta::Put(item) => self.put(item),
				ReplicatedMapDelta::Remove(key) => {
					let removed = self.remove(&key);
					debug_assert!(removed, "a delta removes an item that its map never held");
				}
			}
		}
	}

	fn position(&self, key: &T::Key) -> Result<usize, usize> {
		self.items.binary_search_by(|item| item.key().cmp(key))
	}

	fn items_mut(&mut self) -> &mut Vec<T> {
		self.hash = Arc::default();
		Arc::make_mut(&mut self.items)
	}

	fn put(&mut self, item: T) {
		match self.position(&item.key()) {
			Ok(index) => self.items_mut()[index] = item,
			Err(index) => self.items_mut().insert(index, item),
		}
	}

	fn remove(&mut self, key: &T::Key) -> bool {
		let position = self.position(key);
		if let Ok(index) = position {
			self.items_mut().remove(index);
		}
		position.is_ok()
	}
}

impl<'a, T: Keyed> IntoIterator for &'a ReplicatedMap<T> {
	type Item = &'a T;
	type IntoIter = std::slice::Iter<'a, T>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

#[derive(Clone, Debug)]
pub enum ReplicatedMapDelta<T: Keyed> {
	Put(T),
	Remove(T::Key),
}

#[derive(Debug)]
pub struct ReplicatedMapEdit<T: Keyed> {
	map: ReplicatedMap<T>,
	deltas: Vec<ReplicatedMapDelta<T>>,
}

impl<T: Keyed> Deref for ReplicatedMapEdit<T> {
	type Target = ReplicatedMap<T>;

	fn deref(&self) -> &Self::Target {
		&self.map
	}
}

impl<T: Keyed> ReplicatedMapEdit<T> {
	pub fn put(&mut self, item: T)
	where
		T: PartialEq,
	{
		if self.map.get(&item.key()) == Some(&item) {
			return;
		}
		self.map.put(item.clone());
		self.deltas.push(ReplicatedMapDelta::Put(item));
	}

	pub fn update(&mut self, key: &T::Key, change: impl FnOnce(&mut T)) -> bool {
		let Ok(index) = self.map.position(key) else { return false };
		let item = &mut self.map.items_mut()[index];
		change(item);
		debug_assert!(item.key() == *key, "an update must not change the key of its item");
		self.deltas.push(ReplicatedMapDelta::Put(item.clone()));
		true
	}

	pub fn remove(&mut self, key: &T::Key) -> bool {
		let removed = self.map.remove(key);
		if removed {
			self.deltas.push(ReplicatedMapDelta::Remove(key.clone()));
		}
		removed
	}

	pub fn replace_all(&mut self, items: impl IntoIterator<Item = T>)
	where
		T: PartialEq,
	{
		let mut items: Vec<T> = items.into_iter().collect();
		items.sort_by_key(T::key);
		debug_assert!(items.windows(2).all(|pair| pair[0].key() != pair[1].key()), "two items of a map share a key");

		let removed = self.map.iter().map(T::key).filter(|key| items.binary_search_by(|item| item.key().cmp(key)).is_err());
		self.deltas.extend(removed.map(ReplicatedMapDelta::Remove));
		let put = items.iter().filter(|item| self.map.get(&item.key()) != Some(item));
		self.deltas.extend(put.cloned().map(ReplicatedMapDelta::Put));

		*self.map.items_mut() = items;
	}

	pub fn finish(self) -> (ReplicatedMap<T>, Vec<ReplicatedMapDelta<T>>) {
		(self.map, self.deltas)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
	struct Item {
		key: u8,
		value: u8,
	}

	impl Keyed for Item {
		type Key = u8;

		fn key(&self) -> u8 {
			self.key
		}
	}

	fn item(key: u8, value: u8) -> Item {
		Item { key, value }
	}

	fn map(items: &[Item]) -> ReplicatedMap<Item> {
		let mut edit = ReplicatedMap::default().edit();
		edit.replace_all(items.iter().copied());
		edit.finish().0
	}

	fn small_maps() -> Vec<Vec<Item>> {
		let choices = [None, Some(0), Some(1)];
		let mut maps = Vec::new();
		for a in choices {
			for b in choices {
				for c in choices {
					maps.push([a, b, c].into_iter().zip(0..).filter_map(|(value, key)| Some(item(key, value?))).collect());
				}
			}
		}
		maps
	}

	fn hash(map: &ReplicatedMap<Item>) -> u64 {
		let mut hasher = DefaultHasher::new();
		map.cache_hash(&mut hasher);
		hasher.finish()
	}

	#[test]
	fn a_copy_that_applies_the_deltas_of_an_edit_holds_the_same_items() {
		for old in small_maps() {
			for new in small_maps() {
				let mut copy = map(&old);

				let mut edit = map(&old).edit();
				edit.replace_all(new.iter().copied());
				let (replaced, deltas) = edit.finish();
				assert_eq!(replaced, map(&new));
				assert_eq!(deltas.is_empty(), old == new, "{old:?} to {new:?}");
				copy.apply(deltas);
				assert_eq!(copy, replaced, "{old:?} to {new:?}");

				let mut copy = map(&old);
				let mut edit = map(&old).edit();
				for item in &old {
					assert!(edit.remove(&item.key));
				}
				for item in new.iter().rev() {
					edit.put(*item);
				}
				let (rebuilt, deltas) = edit.finish();
				assert_eq!(rebuilt, map(&new));
				copy.apply(deltas);
				assert_eq!(copy, rebuilt, "{old:?} to {new:?}");
			}
		}
	}

	#[test]
	fn items_are_held_in_key_order_and_found_by_key() {
		let mut edit = ReplicatedMap::default().edit();
		edit.put(item(2, 0));
		edit.put(item(0, 0));
		edit.put(item(1, 0));
		edit.put(item(0, 1));
		assert_eq!(edit.get(&0), Some(&item(0, 1)), "an edit reads what it has done so far");
		let (map, _) = edit.finish();

		assert_eq!(map.iter().copied().collect::<Vec<_>>(), [item(0, 1), item(1, 0), item(2, 0)]);
		assert_eq!(map.get(&1), Some(&item(1, 0)));
		assert_eq!(map.get(&3), None);
		assert_eq!(map.len(), 3);
	}

	#[test]
	fn a_put_of_the_item_already_held_records_no_delta() {
		let mut edit = map(&[item(0, 0)]).edit();
		edit.put(item(0, 0));
		assert!(edit.finish().1.is_empty());
	}

	#[test]
	fn an_update_changes_the_item_in_the_copy_too() {
		let mut copy = map(&[item(0, 0), item(1, 0)]);

		let mut edit = copy.clone().edit();
		assert!(edit.update(&1, |item| item.value = 1));
		assert!(!edit.update(&2, |item| item.value = 1));
		assert!(!edit.remove(&2));
		let (updated, deltas) = edit.finish();

		assert_eq!(updated, map(&[item(0, 0), item(1, 1)]));
		copy.apply(deltas);
		assert_eq!(copy, updated);
	}

	#[test]
	fn an_edit_leaves_other_holders_of_the_set_as_they_were() {
		let held = map(&[item(0, 0)]);

		let mut edit = held.clone().edit();
		edit.put(item(1, 0));
		edit.remove(&0);

		assert_eq!(held, map(&[item(0, 0)]));
	}

	#[test]
	fn the_hash_follows_the_items() {
		let a = map(&[item(0, 0), item(1, 0)]);
		let held = a.clone();
		assert!(a.hash.get().is_none(), "nothing asked for the hash yet");
		assert_eq!(hash(&a), hash(&map(&[item(1, 0), item(0, 0)])));
		assert!(a.hash.get().is_some());
		assert!(held.hash.get().is_some(), "a clone made before shares the hash");

		let mut edit = a.clone().edit();
		edit.update(&1, |item| item.value = 1);
		let (b, deltas) = edit.finish();
		assert!(b.hash.get().is_none(), "a change empties the hash");
		assert_ne!(hash(&a), hash(&b));

		let mut copy = a.clone();
		copy.apply(deltas);
		assert!(copy.hash.get().is_none());
		assert_eq!(hash(&copy), hash(&b));

		let mut edit = b.edit();
		edit.update(&1, |item| item.value = 0);
		assert_eq!(hash(&edit.finish().0), hash(&a), "the same items again hash the same");
	}
}
