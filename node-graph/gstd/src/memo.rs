use std::marker::PhantomData;

use graphene_core::Node;
use once_cell::sync::OnceCell;

/// Caches the output of a given Node and acts as a proxy
#[derive(Default)]
pub struct CacheNode<T> {
	cache: OnceCell<T>,
}
impl<'i, T: 'i> Node<'i, T> for CacheNode<T> {
	type Output = &'i T;
	fn eval<'s: 'i>(&'s self, input: T) -> Self::Output {
		self.cache.get_or_init(|| {
			trace!("Creating new cache node");
			input
		})
	}
}

impl<T> CacheNode<T> {
	pub const fn new() -> CacheNode<T> {
		CacheNode { cache: OnceCell::new() }
	}
}

/// Caches the output of a given Node and acts as a proxy
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LetNode<T> {
	cache: OnceCell<T>,
}
impl<'i, T: 'i> Node<'i, Option<T>> for LetNode<T> {
	type Output = &'i T;
	fn eval<'s: 'i>(&'s self, input: Option<T>) -> Self::Output {
		match input {
			Some(input) => {
				self.cache.set(input).unwrap_or_else(|_| error!("Let node was set twice but is not mutable"));
				self.cache.get().unwrap()
			}
			None => self.cache.get().expect("Let node was not initialized"),
		}
	}
}

impl<T> LetNode<T> {
	pub const fn new() -> LetNode<T> {
		LetNode { cache: OnceCell::new() }
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
	fn eval<'s: 'i>(&'s self, _: &'i T) -> Self::Output {
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
	fn eval<'s: 'i>(&'s self, _: ()) -> Self::Output {
		self.let_node.eval(None)
	}
}

impl<Let, T> RefNode<T, Let> {
	pub const fn new(let_node: Let) -> RefNode<T, Let> {
		RefNode { let_node, _t: PhantomData }
	}
}
