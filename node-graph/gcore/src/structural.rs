use core::marker::PhantomData;

use crate::Node;

#[derive(Clone)]
pub struct ComposeNode<First, Second, I> {
	first: First,
	second: Second,
	phantom: PhantomData<I>,
}

impl<'i, 'f: 'i, 's: 'i, Input: 'i, First, Second> Node<'i, Input> for ComposeNode<First, Second, Input>
where
	First: Node<'i, Input>,
	Second: Node<'i, <First as Node<'i, Input>>::Output> + 'i,
{
	type Output = <Second as Node<'i, <First as Node<'i, Input>>::Output>>::Output;
	fn eval(&'i self, input: Input) -> Self::Output {
		let arg = self.first.eval(input);
		let second = &self.second;
		second.eval(arg)
	}
}

impl<'i, First, Second, Input: 'i> ComposeNode<First, Second, Input>
where
	First: Node<'i, Input>,
	Second: Node<'i, <First as Node<'i, Input>>::Output>,
{
	pub const fn new(first: First, second: Second) -> Self {
		ComposeNode::<First, Second, Input> { first, second, phantom: PhantomData }
	}
}

#[derive(Clone)]
pub struct AsyncComposeNode<First, Second, I> {
	first: First,
	second: Second,
	phantom: PhantomData<I>,
}

impl<'i, 'f: 'i, 's: 'i, Input: 'static, First, Second> Node<'i, Input> for AsyncComposeNode<First, Second, Input>
where
	First: Node<'i, Input>,
	First::Output: core::future::Future,
	Second: Node<'i, <<First as Node<'i, Input>>::Output as core::future::Future>::Output> + 'i,
{
	type Output = core::pin::Pin<Box<dyn core::future::Future<Output = <Second as Node<'i, <<First as Node<'i, Input>>::Output as core::future::Future>::Output>>::Output> + 'i>>;
	fn eval(&'i self, input: Input) -> Self::Output {
		Box::pin(async move {
			let arg = self.first.eval(input).await;
			self.second.eval(arg)
		})
	}
}

impl<'i, First, Second, Input: 'i> AsyncComposeNode<First, Second, Input>
where
	First: Node<'i, Input>,
	First::Output: core::future::Future,
	Second: Node<'i, <<First as Node<'i, Input>>::Output as core::future::Future>::Output> + 'i,
{
	pub const fn new(first: First, second: Second) -> Self {
		AsyncComposeNode::<First, Second, Input> { first, second, phantom: PhantomData }
	}
}

pub trait Then<'i, Input: 'i>: Sized {
	fn then<Second>(self, second: Second) -> ComposeNode<Self, Second, Input>
	where
		Self: Node<'i, Input>,
		Second: Node<'i, <Self as Node<'i, Input>>::Output>,
	{
		ComposeNode::new(self, second)
	}
}

impl<'i, First: Node<'i, Input>, Input: 'i> Then<'i, Input> for First {}

pub trait AndThen<'i, Input: 'i>: Sized {
	fn and_then<Second>(self, second: Second) -> AsyncComposeNode<Self, Second, Input>
	where
		Self: Node<'i, Input>,
		Self::Output: core::future::Future,
		Second: Node<'i, <<Self as Node<'i, Input>>::Output as core::future::Future>::Output> + 'i,
	{
		AsyncComposeNode::new(self, second)
	}
}

impl<'i, First: Node<'i, Input>, Input: 'i> AndThen<'i, Input> for First {}

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

	#[test]
	fn test_ref_eval() {
		let value = ValueNode::new(5);

		assert_eq!(value.eval(()), &5);
		let id = IdNode::new();

		let compose = ComposeNode::new(&value, &id);

		assert_eq!(compose.eval(()), &5);
	}
}
