use core::marker::PhantomData;

use crate::{Node, NodeIO};
pub struct FnNode<T: Fn(I) -> O, I, O>(T, PhantomData<(I, O)>);

impl<'n, I, O, T: Fn(I) -> O> NodeIO<'n> for FnNode<T, I, O> {
	type Input = I;
	type Output = O;
}

impl<T: Fn(I) -> O, O, I> Node for FnNode<T, I, O> {
	fn eval<'i, 's: 'i>(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		self.0(input)
	}
}
/*
impl<'n, T: Fn(I) -> O, O, I> Node<I> for &'n FnNode<T, I, O> {
	type Output = O;

	fn eval(self, input: I) -> Self::Output {
		self.0(input)
	}
}

impl<T: Fn(I) -> O, I, O> FnNode<T, I, O> {
	pub fn new(f: T) -> Self {
		FnNode(f, PhantomData)
	}
}

pub struct FnNodeWithState<'n, T: Fn(I, &'n State) -> O, I, O: 'n, State: 'n>(T, State, PhantomData<&'n (O, I)>);
impl<'n, T: Fn(I, &State) -> O, I, O: 'n, State: 'n> Node<I> for &'n FnNodeWithState<'n, T, I, O, State> {
	type Output = O;

	fn eval(self, input: I) -> Self::Output {
		self.0(input, &self.1)
	}
}
impl<'n, T: Fn(I, &State) -> O, I, O: 'n, State: 'n> Node<I> for FnNodeWithState<'n, T, I, O, State> {
	type Output = O;

	fn eval(self, input: I) -> Self::Output {
		self.0(input, &self.1)
	}
}

impl<'n, T: Fn(I, &State) -> O, I, O, State> FnNodeWithState<'n, T, I, O, State> {
	pub fn new(f: T, state: State) -> Self {
		FnNodeWithState(f, state, PhantomData)
	}
}*/
