use core::marker::PhantomData;

use crate::Node;

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
	First: Node<Input, Output = Inter> + Copy,
	Second: Node<Inter> + Copy,
{
	type Output = Second::Output;

	fn eval(self, input: Input) -> Self::Output {
		// evaluate the first node with the given input
		// and then pipe the result from the first computation
		// into the second node
		let arg: Inter = self.first.eval(input);
		(&self.second).eval(arg)
	}
}

impl<'n, Input, First: 'n, Second: 'n> ComposeNode<First, Second, Input>
where
	First: Node<Input>,
	Second: Node<First::Output>,
{
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<First, Second, Input> { first, second, _phantom: PhantomData }
	}
}
pub trait After<Inter>: Sized {
	fn after<First, Input>(self, first: First) -> ComposeNode<First, Self, Input>
	where
		First: Node<Input, Output = Inter>,
		Self: Node<Inter>,
	{
		ComposeNode::<First, Self, Input> {
			first,
			second: self,
			_phantom: PhantomData,
		}
	}
}
impl<Second: Node<I>, I> After<I> for Second {}

pub trait AfterRef<Inter>: Sized {
	fn after<'n, First: 'n, Input>(&'n self, first: First) -> ComposeNode<First, &'n Self, Input>
	where
		First: Node<Input, Output = Inter> + Copy,
		&'n Self: Node<Inter>,
		Self: 'n,
	{
		ComposeNode::<First, &'n Self, Input> {
			first,
			second: self,
			_phantom: PhantomData,
		}
	}
}
impl<'n, Second: 'n, I> AfterRef<I> for Second where &'n Second: Node<I> {}

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
