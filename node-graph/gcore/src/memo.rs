use crate::Node;
use core::future::Future;

#[cfg(feature = "alloc")]
use alloc::sync::Arc;
use core::cell::Cell;
use core::marker::PhantomData;
use core::pin::Pin;

// Caches the output of a given Node and acts as a proxy
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
	// but that requires a lot of lifetime magic <- This was suggested by copilot but is pretty acurate xD
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

#[cfg(feature = "alloc")]
/// Caches the output of the last graph evaluation for introspection
#[derive(Default)]
pub struct MonitorNode<T> {
	output: Cell<Option<Arc<T>>>,
}

#[cfg(feature = "alloc")]
impl<'i, T: 'static + Clone> Node<'i, T> for MonitorNode<T> {
	type Output = T;
	fn eval(&'i self, input: T) -> Self::Output {
		self.output.set(Some(Arc::new(input.clone())));
		input
	}

	fn serialize(&self) -> Option<Arc<dyn core::any::Any>> {
		let out = self.output.take();
		self.output.set(out.clone());
		(out).as_ref().map(|output| output.clone() as Arc<dyn core::any::Any>)
	}
}

#[cfg(feature = "alloc")]
impl<T> MonitorNode<T> {
	pub const fn new() -> MonitorNode<T> {
		MonitorNode { output: Cell::new(None) }
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
pub struct EndLetNode<Input> {
	input: Input,
}
impl<'i, T: 'i, Input> Node<'i, T> for EndLetNode<Input>
where
	Input: Node<'i, ()>,
{
	type Output = <Input>::Output;
	fn eval(&'i self, _: T) -> Self::Output {
		let result = self.input.eval(());
		result
	}
}

impl<Input> EndLetNode<Input> {
	pub const fn new(input: Input) -> EndLetNode<Input> {
		EndLetNode { input }
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
