use crate::Node;
use core::future::Future;

#[cfg(feature = "alloc")]
use alloc::sync::Arc;
use core::cell::Cell;
use core::marker::PhantomData;
use core::pin::Pin;

/// Caches the output of a given Node and acts as a proxy
#[derive(Default)]
pub struct MemoNode<T, CachedNode> {
	cache: Cell<Option<T>>,
	node: CachedNode,
}
impl<'i, 'o: 'i, T: 'i + Clone + 'o, CachedNode: 'i> Node<'i, ()> for MemoNode<T, CachedNode>
where
	CachedNode: for<'any_input> Node<'any_input, ()>,
	for<'a> <CachedNode as Node<'a, ()>>::Output: core::future::Future<Output = T> + 'a,
{
	// TODO: This should return a reference to the cached cached_value
	// but that requires a lot of lifetime magic <- This was suggested by copilot but is pretty accurate xD
	type Output = Pin<Box<dyn Future<Output = T> + 'i>>;
	fn eval(&'i self, input: ()) -> Pin<Box<dyn Future<Output = T> + 'i>> {
		Box::pin(async move {
			if let Some(cached_value) = self.cache.take() {
				self.cache.set(Some(cached_value.clone()));
				cached_value
			} else {
				let value = self.node.eval(input).await;
				self.cache.set(Some(value.clone()));
				value
			}
		})
	}

	fn reset(&self) {
		self.cache.set(None);
	}
}

impl<T, CachedNode> MemoNode<T, CachedNode> {
	pub const fn new(node: CachedNode) -> MemoNode<T, CachedNode> {
		MemoNode { cache: Cell::new(None), node }
	}
}

/// Caches the output of a given Node and acts as a proxy.
/// In contrast to the relgular `MemoNode`. This node ignores all input.
/// Using this node might result in the document not updating properly,
/// use with caution.
#[derive(Default)]
pub struct ImpureMemoNode<I, T, CachedNode> {
	cache: Cell<Option<T>>,
	node: CachedNode,
	_phantom: std::marker::PhantomData<I>,
}

impl<'i, 'o: 'i, I: 'i, T: 'i + Clone + 'o, CachedNode: 'i> Node<'i, I> for ImpureMemoNode<I, T, CachedNode>
where
	CachedNode: for<'any_input> Node<'any_input, I>,
	for<'a> <CachedNode as Node<'a, I>>::Output: core::future::Future<Output = T> + 'a,
{
	// TODO: This should return a reference to the cached cached_value
	// but that requires a lot of lifetime magic <- This was suggested by copilot but is pretty accurate xD
	type Output = Pin<Box<dyn Future<Output = T> + 'i>>;
	fn eval(&'i self, input: I) -> Pin<Box<dyn Future<Output = T> + 'i>> {
		Box::pin(async move {
			if let Some(cached_value) = self.cache.take() {
				self.cache.set(Some(cached_value.clone()));
				cached_value
			} else {
				let value = self.node.eval(input).await;
				self.cache.set(Some(value.clone()));
				value
			}
		})
	}

	fn reset(&self) {
		self.cache.set(None);
	}
}

impl<T, I, CachedNode> ImpureMemoNode<I, T, CachedNode> {
	pub const fn new(node: CachedNode) -> ImpureMemoNode<I, T, CachedNode> {
		ImpureMemoNode {
			cache: Cell::new(None),
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
	io: Cell<Option<Arc<IORecord<I, T>>>>,
	node: N,
}

#[cfg(feature = "alloc")]
impl<'i, 'a: 'i, T, I, N> Node<'i, I> for MonitorNode<I, T, N>
where
	I: Clone + 'static,
	<N as Node<'i, I>>::Output: Future<Output = T>,
	T: Clone + 'static,
	N: Node<'i, I>,
{
	type Output = Pin<Box<dyn Future<Output = T> + 'i>>;
	fn eval(&'i self, input: I) -> Self::Output {
		Box::pin(async move {
			let output = self.node.eval(input.clone()).await;
			self.io.set(Some(Arc::new(IORecord { input, output: output.clone() })));
			output
		})
	}

	fn serialize(&self) -> Option<Arc<dyn core::any::Any>> {
		let io = self.io.take();
		self.io.set(io.clone());
		(io).as_ref().map(|output| output.clone() as Arc<dyn core::any::Any>)
	}
}

#[cfg(feature = "alloc")]
impl<I, T, N> MonitorNode<I, T, N> {
	pub const fn new(node: N) -> MonitorNode<I, T, N> {
		MonitorNode { io: Cell::new(None), node }
	}
}

// Caches the output of a given Node and acts as a proxy
/// It provides two modes of operation, it can either be set
/// when calling the node with a `Some<T>` variant or the last
/// value that was added is returned when calling it with `None`
#[derive(Default)]
pub struct LetNode<T> {
	// We have to use an append only data structure to make sure the references
	// to the cache entries are always valid
	// TODO: We only ever access the last value so there is not really a reason for us
	// to store the previous entries. This should be reworked in the future
	cache: Cell<Option<T>>,
}
impl<'i, T: 'i + Clone> Node<'i, Option<T>> for LetNode<T> {
	type Output = T;
	fn eval(&'i self, input: Option<T>) -> Self::Output {
		if let Some(input) = input {
			self.cache.set(Some(input.clone()));
			input
		} else {
			let value = self.cache.take();
			self.cache.set(value.clone());
			value.expect("LetNode was not initialized. This can happen if you try to evaluate a node that depends on the EditorApi in the node_registry")
		}
	}
	fn reset(&self) {
		self.cache.set(None);
	}
}

impl<T> LetNode<T> {
	pub fn new() -> LetNode<T> {
		LetNode { cache: Default::default() }
	}
}

/// Caches the output of a given Node and acts as a proxy
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EndLetNode<Input, Parameter> {
	input: Input,
	paramenter: PhantomData<Parameter>,
}
impl<'i, T: 'i, Parameter: 'i + From<T>, Input> Node<'i, T> for EndLetNode<Input, Parameter>
where
	Input: Node<'i, Parameter>,
{
	type Output = <Input>::Output;
	fn eval(&'i self, t: T) -> Self::Output {
		let result = self.input.eval(Parameter::from(t));
		result
	}
}

impl<Input, Parameter> EndLetNode<Input, Parameter> {
	pub const fn new(input: Input) -> EndLetNode<Input, Parameter> {
		EndLetNode { input, paramenter: PhantomData }
	}
}

pub use crate::ops::SomeNode as InitNode;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct RefNode<T, Let> {
	let_node: Let,
	_t: PhantomData<T>,
}

impl<'i, T: 'i, Let> Node<'i, ()> for RefNode<T, Let>
where
	Let: for<'a> Node<'a, Option<T>>,
{
	type Output = <Let as Node<'i, Option<T>>>::Output;
	fn eval(&'i self, _: ()) -> Self::Output {
		self.let_node.eval(None)
	}
}

impl<Let, T> RefNode<T, Let> {
	pub const fn new(let_node: Let) -> RefNode<T, Let> {
		RefNode { let_node, _t: PhantomData }
	}
}
