use core::marker::PhantomData;

use crate::{Node, NodeMut};

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
impl<'i, 'f: 'i, 's: 'i, Input: 'i, First, Second> NodeMut<'i, Input> for ComposeNode<First, Second, Input>
where
	First: Node<'i, Input>,
	Second: NodeMut<'i, <First as Node<'i, Input>>::Output> + 'i,
{
	type MutOutput = <Second as NodeMut<'i, <First as Node<'i, Input>>::Output>>::MutOutput;
	fn eval_mut(&'i mut self, input: Input) -> Self::MutOutput {
		let arg = self.first.eval(input);
		let second = &mut self.second;
		second.eval_mut(arg)
	}
}

impl<'i, First, Second, Input: 'i> ComposeNode<First, Second, Input> {
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

#[cfg(feature = "alloc")]
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

#[cfg(feature = "alloc")]
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

#[cfg(feature = "alloc")]
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

#[cfg(feature = "alloc")]
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

pub struct ApplyNode<O, N> {
	pub node: N,
	_o: PhantomData<O>,
}
/*
#[node_macro::node_fn(ApplyNode)]
fn apply<In, N>(input: In, node: &'any_input N) -> ()
where
	// TODO: try to allows this to return output other than ()
	N: for<'any_input> Node<'any_input, In, Output = ()>,
{
	node.eval(input)
}
*/
impl<'input, In: 'input, N: 'input, S0: 'input, O: 'input> Node<'input, In> for ApplyNode<O, S0>
where
	N: Node<'input, In, Output = O>,
	S0: Node<'input, (), Output = &'input N>,
{
	type Output = <N as Node<'input, In>>::Output;
	#[inline]
	fn eval(&'input self, input: In) -> Self::Output {
		let node = self.node.eval(());
		node.eval(input)
	}
}
impl<'input, S0: 'input, O: 'static> ApplyNode<O, S0> {
	pub const fn new(node: S0) -> Self {
		Self { node, _o: PhantomData }
	}
}

#[cfg(test)]
mod test {
	use crate::{ops::IdentityNode, value::ValueNode};

	use super::*;

	#[test]
	fn compose() {
		let value = ValueNode::new(4u32);
		let compose = value.then(IdentityNode::new());
		assert_eq!(compose.eval(()), &4u32);
		let type_erased = &compose as &dyn for<'i> Node<'i, (), Output = &'i u32>;
		assert_eq!(type_erased.eval(()), &4u32);
	}

	#[test]
	fn test_ref_eval() {
		let value = ValueNode::new(5);

		assert_eq!(value.eval(()), &5);
		let id = IdentityNode::new();

		let compose = ComposeNode::new(&value, &id);

		assert_eq!(compose.eval(()), &5);
	}

	#[test]
	#[allow(clippy::unit_cmp)]
	fn test_apply() {
		let mut array = [1, 2, 3];
		let slice = &mut array;
		let set_node = crate::storage::SetOwnedNode::new(slice);

		let apply = ApplyNode::new(ValueNode::new(set_node));

		assert_eq!(apply.eval((1, 2)), ());
	}
}
