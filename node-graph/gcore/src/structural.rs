use core::marker::PhantomData;

use dyn_any::{StaticType, StaticTypeSized};

use crate::{Node, NodeIO};

#[derive(Debug, Clone, Copy)]
pub struct ComposeNode<First, Second> {
	first: First,
	second: Second,
}

impl<'i, First, Second, Inter> NodeIO<'i> for ComposeNode<First, Second>
where
	First: Node<Output = Inter>,
	Second: Node<Input = Inter>,
{
	type Input = <First as NodeIO<'i>>::Input;
	type Output = <Second as NodeIO<'i>>::Output;
}

impl<Inter, First, Second> Node for ComposeNode<First, Second>
where
	First: Node<Output = Inter>,
	Second: Node<Input = Inter>,
{
	fn eval<'i, 's: 'i>(&'s self, input: <First as NodeIO<'i>>::Input) -> <Second as NodeIO<'i>>::Output {
		// evaluate the first node with the given input
		// and then pipe the result from the first computation
		// into the second node
		let arg: Inter = self.first.eval(input);
		self.second.eval(arg)
	}
}
impl<First: StaticTypeSized, Second: StaticTypeSized> dyn_any::StaticType for ComposeNode<First, Second> {
	type Static = ComposeNode<First::Static, Second::Static>;
}

impl<'n, First: 'n, Second: 'n, Inter> ComposeNode<First, Second>
where
	First: Node<Output = Inter>,
	Second: Node<Input = Inter>,
{
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<First, Second> { first, second }
	}
}

pub trait Then<'n>: Sized {
	fn then<Second>(self, second: Second) -> ComposeNode<Self, Second>
	where
		Self: Node,
		Self: 'n,
		Second: Node<Input = <Self as NodeIO<'n>>::Output>,
	{
		ComposeNode::<Self, Second> { first: self, second }
	}
}

impl<'n, First: Node<Output = Inter>, Inter> Then<'n> for First {}

pub struct ConsNode<Root: Node, Input>(pub Root, PhantomData<Input>);

impl<'n, I, Root: Node<Input = ()>> NodeIO<'n> for ConsNode<Root, I> {
	type Input = I;
	type Output = (I, <Root as NodeIO<'n>>::Output);
}

impl<Root, Input> Node for ConsNode<Root, Input>
where
	Root: Node<Input = ()>,
{
	fn eval<'i, 's: 'i>(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		let arg = self.0.eval(());
		(input, arg)
	}
}
impl<Root: Node<Input = Input>, Input> ConsNode<Root, Input> {
	pub fn new(root: Root) -> Self {
		ConsNode(root, PhantomData)
	}
}
