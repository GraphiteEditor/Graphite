use crate::Node;

use core::marker::PhantomData;
use core::ops::{DerefMut, Index, IndexMut};

struct SetNode<S, I, Storage, Index> {
	storage: Storage,
	index: Index,
	_s: PhantomData<S>,
	_i: PhantomData<I>,
}

#[node_macro::node_fn(SetNode<_S, _I>)]
fn set_node<T, _S, _I>(value: T, storage: &'input mut _S, index: _I)
where
	_S: IndexMut<_I>,
	_S::Output: DerefMut<Target = T> + Sized,
{
	*storage.index_mut(index).deref_mut() = value;
}

struct GetNode<S, Storage> {
	storage: Storage,
	_s: PhantomData<S>,
}

#[node_macro::node_fn(GetNode<_S>)]
fn get_node<_S, I>(index: I, storage: &'input _S) -> &'input _S::Output
where
	_S: Index<I>,
	_S::Output: Sized,
{
	storage.index(index)
}
