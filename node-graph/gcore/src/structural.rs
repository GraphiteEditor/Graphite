use dyn_any::StaticTypeSized;

use crate::Node;

#[derive(Debug, Clone)]
pub struct ComposeNode<First, Second> {
	first: First,
	second: Second,
}

impl<'i, Input: 'i, First, Second> Node<'i, Input> for ComposeNode<First, Second>
where
	First: Node<'i, Input>,
	Second: Node<'i, First::Output>,
{
	type Output = Second::Output;
	fn eval<'s: 'i>(&'s self, input: Input) -> Self::Output {
		// eval<'s: 'i>uate the first node with the given input
		// and then pipe the result from the first computation
		// into the second node
		let arg = self.first.eval(input);
		self.second.eval(arg)
	}
}
impl<First: StaticTypeSized, Second: StaticTypeSized> dyn_any::StaticType for ComposeNode<First, Second> {
	type Static = ComposeNode<First::Static, Second::Static>;
}

impl<'i, First, Second> ComposeNode<First, Second> {
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<First, Second> { first, second }
	}
}
pub trait Then<'i, Input: 'i>: Sized {
	fn then<Second>(self, second: Second) -> ComposeNode<Self, Second>
	where
		Self: Node<'i, Input>,
		Second: Node<'i, Self::Output>,
	{
		ComposeNode::<Self, Second> { first: self, second }
	}
}

impl<'i, First: Node<'i, Input>, Input: 'i> Then<'i, Input> for First {}

pub struct ConsNode<Root>(pub Root);

impl<'i, Root, Input: 'i> Node<'i, Input> for ConsNode<Root>
where
	Root: Node<'i, ()>,
{
	type Output = (Input, Root::Output);
	fn eval<'s: 'i>(&'s self, input: Input) -> Self::Output {
		let arg = self.0.eval(());
		(input, arg)
	}
}
impl<'i, Root: Node<'i, ()>> ConsNode<Root> {
	pub fn new(root: Root) -> Self {
		ConsNode(root)
	}
}
