use core::marker::PhantomData;

use crate::{Node, NodeMut};
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

pub struct FnMutNode<T: FnMut(I) -> O, I, O>(T, PhantomData<(I, O)>);

impl<'i, T: FnMut(I) -> O + 'i, O: 'i, I: 'i> NodeMut<'i, I> for FnMutNode<T, I, O> {
	type MutOutput = O;
	fn eval_mut(&'i mut self, input: I) -> Self::MutOutput {
		self.0(input)
	}
}

impl<'i, T: FnMut(I) -> O + 'i, I: 'i, O: 'i> FnMutNode<T, I, O> {
	pub fn new(f: T) -> Self {
		FnMutNode(f, PhantomData)
	}
}

pub struct FnNodeWithState<'i, T: Fn(I, &'i State) -> O, I, O, State: 'i>(T, State, PhantomData<(&'i O, I)>);
impl<'i, I: 'i, O: 'i, State, T: Fn(I, &'i State) -> O + 'i> Node<'i, I> for FnNodeWithState<'i, T, I, O, State> {
	type Output = O;
	fn eval(&'i self, input: I) -> Self::Output {
		(self.0)(input, &self.1)
	}
}
impl<'i, I, O, State, T: Fn(I, &'i State) -> O> FnNodeWithState<'i, T, I, O, State> {
	pub fn new(f: T, state: State) -> Self {
		FnNodeWithState(f, state, PhantomData)
	}
}
