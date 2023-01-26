use dyn_any::{DynAny, StaticType};
pub use graphene_core::{generic, ops /*, structural*/, Node, NodeIO};
use std::{marker::PhantomData, pin::Pin};

pub struct DynAnyNode<I, O, Node> {
	node: Node,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}
#[node_macro::node_fn(DynAnyNode<_I, _O>)]
fn any_node<_I: StaticType, _O: StaticType, N>(input: Any<'input>, node: &'node N) -> Any<'input>
where
	N: Node<'input, 'node, _I, Output = _O> + 'node,
{
	let node_name = core::any::type_name::<N>();
	let input: Box<_I> = dyn_any::downcast(input).unwrap_or_else(|_| panic!("DynAnyNode Input in:\n{node_name}"));
	Box::new(node.eval(*input))
}

type TypeErasedNode<'n> = Pin<Box<dyn for<'i, 's> Node<'i, 's, Any<'i>, Output = Any<'i>> + 'n>>;

pub trait IntoTypeErasedNode<'n> {
	fn into_type_erased(self) -> TypeErasedNode<'n>;
}

impl<'n, N: 'n> IntoTypeErasedNode<'n> for N
where
	N: for<'i, 's> Node<'i, 's, Any<'i>, Output = Any<'i>>,
{
	fn into_type_erased(self) -> TypeErasedNode<'n> {
		Box::pin(self)
	}
}

pub struct DowncastNode<O, Node> {
	node: Node,
	_o: PhantomData<O>,
}
impl<N: Clone, O: StaticType> Clone for DowncastNode<O, N> {
	fn clone(&self) -> Self {
		Self { node: self.node.clone(), _o: self._o }
	}
}
impl<N: Copy, O: StaticType> Copy for DowncastNode<O, N> {}

#[node_macro::node_fn(DowncastNode<_O>)]
fn downcast<N, _O: StaticType>(input: Any<'input>, node: &'node N) -> _O
where
	N: Node<'input, 'node, Any<'input>, Output = Any<'input>> + 'node,
{
	let node_name = core::any::type_name::<N>();
	let out = dyn_any::downcast(node.eval(input)).unwrap_or_else(|_| panic!("DynAnyNode Input in:\n{node_name}"));
	*out
}

/// Boxes the input and downcasts the output.
/// Wraps around a node taking Box<dyn DynAny> and returning Box<dyn DynAny>
pub struct DowncastBothNode<I, O, Node> {
	node: Node,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}
impl<N: Clone, I: StaticType, O: StaticType> Clone for DowncastBothNode<I, O, N> {
	fn clone(&self) -> Self {
		Self {
			node: self.node.clone(),
			_i: self._i,
			_o: self._o,
		}
	}
}
impl<N: Copy, I: StaticType, O: StaticType> Copy for DowncastBothNode<I, O, N> {}

#[node_macro::node_fn(DowncastBothNode<_I,_O>)]
fn downcast_both<N, _O: StaticType, _I: StaticType>(input: _I, node: &'node N) -> _O
where
	N: Node<'input, 'node, Any<'input>, Output = Any<'input>> + 'node,
{
	let node_name = core::any::type_name::<N>();
	let input = Box::new(input);
	let out = dyn_any::downcast(node.eval(input)).unwrap_or_else(|_| panic!("DynAnyNode Input in:\n{node_name}"));
	*out
}

pub type Any<'n> = Box<dyn DynAny<'n> + 'n>;

#[cfg(test)]
mod test {
	use super::*;
	use graphene_core::{ops::AddNode, value::ValueNode};

	#[test]
	#[should_panic]
	pub fn dyn_input_invalid_eval_panic() {
		static ADD: &DynAnyNode<(u32, u32), u32, AddNode> = &DynAnyNode::new(AddNode::new());

		//let add = DynAnyNode::new(AddNode::new()).into_type_erased();
		//add.eval(Box::new(&("32", 32u32)));
		DynAnyNode::<(u32, u32), u32, _>::new(ValueNode::new(AddNode::new())).eval(Box::new(&("32", 32u32)));
	}

	#[test]
	pub fn dyn_input_storage_composition() {
		// todo readd test
	}
}
