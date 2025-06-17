use crate::Node;
use std::marker::PhantomData;

/// This is how we can generically define composition of two nodes.
/// This is done generically as shown: <https://files.keavon.com/-/SurprisedGaseousAnhinga/capture.png>
/// A concrete example: <https://files.keavon.com/-/ExcitableGoldRay/capture.png>
/// And showing the direction of data flow: <https://files.keavon.com/-/SoreShimmeringElephantseal/capture.png>
/// ```text
///                       ┌────────────────┐
///                 T     │                │     U
///           ───────────►│  Compose Node  ├───────────►
///                       │                │
///                       └────┬───────────┤
///  ┌──────────┐              │           │
///  │          │    T -> V    │           │
///  │  First   ├─────────────►│           │
///  │          │              │           │
///  └──────────┘              │           │
///  ┌──────────┐              │           │
///  │          │    V -> U    │           │
///  │  Second  ├─────────────►│           │
///  │          │              └───────────┘
///  └──────────┘
/// ```
#[derive(Clone, Copy)]
pub struct ComposeNode<First, Second, I> {
	first: First,
	second: Second,
	phantom: PhantomData<I>,
}

impl<'i, Input: 'i, First, Second> Node<'i, Input> for ComposeNode<First, Second, Input>
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

impl<First, Second, Input> ComposeNode<First, Second, Input> {
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

impl<'i, Input: 'static, First, Second> Node<'i, Input> for AsyncComposeNode<First, Second, Input>
where
	First: Node<'i, Input>,
	First::Output: Future,
	Second: Node<'i, <<First as Node<'i, Input>>::Output as Future>::Output> + 'i,
{
	type Output = std::pin::Pin<Box<dyn Future<Output = <Second as Node<'i, <<First as Node<'i, Input>>::Output as Future>::Output>>::Output> + 'i>>;
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
	First::Output: Future,
	Second: Node<'i, <<First as Node<'i, Input>>::Output as Future>::Output> + 'i,
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
		Self::Output: Future,
		Second: Node<'i, <<Self as Node<'i, Input>>::Output as Future>::Output> + 'i,
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
	use super::*;
	use crate::generic::FnNode;
	use crate::value::ValueNode;

	#[test]
	fn compose() {
		let value = ValueNode::new(4u32);
		let compose = value.then(FnNode::new(|x| x));
		assert_eq!(compose.eval(()), &4u32);
		let type_erased = &compose as &dyn Node<'_, (), Output = &'_ u32>;
		assert_eq!(type_erased.eval(()), &4u32);
	}

	#[test]
	fn test_ref_eval() {
		let value = ValueNode::new(5);

		assert_eq!(value.eval(()), &5);
		let id = FnNode::new(|x| x);

		let compose = ComposeNode::new(&value, &id);

		assert_eq!(compose.eval(()), &5);
	}
}
