use core::marker::PhantomData;

use dyn_any::{StaticType, StaticTypeSized};

use crate::{Node, NodeIO};
/*
pub trait Pair<'i, 's: 'i>: NodeIO<'i>
where
	Self: 'i + 's,
{
	type First: NodeIO<'i, Input = Self::Input> + 's + Node;
	type Second: NodeIO<'i, Input = <Self::First as NodeIO<'i>>::Output, Output = Self::Output> + 's + Node;
}

pub trait PairFn<'i, 'a: 'i>: Pair<'i, 'a> {
	fn first(&'a self) -> &'a <Self as Pair<'i, 'a>>::First;
	fn second(&'a self) -> &'a <Self as Pair<'i, 'a>>::Second;
}

pub trait PairEval<'i, 's: 'i>: Pair<'i, 's> + PairFn<'i, 's> {
	fn eval_pair(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		let arg = self.first().eval(input);
		self.second().eval(arg)
	}
}

impl<'a: 'r, 'r, P: Pair<'r, 'a> + PairFn<'r, 'a>> PairEval<'r, 'a> for P {}

impl<'n: 'r, 'r, First: 'n, Second: 'n, Input: 'n> PairFn<'r, 'n> for ComposeNode<Input, First, Second>
where
	First: Node<Input = Input> + 'r,
	Second: Node<Input = <First as NodeIO<'r>>::Output>,
{
	fn first(&self) -> &<Self as Pair<'r, 'n>>::First {
		&self.first
	}
	fn second(&self) -> &<Self as Pair<'r, 'n>>::Second {
		&self.second
	}
}
*/
#[derive(Debug, Clone, Copy)]
pub struct ComposeNode<Input, First, Second> {
	first: First,
	second: Second,
	_phantom: PhantomData<Input>,
}

impl<'i, 's: 'i, Input, First, Second> NodeIO<'i> for ComposeNode<Input, First, Second>
where
	First: Node<'i, 's, Input = Input>,
	Second: Node<'i, 's, Input = <First as NodeIO<'i>>::Output>,
{
	type Input = Input;
	type Output = <Second as NodeIO<'i>>::Output;
}
/*
impl<'i, 's: 'i, Input: 's, First: 's, Second: 's> Pair<'i, 's> for ComposeNode<Input, First, Second>
where
	First: Node<Input = Input>,
	Second: Node<Input = <First as NodeIO<'i>>::Output>,
{
	type First = First;
	type Second = Second;
}*/

impl<'i, 's: 'i, Input: 'i, First, Second> Node<'i, 's> for ComposeNode<Input, First, Second>
where
	First: Node<'i, 's, Input = Input>,
	Second: Node<'i, 's, Input = <First as NodeIO<'i>>::Output>,
{
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		// evaluate the first node with the given input
		// and then pipe the result from the first computation
		// into the second node
		let arg = self.first.eval(input);
		self.second.eval(arg)
	}
}
/*
impl<First: StaticTypeSized, Second: StaticTypeSized> dyn_any::StaticType for ComposeNode<First, Second> {
	type Static = ComposeNode<First::Static, Second::Static>;
}*/

impl<'i, 's: 'i, Input, First, Second> ComposeNode<Input, First, Second>
where
	First: Node<'i, 's, Input = Input>,
	Second: Node<'i, 's, Input = <First as NodeIO<'i>>::Output>,
{
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<Input, First, Second> { first, second, _phantom: PhantomData }
	}
}
pub trait Then<'i, 's: 'i, Input: 'i>: Sized {
	fn then<Second>(self, second: Second) -> ComposeNode<Input, Self, Second>
	where
		Self: Node<'i, 's>,
		Second: Node<'i, 's, Input = <Self as NodeIO<'i>>::Output>,
	{
		ComposeNode::<Input, Self, Second> {
			first: self,
			second,
			_phantom: PhantomData,
		}
	}
}

impl<'i, 's: 'i, First: Node<'i, 's>, Input: 'i> Then<'i, 's, Input> for First {}

pub struct ConsNode<Root, Input>(pub Root, PhantomData<Input>);

impl<'i, 's: 'i, I: 'i, Root: Node<'i, 's, Input = ()>> NodeIO<'i> for ConsNode<Root, I> {
	type Input = I;
	type Output = (I, <Root as NodeIO<'i>>::Output);
}

impl<'i, 's: 'i, Root, Input: 'i> Node<'i, 's> for ConsNode<Root, Input>
where
	Root: Node<'i, 's, Input = ()>,
{
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		let arg = self.0.eval(());
		(input, arg)
	}
}
impl<'i, 's: 'i, Root: Node<'i, 's, Input = ()>, Input> ConsNode<Root, Input> {
	pub fn new(root: Root) -> Self {
		ConsNode(root, PhantomData)
	}
}
