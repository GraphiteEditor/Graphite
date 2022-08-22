/*
use core::marker::PhantomData;

use graphene_core::{structural::After, structural::ComposeNode, value::ValueNode, Node, RefNode};

use crate::any::Any;
use crate::any::DynAnyNode;

pub trait DocumentNode<I>: RefNode<I> {
	fn input_hints(&self) -> &'static [&'static str];
	fn input_types(&self) -> &'static [&'static str];
	fn inputs(&self) -> Vec<String> {
		self.input_hints().iter().zip(self.input_types()).map(|(a, b)| format!("{}{}", a, b)).collect()
	}
}

struct InjectNode<N: Node<I> + Copy, I>(N, PhantomData<I>);

impl<'n, N: Node<I> + Copy, I> Node<&'n [&'n AnyNode<'n>]> for &'n InjectNode<N, I> {
	type Output = Box<dyn RefNode<Any<'n>, Output = Any<'n>> + 'n>;
	fn eval(self, input: &'n [&'n AnyNode<'n>]) -> Self::Output {
		assert_eq!(input.len(), 1);
		Box::new(ComposeNode::new(&DynAnyNode(input[0]), &DynAnyNode(self.0)))
	}
}

impl<N: Node<I> + Copy, I> InjectNode<N, I> {
	const TYPES: &'static [&'static str] = &[core::any::type_name::<I>()];
	const HINTS: &'static [&'static str] = &["input: "];
}
impl<'n, N: Node<I> + Copy, I> DocumentNode<&'n [&'n AnyNode<'n>]> for &'n InjectNode<N, I> {
	fn input_hints(&self) -> &'static [&'static str] {
		InjectNode::<N, I>::HINTS
	}
	fn input_types(&self) -> &'static [&'static str] {
		InjectNode::<N, I>::TYPES
	}
}

pub type NodeId = u32;

type AnyNode<'n> = dyn RefNode<Any<'n>, Output = Any<'n>>;

pub struct DocumentGraphNode<'n> {
	pub id: NodeId,
	pub inputs: Vec<NodeInput>,
	pub node: NodeWrapper<'n>,
}

impl<'n> DocumentGraphNode<'n> {
	pub fn new(id: NodeId, inputs: Vec<NodeInput>, node: NodeWrapper<'n>) -> Self {
		Self { id, inputs, node }
	}
}

pub struct NodeWrapper<'n> {
	pub node: &'n (dyn Node<Any<'n>, Output = Any<'n>> + 'n),

	pub path: &'static str,
}

impl<'n> NodeWrapper<'n> {
	pub fn new(node: &'n (dyn Node<Any<'n>, Output = Any<'n>> + 'n), path: &'static str) -> Self {
		Self { node, path }
	}
}

pub enum NodeInput {
	Node(NodeId),
	Default(ValueNode<Any<'static>>),
}

#[cfg(test)]
mod test {
	use crate::any::DynAnyNode;

	use super::*;
	use graphene_core::value::ValueNode;

	#[test]
	fn inject_node() {
		let inject_node = InjectNode(&ValueNode(4u32), PhantomData);
		use super::DocumentNode;
		/*assert_eq!(
			(&inject_node as &dyn DocumentNode<&[&AnyNode], Output = ComposeNode<&AnyNode, ValueNode<u32>, ()>>).inputs(),
			vec!["input: ()"]
		);*/
		let any_inject = DynAnyNode(&inject_node, PhantomData);
		let any_inject = Box::leak(Box::new(any_inject));
		let wrapped = NodeWrapper::new((&any_inject) as &(dyn Node<&[&AnyNode], Output = Any>), "grahpene_core::document::InjectNode");
		let document_node = DocumentGraphNode::new(0, vec![], wrapped);
	}
}
*/
