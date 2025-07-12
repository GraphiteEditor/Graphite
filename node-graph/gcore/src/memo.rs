use crate::{Node, WasmNotSend};
use dyn_any::DynFuture;
use std::future::Future;
use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::Mutex;

/// Caches the output of a given Node and acts as a proxy
#[derive(Default)]
pub struct MonitorMemoNode<T, CachedNode> {
	// Introspection cache, uses the hash of the nullified context with default var args
	// cache: Arc<Mutex<std::collections::HashMap<u64, Arc<T>>>>,
	cache: Arc<Mutex<Option<(u64, Arc<T>)>>>,
	node: CachedNode,
	changed_since_last_eval: Arc<Mutex<bool>>,
}
impl<'i, I: Hash + 'i + std::fmt::Debug, T: 'static + Clone + Send + Sync, CachedNode: 'i> Node<'i, I> for MonitorMemoNode<T, CachedNode>
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
			let cloned_data = (*data).clone();
			Box::pin(async move { cloned_data })
		} else {
			let fut = self.node.eval(input);
			let cache = self.cache.clone();
			*self.changed_since_last_eval.lock().unwrap() = true;
			Box::pin(async move {
				let value = fut.await;
				*cache.lock().unwrap() = Some((hash, Arc::new(value.clone())));
				value
			})
		}
	}

	// TODO: Consider returning a reference to the entire cache so the frontend reference is automatically updated as the context changes
	fn introspect(&self, check_if_evaluated: bool) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		let mut changed = self.changed_since_last_eval.lock().unwrap();
		if check_if_evaluated {
			if !*changed {
				return None;
			}
		}
		*changed = false;

		let cache_guard = self.cache.lock().unwrap();
		let cached = cache_guard.as_ref().expect("Cached data should always be evaluated before introspection");
		Some(cached.1.clone() as Arc<dyn std::any::Any + Send + Sync>)
	}
}

impl<T, CachedNode> MonitorMemoNode<T, CachedNode> {
	pub fn new(node: CachedNode) -> MonitorMemoNode<T, CachedNode> {
		MonitorMemoNode {
			cache: Default::default(),
			node,
			changed_since_last_eval: Arc::new(Mutex::new(true)),
		}
	}
}

/// Caches the output of a given Node and acts as a proxy
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

pub mod memo {
	pub const IDENTIFIER: crate::ProtoNodeIdentifier = crate::ProtoNodeIdentifier::new("graphene_core::memo::MemoNode");
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

impl<'i, I: 'i, T: 'i + Clone + WasmNotSend, CachedNode: 'i> Node<'i, I> for ImpureMemoNode<I, T, CachedNode>
where
	CachedNode: for<'any_input> Node<'any_input, I>,
	for<'a> <CachedNode as Node<'a, I>>::Output: Future<Output = T> + WasmNotSend,
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
			_phantom: std::marker::PhantomData,
		}
	}
}

pub mod impure_memo {
	pub const IDENTIFIER: crate::ProtoNodeIdentifier = crate::ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode");
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum IntrospectMode {
	Input,
	Data,
}

/// Caches the output of the last graph evaluation for introspection
#[derive(Default)]
pub struct MonitorNode<I, O, N> {
	#[allow(clippy::type_complexity)]
	input: Arc<Mutex<Option<Arc<I>>>>,
	output: Arc<Mutex<Option<Arc<O>>>>,
	// Gets set to true by the editor when before evaluating the network, then reset when the monitor node is evaluated
	introspect_input: Arc<Mutex<bool>>,
	introspect_output: Arc<Mutex<bool>>,
	node: N,
}

impl<'i, I, O, N> Node<'i, I> for MonitorNode<I, O, N>
where
	I: Clone + 'static + Send + Sync,
	O: Clone + 'static + Send + Sync,
	for<'a> N: Node<'a, I, Output: Future<Output = O> + WasmNotSend> + Send + Sync + 'i,
{
	type Output = DynFuture<'i, O>;
	fn eval(&'i self, input: I) -> Self::Output {
		Box::pin(async move {
			let output = self.node.eval(input.clone()).await;
			let mut introspect_input = self.introspect_input.lock().unwrap();
			if *introspect_input {
				*self.input.lock().unwrap() = Some(Arc::new(input));
				*introspect_input = false;
			}
			let mut introspect_output = self.introspect_output.lock().unwrap();
			if *introspect_output {
				*self.output.lock().unwrap() = Some(Arc::new(output.clone()));
				*introspect_output = false;
			}
			output
		})
	}
}

impl<I, O, N> MonitorNode<I, O, N> {
	pub fn new(node: N) -> MonitorNode<I, O, N> {
		MonitorNode {
			input: Arc::new(Mutex::new(None)),
			output: Arc::new(Mutex::new(None)),
			introspect_input: Arc::new(Mutex::new(false)),
			introspect_output: Arc::new(Mutex::new(false)),
			node,
		}
	}
}

pub mod monitor {
	pub const IDENTIFIER: crate::ProtoNodeIdentifier = crate::ProtoNodeIdentifier::new("graphene_core::memo::MonitorNode");
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct MemoHash<T: Hash> {
	hash: u64,
	value: T,
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
		Self { hash, value }
	}
	pub fn new_with_hash(value: T, hash: u64) -> Self {
		Self { hash, value }
	}

	fn calc_hash(data: &T) -> u64 {
		let mut hasher = DefaultHasher::new();
		data.hash(&mut hasher);
		hasher.finish()
	}

	pub fn inner_mut(&mut self) -> MemoHashGuard<'_, T> {
		MemoHashGuard { inner: self }
	}
	pub fn into_inner(self) -> T {
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

impl<T: Hash> std::ops::DerefMut for MemoHashGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner.value
	}
}
