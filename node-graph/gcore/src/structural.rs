use core::marker::PhantomData;

use crate::Node;

pub struct ComposeNode<'n, Input, First: Node<'n, Input>, Second> {
	first: First,
	second: Second,
	_phantom: PhantomData<&'n Input>,
}

impl<'n, Input, Inter, First, Second> Node<'n, Input> for ComposeNode<'n, Input, First, Second>
where
	First: Node<'n, Input, Output = Inter>,
	Second: Node<'n, Inter>,
{
	type Output = <Second as Node<'n, Inter>>::Output;

	fn eval(&'n self, input: Input) -> Self::Output {
		// evaluate the first node with the given input
		// and then pipe the result from the first computation
		// into the second node
		let arg: Inter = self.first.eval(input);
		self.second.eval(arg)
	}
}

impl<'n, Input, First, Second> ComposeNode<'n, Input, First, Second>
where
	First: Node<'n, Input>,
	Second: Node<'n, First::Output>,
{
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<'n, Input, First, Second> { first, second, _phantom: PhantomData }
	}
}

pub trait After<Inter>: Sized {
	fn after<'n, First, Input>(self, first: First) -> ComposeNode<'n, Input, First, Self>
	where
		First: Node<'n, Input, Output = Inter>,
		Self: Node<'n, Inter>,
	{
		ComposeNode::new(first, self)
	}
}
impl<'n, Second: Node<'n, I>, I> After<I> for Second {}

pub struct ConsNode<Root>(pub Root);

impl<'n, Root, Input> Node<'n, Input> for ConsNode<Root>
where
	Root: Node<'n, ()>,
{
	type Output = (Input, <Root as Node<'n, ()>>::Output);

	fn eval(&'n self, input: Input) -> Self::Output {
		let arg = self.0.eval(());
		(input, arg)
	}
}

pub struct ConsPassInputNode<Root>(pub Root);

impl<'n, Root, L, R> Node<'n, (L, R)> for ConsPassInputNode<Root>
where
	Root: Node<'n, R>,
{
	type Output = (L, <Root as Node<'n, R>>::Output);

	fn eval(&'n self, input: (L, R)) -> Self::Output {
		let arg = self.0.eval(input.1);
		(input.0, arg)
	}
}
