use graphene_core::Node;

use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use xxhash_rust::xxh3::Xxh3;

/// Caches the output of a given Node and acts as a proxy
#[derive(Default)]
pub struct CacheNode<T, CachedNode> {
	// We have to use an append only data structure to make sure the references
	// to the cache entries are always valid
	cache: boxcar::Vec<(u64, T, AtomicBool)>,
	node: CachedNode,
}
impl<'i, T: 'i, I: 'i + Hash, CachedNode: 'i> Node<'i, I> for CacheNode<T, CachedNode>
where
	CachedNode: for<'any_input> Node<'any_input, I, Output = T>,
{
	type Output = &'i T;
	fn eval(&'i self, input: I) -> Self::Output {
		let mut hasher = Xxh3::new();
		input.hash(&mut hasher);
		let hash = hasher.finish();

		if let Some((_, cached_value, keep)) = self.cache.iter().find(|(h, _, _)| *h == hash) {
			keep.store(true, std::sync::atomic::Ordering::Relaxed);
			return cached_value;
		} else {
			trace!("Cache miss");
			let output = self.node.eval(input);
			let index = self.cache.push((hash, output, AtomicBool::new(true)));
			return &self.cache[index].1;
		}
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
				return &self.cache[index].1;
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
		self.input.eval(())
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
	Let: for<'a> Node<'a, Option<T>, Output = &'a T>,
{
	type Output = &'i T;
	fn eval(&'i self, _: ()) -> Self::Output {
		self.let_node.eval(None)
	}
}

impl<Let, T> RefNode<T, Let> {
	pub const fn new(let_node: Let) -> RefNode<T, Let> {
		RefNode { let_node, _t: PhantomData }
	}
}
