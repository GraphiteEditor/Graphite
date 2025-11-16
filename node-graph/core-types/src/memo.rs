use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

/// Stores both what a node was called with and what it returned.
#[derive(Clone, Debug)]
pub struct IORecord<I, O> {
	pub input: I,
	pub output: O,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct MemoHash<T: Hash> {
	hash: u64,
	value: Arc<T>,
}

impl<'de, T: serde::Deserialize<'de> + Hash> serde::Deserialize<'de> for MemoHash<T> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		T::deserialize(deserializer).map(|value| Self::new(value))
	}
}

impl<T: Hash + serde::Serialize> serde::Serialize for MemoHash<T> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		self.value.serialize(serializer)
	}
}

impl<T: Hash> MemoHash<T> {
	pub fn new(value: T) -> Self {
		let hash = Self::calc_hash(&value);
		Self { hash, value: value.into() }
	}
	pub fn new_with_hash(value: T, hash: u64) -> Self {
		Self { hash, value: value.into() }
	}

	fn calc_hash(data: &T) -> u64 {
		let mut hasher = DefaultHasher::new();
		data.hash(&mut hasher);
		hasher.finish()
	}

	pub fn inner_mut(&mut self) -> MemoHashGuard<'_, T> {
		MemoHashGuard { inner: self }
	}
	pub fn into_inner(self) -> Arc<T> {
		self.value
	}
	pub fn hash_code(&self) -> u64 {
		self.hash
	}
}
impl<T: Hash> From<T> for MemoHash<T> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

impl<T: Hash> Hash for MemoHash<T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.hash.hash(state)
	}
}

impl<T: Hash> Deref for MemoHash<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

pub struct MemoHashGuard<'a, T: Hash> {
	inner: &'a mut MemoHash<T>,
}

impl<T: Hash> Drop for MemoHashGuard<'_, T> {
	fn drop(&mut self) {
		let hash = MemoHash::<T>::calc_hash(&self.inner.value);
		self.inner.hash = hash;
	}
}

impl<T: Hash> Deref for MemoHashGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner.value
	}
}

impl<T: Hash + Clone> std::ops::DerefMut for MemoHashGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		Arc::make_mut(&mut self.inner.value)
	}
}
