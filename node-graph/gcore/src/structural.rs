use core::marker::PhantomData;

use dyn_any::{StaticType, StaticTypeSized};

use crate::{Node, NodeIO};

trait Pair<'i, 's: 'i>: NodeIO<'i>
where
	Self: 'i + 's,
{
	type First: NodeIO<'i, Input = Self::Input> + 's + Node;
	type Second: NodeIO<'i, Input = <Self::First as NodeIO<'i>>::Output, Output = Self::Output> + 's + Node;
}
trait PairFn<'i, 'a>: for<'o, 's> Pair<'o, 's> {
	fn first(&'a self) -> &'a <Self as Pair<'i, 'a>>::First;
	fn second(&'a self) -> &'a <Self as Pair<'i, 'a>>::Second;
}

trait PairEval<'a>: for<'i> Pair<'i, 'a> + for<'i> PairFn<'i, 'a> {
	fn eval<'i, 's: 'i>(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		let arg = self.first().eval(input);
		self.second().eval(arg)
	}
}

impl<'a, P: for<'i, 's> Pair<'i, 's> + for<'i> PairFn<'i, 'a>> PairEval<'a> for P {}

impl<'n, 'r, First: 'n, Second: 'n, Input: 'n> PairFn<'r, 'n> for ComposeNode<Input, First, Second>
where
	First: Node<Input = Input> + 'r,
	Second: Node<Input = <First as NodeIO<'r>>::Output>,
	Self: for<'i> Pair<'i, 'n>,
{
	fn first(&self) -> &<Self as Pair<'r, 'n>>::First {
		&self.0
	}
	fn second(&self) -> &<Self as Pair<'r, 'n>>::Second {
		&self.1
	}
}

#[derive(Debug, Clone, Copy)]
pub struct ComposeNode<Input, First, Second> {
	first: First,
	second: Second,
	_phantom: PhantomData<Input>,
}

impl<'i, Input, First, Second> NodeIO<'i> for ComposeNode<Input, First, Second>
where
	First: Node<Input = Input>,
	Second: Node<Input = <First as NodeIO<'i>>::Output>,
{
	type Input = Input;
	type Output = <Second as NodeIO<'i>>::Output;
}
impl<'i, 's: 'i, Input: 's, First: 's, Second: 's> Pair<'i, 's> for ComposeNode<Input, First, Second>
where
	First: Node<Input = Input>,
	Second: Node<Input = <First as NodeIO<'i>>::Output>,
{
	type First = First;
	type Second = Second;
}
/*
impl<Input, First, Second> Node for ComposeNode<Input, First, Second>
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
}*/
/*
impl<First: StaticTypeSized, Second: StaticTypeSized> dyn_any::StaticType for ComposeNode<First, Second> {
	type Static = ComposeNode<First::Static, Second::Static>;
}*/

impl<'n, Input, First, Second> ComposeNode<Input, First, Second>
where
	First: Node<Input = Input> + 'n,
	Second: Node<Input = <First as NodeIO<'n>>::Output>,
{
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<Input, First, Second> { first, second, _phantom: PhantomData }
	}
}
/*
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
}*/
