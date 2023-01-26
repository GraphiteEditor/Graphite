use core::marker::PhantomData;

use crate::{Node, NodeIO};
pub struct FnNode<T: Fn(I) -> O, I, O>(T, PhantomData<(I, O)>);

impl<'i, 's: 'i, I: 'i, O: 'i, T: Fn(I) -> O> NodeIO<'i, I> for FnNode<T, I, O> {
	type Output = O;
}

impl<'i, 's: 'i, T: Fn(I) -> O + 'i, O: 'i, I: 'i> Node<'i, 's, I> for FnNode<T, I, O> {
	fn eval(&'s self, input: I) -> <Self as NodeIO<'i, I>>::Output {
		self.0(input)
	}
}

impl<T: Fn(I) -> O, I, O> FnNode<T, I, O> {
	pub fn new(f: T) -> Self {
		FnNode(f, PhantomData)
	}
}

pub struct FnNodeWithState<'i, T: Fn(I, &'i State) -> O, I, O, State: 'i>(T, State, PhantomData<(&'i O, I)>);
impl<'i, 's: 'i, I: 'i, O: 'i, State: 'i, T: Fn(I, &'i State) -> O + 'i> NodeIO<'i, I> for FnNodeWithState<'i, T, I, O, State> {
	type Output = O;
}
impl<'i, 's: 'i, I: 'i, O: 'i, State, T: Fn(I, &'i State) -> O + 'i> Node<'i, 's, I> for FnNodeWithState<'i, T, I, O, State> {
	fn eval(&'s self, input: I) -> <Self as NodeIO<'i, I>>::Output {
		(self.0)(input, &self.1)
	}
}
impl<'i, 's: 'i, I, O, State, T: Fn(I, &'i State) -> O> FnNodeWithState<'i, T, I, O, State> {
	pub fn new(f: T, state: State) -> Self {
		FnNodeWithState(f, state, PhantomData)
	}
}
