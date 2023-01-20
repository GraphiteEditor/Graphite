use dyn_any::{StaticType, StaticTypeSized};

use crate::{Node, NodeIO};

#[derive(Debug, Clone, Copy)]
pub struct ComposeNode<First, Second> {
	first: First,
	second: Second,
}

impl<'i, 's: 'i, Input: 'i, First, Second> NodeIO<'i, Input> for ComposeNode<First, Second>
where
	First: Node<'i, 's, Input>,
	Second: Node<'i, 's, <First as NodeIO<'i, Input>>::Output>,
{
	type Output = <Second as NodeIO<'i, <First as NodeIO<'i, Input>>::Output>>::Output;
}

impl<'i, 's: 'i, Input: 'i, First, Second> Node<'i, 's, Input> for ComposeNode<First, Second>
where
	First: Node<'i, 's, Input>,
	Second: Node<'i, 's, <First as NodeIO<'i, Input>>::Output>,
{
	fn eval(&'s self, input: Input) -> <Self as NodeIO<'i, Input>>::Output {
		// evaluate the first node with the given input
		// and then pipe the result from the first computation
		// into the second node
		let arg = self.first.eval(input);
		self.second.eval(arg)
	}
}
impl<First: StaticTypeSized, Second: StaticTypeSized> dyn_any::StaticType for ComposeNode<First, Second> {
	type Static = ComposeNode<First::Static, Second::Static>;
}

impl<'i, 's: 'i, First, Second> ComposeNode<First, Second> {
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<First, Second> { first, second }
	}
}
pub trait Then<'i, 's: 'i, Input: 'i>: Sized {
	fn then<Second>(self, second: Second) -> ComposeNode<Self, Second>
	where
		Self: Node<'i, 's, Input>,
		Second: Node<'i, 's, <Self as NodeIO<'i, Input>>::Output>,
	{
		ComposeNode::<Self, Second> { first: self, second }
	}
}

impl<'i, 's: 'i, First: Node<'i, 's, Input>, Input: 'i> Then<'i, 's, Input> for First {}

pub struct ConsNode<Root>(pub Root);

impl<'i, 's: 'i, Input: 'i, Root: Node<'i, 's, ()>> NodeIO<'i, Input> for ConsNode<Root> {
	type Output = (Input, <Root as NodeIO<'i, ()>>::Output);
}

impl<'i, 's: 'i, Root, Input: 'i> Node<'i, 's, Input> for ConsNode<Root>
where
	Root: Node<'i, 's, ()>,
{
	fn eval(&'s self, input: Input) -> <Self as NodeIO<'i, Input>>::Output {
		let arg = self.0.eval(());
		(input, arg)
	}
}
impl<'i, 's: 'i, Root: Node<'i, 's, ()>> ConsNode<Root> {
	pub fn new(root: Root) -> Self {
		ConsNode(root)
	}
}
