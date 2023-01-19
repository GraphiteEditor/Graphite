use core::marker::PhantomData;

use dyn_any::{StaticType, StaticTypeSized};

use crate::{Node, NodeIO};

trait Pair<'i>: NodeIO<'i>
where
	Self: 'i,
{
	type First: NodeIO<'i, Input = Self::Input> + 'i + Node;
	type Second: NodeIO<'i, Input = <Self::First as NodeIO<'i>>::Output, Output = Self::Output> + 'i + Node;
}
trait PairEval: for<'n> Pair<'n> {
	fn eval<'i, 's: 'i>(first: &'s <Self as Pair<'i>>::First, second: <Self as Pair<'i>>::Second, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output;
}

#[derive(Debug, Clone, Copy)]
pub struct ComposeNode<First, Second> {
	first: First,
	second: Second,
}

impl<'n, First, Second> NodeIO<'n> for (First, Second)
where
	First: Node,
	First: NodeIO<'n, Input = Self::Input>,
	Second: Node,
	Second: NodeIO<'n, Input = <First as NodeIO<'n>>::Output, Output = Self::Output>,
{
	type Input = <First as NodeIO<'n>>::Input;
	type Output = <Second as NodeIO<'n>>::Output;
}

impl<'n, First, Second> Pair<'n> for (First, Second)
where
	First: Node,
	First: NodeIO<'n, Input = Self::Input>,
	Second: Node,
	Second: NodeIO<'n, Input = <First as NodeIO<'n>>::Output, Output = Self::Output>,
{
	type First = First;
	type Second = Second;
}

impl<'i, First, Second> NodeIO<'i> for ComposeNode<First, Second>
where
	First: Node,
	Second: Node<Input = <First as NodeIO<'i>>::Output>,
{
	type Input = <First as NodeIO<'i>>::Input;
	type Output = <Second as NodeIO<'i>>::Output;
}

impl<First, Second> Node for ComposeNode<First, Second>
where
	Self: for<'a> Pair<'a>,
	First: Node,
	Second: Node,
{
	fn eval<'i, 's: 'i>(&'s self, input: <First as NodeIO<'i>>::Input) -> <Second as NodeIO<'i>>::Output {
		// evaluate the first node with the given input
		// and then pipe the result from the first computation
		// into the second node
		let arg = self.first.eval(input);
		self.second.eval(arg.into())
	}
}
impl<First: StaticTypeSized, Second: StaticTypeSized> dyn_any::StaticType for ComposeNode<First, Second> {
	type Static = ComposeNode<First::Static, Second::Static>;
}

impl<'n, First, Second> ComposeNode<First, Second>
where
	First: Node + 'n,
	Second: Node<Input = <First as NodeIO<'n>>::Output>,
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

impl<'n, First: Node> Then<'n> for First {}

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
