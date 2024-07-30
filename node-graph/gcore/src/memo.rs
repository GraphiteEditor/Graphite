use crate::{Node, WasmNotSend};
use core::future::Future;
use core::ops::Deref;
use std::sync::Mutex;

#[cfg(feature = "alloc")]
use alloc::sync::Arc;
use dyn_any::DynFuture;

/// Caches the output of a given Node and acts as a proxy
#[derive(Default)]
pub struct MemoNode<T, CachedNode> {
	cache: Arc<Mutex<Option<T>>>,
	node: CachedNode,
}
impl<'i, 'o: 'i, T: 'i + Clone + 'o + WasmNotSend, CachedNode: 'i> Node<'i, ()> for MemoNode<T, CachedNode>
where
	CachedNode: for<'any_input> Node<'any_input, ()>,
	for<'a> <CachedNode as Node<'a, ()>>::Output: core::future::Future<Output = T> + WasmNotSend,
{
	// TODO: This should return a reference to the cached cached_value
	// but that requires a lot of lifetime magic <- This was suggested by copilot but is pretty accurate xD
	type Output = DynFuture<'i, T>;
	fn eval(&'i self, input: ()) -> Self::Output {
		if let Some(cached_value) = self.cache.lock().as_ref().unwrap().deref() {
			let data = cached_value.clone();
			Box::pin(async move { data })
		} else {
			let fut = self.node.eval(input);
			let cache = self.cache.clone();
			Box::pin(async move {
				let value = fut.await;
				*cache.lock().unwrap() = Some(value.clone());
				value
			})
		}
	}

	fn reset(&self) {
		self.cache.lock().unwrap().take();
	}
}

impl<T, CachedNode> MemoNode<T, CachedNode> {
	pub fn new(node: CachedNode) -> MemoNode<T, CachedNode> {
		MemoNode { cache: Default::default(), node }
	}
}

/// Caches the output of a given Node and acts as a proxy.
/// In contrast to the regular `MemoNode`. This node ignores all input.
/// Using this node might result in the document not updating properly,
/// use with caution.
#[derive(Default)]
pub struct ImpureMemoNode<I, T, CachedNode> {
	cache: Arc<Mutex<Option<T>>>,
	node: CachedNode,
	_phantom: std::marker::PhantomData<I>,
}

impl<'i, 'o: 'i, I: 'i, T: 'i + Clone + 'o + WasmNotSend, CachedNode: 'i> Node<'i, I> for ImpureMemoNode<I, T, CachedNode>
where
	CachedNode: for<'any_input> Node<'any_input, I>,
	for<'a> <CachedNode as Node<'a, I>>::Output: core::future::Future<Output = T> + WasmNotSend,
{
	// TODO: This should return a reference to the cached cached_value
	// but that requires a lot of lifetime magic <- This was suggested by copilot but is pretty accurate xD
	type Output = DynFuture<'i, T>;
	fn eval(&'i self, input: I) -> Self::Output {
		if let Some(cached_value) = self.cache.lock().as_ref().unwrap().deref() {
			let data = cached_value.clone();
			Box::pin(async move { data })
		} else {
			let fut = self.node.eval(input);
			let cache = self.cache.clone();
			Box::pin(async move {
				let value = fut.await;
				*cache.lock().unwrap() = Some(value.clone());
				value
			})
		}
	}

	fn reset(&self) {
		self.cache.lock().unwrap().take();
	}
}

impl<T, I, CachedNode> ImpureMemoNode<I, T, CachedNode> {
	pub fn new(node: CachedNode) -> ImpureMemoNode<I, T, CachedNode> {
		ImpureMemoNode {
			cache: Default::default(),
			node,
			_phantom: core::marker::PhantomData,
		}
	}
}

/// Stores both what a node was called with and what it returned.
#[derive(Clone, Debug)]
pub struct IORecord<I, O> {
	pub input: I,
	pub output: O,
}

#[cfg(feature = "alloc")]
/// Caches the output of the last graph evaluation for introspection
#[derive(Default)]
pub struct MonitorNode<I, T, N> {
	#[allow(clippy::type_complexity)]
	io: Arc<Mutex<Option<Arc<IORecord<I, T>>>>>,
	node: N,
}

#[cfg(feature = "alloc")]
impl<'i, T, I, N> Node<'i, I> for MonitorNode<I, T, N>
where
	I: Clone + 'static + Send + Sync,
	T: Clone + 'static + Send + Sync,
	for<'a> N: Node<'a, I, Output: Future<Output = T> + WasmNotSend> + 'i,
{
	type Output = DynFuture<'i, T>;
	fn eval(&'i self, input: I) -> Self::Output {
		let io = self.io.clone();
		let output_fut = self.node.eval(input.clone());
		Box::pin(async move {
			let output = output_fut.await;
			*io.lock().unwrap() = Some(Arc::new(IORecord { input, output: output.clone() }));
			output
		})
	}

	fn serialize(&self) -> Option<Arc<dyn core::any::Any>> {
		let io = self.io.lock().unwrap();
		(io).as_ref().map(|output| output.clone() as Arc<dyn core::any::Any>)
	}
}

#[cfg(feature = "alloc")]
impl<I, T, N> MonitorNode<I, T, N> {
	pub fn new(node: N) -> MonitorNode<I, T, N> {
		MonitorNode { io: Arc::new(Mutex::new(None)), node }
	}
}

use core::hash::{Hash, Hasher};
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct MemoHash<T: Hash> {
	hash: u64,
	value: T,
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de> + Hash> serde::Deserialize<'de> for MemoHash<T> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		T::deserialize(deserializer).map(|value| Self::new(value))
	}
}

#[cfg(feature = "serde")]
impl<T: Hash + serde::Serialize> serde::Serialize for MemoHash<T> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		self.value.serialize(serializer)
	}
}

#[cfg(feature = "std")]
impl<T: Hash> MemoHash<T> {
	pub fn new(value: T) -> Self {
		let hash = Self::calc_hash(&value);
		Self { hash, value }
	}
	pub fn new_with_hash(value: T, hash: u64) -> Self {
		Self { hash, value }
	}

	fn calc_hash(data: &T) -> u64 {
		let mut hasher = std::collections::hash_map::DefaultHasher::new();
		data.hash(&mut hasher);
		hasher.finish()
	}

	pub fn inner_mut<'a>(&'a mut self) -> MemoHashGuard<'a, T> {
		MemoHashGuard { inner: self }
	}
	pub fn into_inner<'a>(self) -> T {
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

impl<T: Hash> core::ops::Deref for MemoHash<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

pub struct MemoHashGuard<'a, T: Hash> {
	inner: &'a mut MemoHash<T>,
}

impl<'a, T: Hash> core::ops::Drop for MemoHashGuard<'a, T> {
	fn drop(&mut self) {
		let hash = MemoHash::<T>::calc_hash(&self.inner.value);
		self.inner.hash = hash;
	}
}

impl<'a, T: Hash> core::ops::Deref for MemoHashGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner.value
	}
}

impl<'a, T: Hash> core::ops::DerefMut for MemoHashGuard<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner.value
	}
}
