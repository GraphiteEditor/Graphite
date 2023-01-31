use core::marker::PhantomData;

use dyn_any::StaticTypeSized;

use crate::Node;

#[derive(Debug, Clone)]
pub struct ComposeNode<First, Second> {
	first: First,
	second: Second,
}

impl<'i, Input: 'i, First, Second> Node<'i, Input> for ComposeNode<First, Second>
where
	First: for<'a> Node<'a, Input>,
	Second: for<'a> Node<'a, <First as Node<'a, Input>>::Output>,
{
	type Output = <Second as Node<'i, <First as Node<'i, Input>>::Output>>::Output;
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

impl<First, Second> ComposeNode<First, Second> {
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<First, Second> { first, second }
	}
}
pub trait Then<'i, Input: 'i>: Sized {
	fn then<Second>(self, second: Second) -> ComposeNode<Self, Second>
	where
		Self: for<'a> Node<'a, Input>,
		Second: for<'a> Node<'a, <Self as Node<'a, Input>>::Output>,
	{
		ComposeNode::<Self, Second> { first: self, second }
	}
}

impl<'i, First: for<'a> Node<'a, Input>, Input: 'i> Then<'i, Input> for First {}

pub struct ConsNode<I: From<()>, Root>(pub Root, PhantomData<I>);

impl<'i, Root, Input: 'i, I: 'i + From<()>> Node<'i, Input> for ConsNode<I, Root>
where
	Root: Node<'i, I>,
{
	type Output = (Input, Root::Output);
	fn eval<'s: 'i>(&'s self, input: Input) -> Self::Output {
		let arg = self.0.eval(I::from(()));
		(input, arg)
	}
}
impl<'i, Root: Node<'i, I>, I: 'i + From<()>> ConsNode<I, Root> {
	pub fn new(root: Root) -> Self {
		ConsNode(root, PhantomData)
	}
}

#[cfg(test)]
mod test {
	use crate::{ops::IdNode, value::ValueNode};

	use super::*;

	#[test]
	fn compose() {
		let value = ValueNode::new(4u32);
		let compose = ComposeNode::new(value, IdNode::new());
		assert_eq!(compose.eval(()), &4u32);
		let type_erased = &compose as &dyn for<'i> Node<'i, (), Output = &u32>;
		assert_eq!(type_erased.eval(()), &4u32);
	}
}
