use core::marker::PhantomData;

use crate::Node;
pub struct FnNode<'n, T: Fn(I) -> O, I, O>(T, PhantomData<&'n (I, O)>);
impl<'n, T: Fn(I) -> O, O, I> Node<'n, I> for FnNode<'n, T, I, O> {
	type Output = O;

	fn eval(&'n self, input: I) -> Self::Output {
		self.0(input)
	}
}

impl<'n, T: Fn(I) -> O, I, O> FnNode<'n, T, I, O> {
	pub fn new(f: T) -> Self {
		FnNode(f, PhantomData)
	}
}

pub struct FnNodeWithState<'n, T: Fn(I, &'n State) -> O, I, O, State: 'n>(T, State, PhantomData<&'n (O, I)>);
impl<'n, T: Fn(I, &'n State) -> O, I, O: 'n, State: 'n> Node<'n, I> for FnNodeWithState<'n, T, I, O, State> {
	type Output = O;

	fn eval(&'n self, input: I) -> Self::Output {
		self.0(input, &self.1)
	}
}

impl<'n, T: Fn(I, &'n State) -> O, I, O: 'n, State: 'n> FnNodeWithState<'n, T, I, O, State> {
	pub fn new(f: T, state: State) -> Self {
		FnNodeWithState(f, state, PhantomData)
	}
}
