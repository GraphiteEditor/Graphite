use crate::Node;
use crate::poll::Poll;
use std::marker::PhantomData;
#[derive(Clone)]
pub struct FnNode<T: Fn(&I) -> O, I, O>(T, PhantomData<(I, O)>);

impl<T: Fn(&I) -> O, I, O> Node<I> for FnNode<T, I, O> {
	type Output = O;
	fn eval(&self, input: &I) -> Poll<O> {
		Poll::Final(self.0(input))
	}
}

impl<T: Fn(&I) -> O, I, O> FnNode<T, I, O> {
	pub fn new(f: T) -> Self {
		FnNode(f, PhantomData)
	}
}
