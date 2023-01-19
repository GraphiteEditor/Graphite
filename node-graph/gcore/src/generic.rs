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

impl<T: Fn(I) -> O, I, O> FnNode<T, I, O> {
	pub fn new(f: T) -> Self {
		FnNode(f, PhantomData)
	}
}

pub struct FnNodeWithState<T: for<'s> Fn(I, &'s State) -> O, I, O, State>(T, State, PhantomData<(O, I)>);
impl<'n, I, O, State, T: for<'s> Fn(I, &'s State) -> O> NodeIO<'n> for FnNodeWithState<T, I, O, State> {
	type Input = I;
	type Output = O;
}
impl<I, O, State, T: for<'state> Fn(I, &'state State) -> O> Node for FnNodeWithState<T, I, O, State> {
	fn eval<'i, 's: 'i>(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		(self.0)(input, &self.1)
	}
}
impl<I, O, State, T: for<'s> Fn(I, &'s State) -> O> FnNodeWithState<T, I, O, State> {
	pub fn new(f: T, state: State) -> Self {
		FnNodeWithState(f, state, PhantomData)
	}
}
