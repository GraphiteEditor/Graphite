use crate::{Node, WasmNotSend};
use dyn_any::DynFuture;
use std::future::Future;
use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::Mutex;

/// Caches the output of a given node called with a specific input.
///
/// A cache miss occurs when the Option is None. In this case, the node evaluates the inner node and memoizes (stores) the result.
///
/// A cache hit occurs when the Option is Some and has a stored hash matching the hash of the call argument. In this case, the node returns the cached value without re-evaluating the inner node.
///
/// Currently, only one input-output pair is cached. Subsequent calls with different inputs will overwrite the previous cache.
#[derive(Default)]
pub struct MemoNode<T, CachedNode> {
	cache: Arc<Mutex<Option<(u64, T)>>>,
	node: CachedNode,
}
impl<'i, I: Hash + 'i, T: 'i + Clone + WasmNotSend, CachedNode: 'i> Node<'i, I> for MemoNode<T, CachedNode>
where
	CachedNode: for<'any_input> Node<'any_input, I>,
	for<'a> <CachedNode as Node<'a, I>>::Output: Future<Output = T> + WasmNotSend,
{
	// TODO: This should return a reference to the cached cached_value
	// but that requires a lot of lifetime magic <- This was suggested by copilot but is pretty accurate xD
	type Output = DynFuture<'i, T>;
	fn eval(&'i self, input: I) -> Self::Output {
		let mut hasher = DefaultHasher::new();
		input.hash(&mut hasher);
		let hash = hasher.finish();

		if let Some(data) = self.cache.lock().as_ref().unwrap().as_ref().and_then(|data| (data.0 == hash).then_some(data.1.clone())) {
			Box::pin(async move { data })
		} else {
			let fut = self.node.eval(input);
			let cache = self.cache.clone();
			Box::pin(async move {
				let value = fut.await;
				*cache.lock().unwrap() = Some((hash, value.clone()));
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

#[allow(clippy::module_inception)]
pub mod memo {
	pub const IDENTIFIER: crate::ProtoNodeIdentifier = crate::ProtoNodeIdentifier::new("graphene_core::memo::MemoNode");
}

/// Caches the output of a given Node and acts as a proxy.
/// In contrast to the regular `MemoNode`, this variant ignores all input.
/// This node might result in the document not updating properly. Use with caution!
#[derive(Default)]
pub struct ImpureMemoNode<I, T, CachedNode> {
	cache: Arc<Mutex<Option<T>>>,
	node: CachedNode,
	_phantom: std::marker::PhantomData<I>,
}

impl<'i, I: 'i, T: 'i + Clone + WasmNotSend, CachedNode: 'i> Node<'i, I> for ImpureMemoNode<I, T, CachedNode>
where
	CachedNode: for<'any_input> Node<'any_input, I>,
	for<'a> <CachedNode as Node<'a, I>>::Output: Future<Output = T> + WasmNotSend,
{
	// TODO: This should return a reference to the cached cached_value but that requires a lot of lifetime magic
	// TODO: (This was suggested by copilot but is pretty accurate xD)
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
			_phantom: std::marker::PhantomData,
		}
	}
}

pub mod impure_memo {
	pub const IDENTIFIER: crate::ProtoNodeIdentifier = crate::ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode");
}

/// Stores both what a node was called with and what it returned.
#[derive(Clone, Debug)]
pub struct IORecord<I, O> {
	pub input: I,
	pub output: O,
}

/// Caches the output of the last graph evaluation for introspection
#[derive(Default)]
pub struct MonitorNode<I, T, N> {
	#[allow(clippy::type_complexity)]
	io: Arc<Mutex<Option<Arc<IORecord<I, T>>>>>,
	node: N,
}

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

	fn serialize(&self) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
		let io = self.io.lock().unwrap();
		(io).as_ref().map(|output| output.clone() as Arc<dyn std::any::Any + Send + Sync>)
	}
}

impl<I, T, N> MonitorNode<I, T, N> {
	pub fn new(node: N) -> MonitorNode<I, T, N> {
		MonitorNode { io: Arc::new(Mutex::new(None)), node }
	}
}

pub mod monitor {
	pub const IDENTIFIER: crate::ProtoNodeIdentifier = crate::ProtoNodeIdentifier::new("graphene_core::memo::MonitorNode");
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
