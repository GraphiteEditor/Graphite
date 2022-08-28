use core::marker::PhantomData;

use crate::{Node, RefNode};

#[derive(Debug)]
pub struct ComposeNode<First, Second, Input> {
	first: First,
	second: Second,
	_phantom: PhantomData<Input>,
}

impl<Input, Inter, First, Second> Node<Input> for ComposeNode<First, Second, Input>
where
	First: Node<Input, Output = Inter>,
	Second: Node<Inter>,
{
	type Output = <Second as Node<Inter>>::Output;

	fn eval(self, input: Input) -> Self::Output {
		// evaluate the first node with the given input
		// and then pipe the result from the first computation
		// into the second node
		let arg: Inter = self.first.eval(input);
		self.second.eval(arg)
	}
}
impl<'n, Input, Inter, First, Second> Node<Input> for &'n ComposeNode<First, Second, Input>
where
	First: RefNode<Input, Output = Inter> + Copy,
	Second: RefNode<Inter> + Copy,
{
	type Output = <Second as RefNode<Inter>>::Output;

	fn eval(self, input: Input) -> Self::Output {
		// evaluate the first node with the given input
		// and then pipe the result from the first computation
		// into the second node
		let arg: Inter = (self.first).eval_ref(input);
		(self.second).eval_ref(arg)
	}
}
impl<Input, Inter, First, Second> RefNode<Input> for ComposeNode<First, Second, Input>
where
	First: RefNode<Input, Output = Inter> + Copy,
	Second: RefNode<Inter> + Copy,
{
	type Output = <Second as RefNode<Inter>>::Output;

	fn eval_ref(&self, input: Input) -> Self::Output {
		// evaluate the first node with the given input
		// and then pipe the result from the first computation
		// into the second node
		let arg: Inter = (self.first).eval_ref(input);
		(self.second).eval_ref(arg)
	}
}
#[cfg(feature = "std")]
impl<Input: 'static, First: 'static, Second: 'static> dyn_any::StaticType for ComposeNode<First, Second, Input> {
	type Static = ComposeNode<First, Second, Input>;
}

impl<'n, Input, First: 'n, Second: 'n> ComposeNode<First, Second, Input> {
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<First, Second, Input> { first, second, _phantom: PhantomData }
	}
}

pub trait Then<Inter, Input>: Sized {
	fn then<Second>(self, second: Second) -> ComposeNode<Self, Second, Input>
	where
		Self: Node<Input, Output = Inter>,
		Second: Node<Inter>,
	{
		ComposeNode::<Self, Second, Input> {
			first: self,
			second,
			_phantom: PhantomData,
		}
	}
}

impl<First: Node<Input, Output = Inter>, Inter, Input> Then<Inter, Input> for First {}

pub trait ThenRef<Inter, Input>: Sized {
	fn after<'n, Second: 'n>(&'n self, second: Second) -> ComposeNode<&'n Self, Second, Input>
	where
		&'n Self: Node<Input, Output = Inter> + Copy,
		Second: Node<Inter>,
		Self: 'n,
	{
		ComposeNode::<&'n Self, Second, Input> {
			first: self,
			second,
			_phantom: PhantomData,
		}
	}
}
impl<'n, First: 'n, Inter, Input> ThenRef<Inter, Input> for First where &'n First: Node<Input, Output = Inter> {}

#[cfg(feature = "async")]
pub trait ThenBox<Inter, Input> {
	fn then<'n, Second: 'n>(self, second: Second) -> ComposeNode<Self, Second, Input>
	where
		alloc::boxed::Box<Self>: Node<Input, Output = Inter>,
		Second: Node<Inter> + Copy,
		Self: Sized,
	{
		ComposeNode::<Self, Second, Input> {
			first: self,
			second,
			_phantom: PhantomData,
		}
	}
}
#[cfg(feature = "async")]
impl<'n, First: 'n, Inter, Input> ThenBox<Inter, Input> for alloc::boxed::Box<First> where &'n alloc::boxed::Box<First>: Node<Input, Output = Inter> {}

pub struct ConsNode<Root>(pub Root);

impl<Root, Input> Node<Input> for ConsNode<Root>
where
	Root: Node<()>,
{
	type Output = (Input, <Root as Node<()>>::Output);

	fn eval(self, input: Input) -> Self::Output {
		let arg = self.0.eval(());
		(input, arg)
	}
}
impl<'n, Root: Node<()> + Copy, Input> Node<Input> for &'n ConsNode<Root> {
	type Output = (Input, Root::Output);

	fn eval(self, input: Input) -> Self::Output {
		let arg = (&self.0).eval(());
		(input, arg)
	}
}

pub struct ConsPassInputNode<Root>(pub Root);

impl<Root, L, R> Node<(L, R)> for ConsPassInputNode<Root>
where
	Root: Node<R>,
{
	type Output = (L, <Root as Node<R>>::Output);

	fn eval(self, input: (L, R)) -> Self::Output {
		let arg = self.0.eval(input.1);
		(input.0, arg)
	}
}
impl<'n, Root, L, R> Node<(L, R)> for &'n ConsPassInputNode<Root>
where
	&'n Root: Node<R>,
{
	type Output = (L, <&'n Root as Node<R>>::Output);

	fn eval(self, input: (L, R)) -> Self::Output {
		let arg = (&self.0).eval(input.1);
		(input.0, arg)
	}
}
