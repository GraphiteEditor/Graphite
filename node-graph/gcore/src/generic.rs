use crate::Node;
use std::marker::PhantomData;
#[derive(Clone)]
pub struct FnNode<T: Fn(I) -> O, I, O>(T, PhantomData<(I, O)>);

impl<'i, T: Fn(I) -> O + 'i, O: 'i, I: 'i> Node<'i, I> for FnNode<T, I, O> {
	type Output = O;
	fn eval(&'i self, input: I) -> Self::Output {
		self.0(input)
	}
}

impl<T: Fn(I) -> O, I, O> FnNode<T, I, O> {
	pub fn new(f: T) -> Self {
		FnNode(f, PhantomData)
	}
}
