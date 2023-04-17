use core::marker::PhantomData;

use crate::Node;

pub struct ComposeNode<First: for<'i> Node<'i, I>, Second: for<'i> Node<'i, <First as Node<'i, I>>::Output>, I> {
	first: First,
	second: Second,
	phantom: PhantomData<I>,
}

impl<'i, Input: 'i, First, Second> Node<'i, Input> for ComposeNode<First, Second, Input>
where
	First: for<'a> Node<'a, Input> + 'i,
	Second: for<'a> Node<'a, <First as Node<'a, Input>>::Output> + 'i,
{
	type Output = <Second as Node<'i, <First as Node<'i, Input>>::Output>>::Output;
	fn eval(&'i self, input: Input) -> Self::Output {
		let arg = self.first.eval(input);
		self.second.eval(arg)
	}
}

impl<First, Second, Input> ComposeNode<First, Second, Input>
where
	First: for<'a> Node<'a, Input>,
	Second: for<'a> Node<'a, <First as Node<'a, Input>>::Output>,
{
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<First, Second, Input> { first, second, phantom: PhantomData }
	}
}

// impl Clone for ComposeNode<First, Second, Input>
impl<First, Second, Input> Clone for ComposeNode<First, Second, Input>
where
	First: for<'a> Node<'a, Input> + Clone,
	Second: for<'a> Node<'a, <First as Node<'a, Input>>::Output> + Clone,
{
	fn clone(&self) -> Self {
		ComposeNode::<First, Second, Input> {
			first: self.first.clone(),
			second: self.second.clone(),
			phantom: PhantomData,
		}
	}
}

pub trait Then<'i, Input: 'i>: Sized {
	fn then<Second>(self, second: Second) -> ComposeNode<Self, Second, Input>
	where
		Self: for<'a> Node<'a, Input>,
		Second: for<'a> Node<'a, <Self as Node<'a, Input>>::Output>,
	{
		ComposeNode::new(self, second)
	}
}

impl<'i, First: for<'a> Node<'a, Input>, Input: 'i> Then<'i, Input> for First {}

pub struct ConsNode<I: From<()>, Root>(pub Root, PhantomData<I>);

impl<'i, Root, Input: 'i, I: 'i + From<()>> Node<'i, Input> for ConsNode<I, Root>
where
	Root: Node<'i, I>,
{
	type Output = (Input, Root::Output);
	fn eval(&'i self, input: Input) -> Self::Output {
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
		let compose = value.then(IdNode::new());
		assert_eq!(compose.eval(()), &4u32);
		let type_erased = &compose as &dyn for<'i> Node<'i, (), Output = &'i u32>;
		assert_eq!(type_erased.eval(()), &4u32);
	}
}
