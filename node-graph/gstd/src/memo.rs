use futures::Future;

use graphene_core::Node;

use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use xxhash_rust::xxh3::Xxh3;

/// Caches the output of a given Node and acts as a proxy
#[derive(Default)]
pub struct CacheNode<T, CachedNode> {
	// We have to use an append only data structure to make sure the references
	// to the cache entries are always valid
	cache: boxcar::Vec<(u64, T, AtomicBool)>,
	node: CachedNode,
}
impl<'i, T: 'i + Clone, I: 'i + Hash, CachedNode: 'i> Node<'i, I> for CacheNode<T, CachedNode>
where
	CachedNode: for<'any_input> Node<'any_input, I>,
	for<'a> <CachedNode as Node<'a, I>>::Output: core::future::Future<Output = T> + 'a,
{
	// TODO: This should return a reference to the cached cached_value
	// but that requires a lot of lifetime magic <- This was suggested by copilot but is pretty acurate xD
	type Output = Pin<Box<dyn Future<Output = T> + 'i>>;
	fn eval(&'i self, input: I) -> Self::Output {
		Box::pin(async move {
			let mut hasher = Xxh3::new();
			input.hash(&mut hasher);
			let hash = hasher.finish();

			if let Some((_, cached_value, keep)) = self.cache.iter().find(|(h, _, _)| *h == hash) {
				keep.store(true, std::sync::atomic::Ordering::Relaxed);
				cached_value.clone()
			} else {
				trace!("Cache miss");
				let output = self.node.eval(input).await;
				let index = self.cache.push((hash, output, AtomicBool::new(true)));
				self.cache[index].1.clone()
			}
		})
	}

	fn reset(mut self: Pin<&mut Self>) {
		let old_cache = std::mem::take(&mut self.cache);
		self.cache = old_cache.into_iter().filter(|(_, _, keep)| keep.swap(false, std::sync::atomic::Ordering::Relaxed)).collect();
	}
}

impl<T, CachedNode> std::marker::Unpin for CacheNode<T, CachedNode> {}

impl<T, CachedNode> CacheNode<T, CachedNode> {
	pub fn new(node: CachedNode) -> CacheNode<T, CachedNode> {
		CacheNode { cache: boxcar::Vec::new(), node }
	}
}

/// Caches the output of the last graph evaluation for introspection
#[derive(Default)]
pub struct MonitorNode<T> {
	output: Mutex<Option<Arc<T>>>,
}
impl<'i, T: 'static + Clone> Node<'i, T> for MonitorNode<T> {
	type Output = T;
	fn eval(&'i self, input: T) -> Self::Output {
		*self.output.lock().unwrap() = Some(Arc::new(input.clone()));
		input
	}

	fn serialize(&self) -> Option<Arc<dyn core::any::Any>> {
		let output = self.output.lock().unwrap();
		(*output).as_ref().map(|output| output.clone() as Arc<dyn core::any::Any>)
	}
}

impl<T> MonitorNode<T> {
	pub const fn new() -> MonitorNode<T> {
		MonitorNode { output: Mutex::new(None) }
	}
}

/// Caches the output of a given Node and acts as a proxy
/// It provides two modes of operation, it can either be set
/// when calling the node with a `Some<T>` variant or the last
/// value that was added is returned when calling it with `None`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LetNode<T> {
	// We have to use an append only data structure to make sure the references
	// to the cache entries are always valid
	// TODO: We only ever access the last value so there is not really a reason for us
	// to store the previous entries. This should be reworked in the future
	cache: boxcar::Vec<(u64, T)>,
}
impl<'i, T: 'i + Hash> Node<'i, Option<T>> for LetNode<T> {
	type Output = &'i T;
	fn eval(&'i self, input: Option<T>) -> Self::Output {
		match input {
			Some(input) => {
				let mut hasher = Xxh3::new();
				input.hash(&mut hasher);
				let hash = hasher.finish();

				if let Some((cached_hash, cached_value)) = self.cache.iter().last() {
					if hash == *cached_hash {
						return cached_value;
					}
				}
				trace!("Cache miss");
				let index = self.cache.push((hash, input));
				&self.cache[index].1
			}
			None => &self.cache.iter().last().expect("Let node was not initialized").1,
		}
	}

	fn reset(mut self: Pin<&mut Self>) {
		if let Some(last) = std::mem::take(&mut self.cache).into_iter().last() {
			self.cache = boxcar::vec![last];
		}
	}
}

impl<T> std::marker::Unpin for LetNode<T> {}

impl<T> LetNode<T> {
	pub fn new() -> LetNode<T> {
		LetNode { cache: boxcar::Vec::new() }
	}
}

/// Caches the output of a given Node and acts as a proxy
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EndLetNode<Input> {
	input: Input,
}
impl<'i, T: 'i, Input> Node<'i, &'i T> for EndLetNode<Input>
where
	Input: Node<'i, ()>,
{
	type Output = <Input>::Output;
	fn eval(&'i self, _: &'i T) -> Self::Output {
		let result = self.input.eval(());
		result
	}
}

impl<Input> EndLetNode<Input> {
	pub const fn new(input: Input) -> EndLetNode<Input> {
		EndLetNode { input }
	}
}

pub use graphene_core::ops::SomeNode as InitNode;

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
