use core::marker::PhantomData;

use crate::Node;
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

pub struct FnOnceNode<T: FnMut(I) -> O, I, O: Default>(T, PhantomData<(I, O)>);

impl<'i, T: FnMut(I) -> O + 'i, I: 'i, O: 'i + Default> FnOnceNode<T, I, O> {
	pub fn new(f: T) -> Self {
		FnOnceNode(f, PhantomData)
	}
	fn eval(&'i mut self, input: I) -> O {
		self.0(input)
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
