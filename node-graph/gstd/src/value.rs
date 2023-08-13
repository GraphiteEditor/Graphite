use core::marker::PhantomData;
pub use graphene_core::value::*;
use graphene_core::Node;

use dyn_any::DynAny;

pub struct AnyRefNode<'n, N: Node<'n>>(N, PhantomData<&'n ()>);

impl<'n, N: Node<'n, Output = &'n O>, O: DynAny<'n> + 'n> Node<'n> for AnyRefNode<'n, N> {
	fn eval(&'n self) -> &'n (dyn DynAny<'n>) {
		let value: &O = self.0.eval();
		value
	}
}
impl<'n, N: Node<'n, Output = &'n O>, O: 'n + ?Sized> AnyRefNode<'n, N> {
	pub fn new(n: N) -> AnyRefNode<'n, N> {
		AnyRefNode(n, PhantomData)
	}
}

pub struct StorageNode<'n>(&'n dyn Node<'n, Output = &'n dyn DynAny<'n>>);

impl<'n> Node<'n> for StorageNode<'n> {
	fn eval(&'n self) -> &'n (dyn DynAny<'n>) {
		self.0.eval()
	}
}
impl<'n> StorageNode<'n> {
	pub fn new<N: Node<'n, Output = &'n dyn DynAny<'n>>>(n: &'n N) -> StorageNode<'n> {
		StorageNode(n)
	}
}

#[derive(Default)]
pub struct AnyValueNode<'n, T>(T, PhantomData<&'n ()>);
impl<'n, T: 'n + DynAny<'n>> Node<'n> for AnyValueNode<'n, T> {
	fn eval(&'n self) -> &'n dyn DynAny<'n> {
		&self.0
	}
}

impl<'n, T> AnyValueNode<'n, T> {
	pub const fn new(value: T) -> AnyValueNode<'n, T> {
		AnyValueNode(value, PhantomData)
	}
}
