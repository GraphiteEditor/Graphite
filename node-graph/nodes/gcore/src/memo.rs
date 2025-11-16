use core_types::memo::*;
use core_types::{Node, WasmNotSend};
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
	use core_types::ProtoNodeIdentifier;

	pub const IDENTIFIER: ProtoNodeIdentifier = ProtoNodeIdentifier::new("graphene_core::memo::MemoNode");
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
	use core_types::ProtoNodeIdentifier;

	pub const IDENTIFIER: ProtoNodeIdentifier = ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode");
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
	use core_types::ProtoNodeIdentifier;

	pub const IDENTIFIER: ProtoNodeIdentifier = ProtoNodeIdentifier::new("graphene_core::memo::MonitorNode");
}
