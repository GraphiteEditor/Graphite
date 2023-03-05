use dyn_any::StaticType;
pub use graph_craft::proto::{Any, TypeErasedNode, TypeErasedPinned, TypeErasedPinnedRef};
use graphene_core::NodeIO;
pub use graphene_core::{generic, ops, Node};
use std::marker::PhantomData;

pub struct DynAnyNode<I, O, Node> {
	node: Node,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}
#[node_macro::node_fn(DynAnyNode<_I, _O>)]
fn any_node<_I: StaticType, _O: StaticType, N>(input: Any<'input>, node: &'any_input N) -> Any<'input>
where
	N: for<'any_input> Node<'any_input, _I, Output = _O>,
{
	let node_name = core::any::type_name::<N>();
	let input: Box<_I> = dyn_any::downcast(input).unwrap_or_else(|e| panic!("DynAnyNode Input, {e} in:\n{node_name}"));
	Box::new(node.eval(*input))
}
pub struct DynAnyRefNode<I, O, Node> {
	node: Node,
	_i: PhantomData<(I, O)>,
}
impl<'input, _I: 'input + StaticType, _O: 'input + StaticType, N: 'input> Node<'input, Any<'input>> for DynAnyRefNode<_I, _O, N>
where
	N: for<'any_input> Node<'any_input, _I, Output = &'any_input _O>,
{
	type Output = Any<'input>;
	fn eval<'node: 'input>(&'node self, input: Any<'input>) -> Self::Output {
		{
			let node_name = core::any::type_name::<N>();
			let input: Box<_I> = dyn_any::downcast(input).unwrap_or_else(|e| panic!("DynAnyNode Input, {e} in:\n{node_name}"));
			Box::new(self.node.eval(*input))
		}
	}
}
impl<_I, _O, S0> DynAnyRefNode<_I, _O, S0> {
	pub const fn new(node: S0) -> Self {
		Self { node, _i: core::marker::PhantomData }
	}
}
pub struct DynAnyInRefNode<I, O, Node> {
	node: Node,
	_i: PhantomData<(I, O)>,
}
impl<'input, _I: 'input + StaticType, _O: 'input + StaticType, N: 'input> Node<'input, Any<'input>> for DynAnyInRefNode<_I, _O, N>
where
	N: for<'any_input> Node<'any_input, &'any_input _I, Output = _O>,
{
	type Output = Any<'input>;
	fn eval<'node: 'input>(&'node self, input: Any<'input>) -> Self::Output {
		{
			let node_name = core::any::type_name::<N>();
			let input: Box<&_I> = dyn_any::downcast(input).unwrap_or_else(|e| panic!("DynAnyNode Input, {e} in:\n{node_name}"));
			Box::new(self.node.eval(*input))
		}
	}
}
impl<_I, _O, S0> DynAnyInRefNode<_I, _O, S0> {
	pub const fn new(node: S0) -> Self {
		Self { node, _i: core::marker::PhantomData }
	}
}

pub trait IntoTypeErasedNode<'n> {
	fn into_type_erased(self) -> TypeErasedPinned<'n>;
}

impl<'n, N: 'n> IntoTypeErasedNode<'n> for N
where
	N: for<'i> NodeIO<'i, Any<'i>, Output = Any<'i>> + Send + Sync + 'n,
{
	fn into_type_erased(self) -> TypeErasedPinned<'n> {
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
fn downcast<N, _O: StaticType>(input: Any<'input>, node: &'input N) -> _O
where
	N: for<'any_input> Node<'any_input, Any<'any_input>, Output = Any<'any_input>> + 'input,
{
	let node_name = core::any::type_name::<N>();
	let out = dyn_any::downcast(node.eval(input)).unwrap_or_else(|e| panic!("DowncastNode Input {e} in:\n{node_name}"));
	*out
}

/// Boxes the input and downcasts the output.
/// Wraps around a node taking Box<dyn DynAny> and returning Box<dyn DynAny>
#[derive(Clone, Copy)]
pub struct DowncastBothNode<'a, I, O> {
	node: TypeErasedPinnedRef<'a>,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}
impl<'n: 'input, 'input, O: 'input + StaticType, I: 'input + StaticType> Node<'input, I> for DowncastBothNode<'n, I, O> {
	type Output = O;
	#[inline]
	fn eval<'node: 'input>(&'node self, input: I) -> Self::Output {
		{
			let input = Box::new(input);
			let out = dyn_any::downcast(self.node.eval(input)).unwrap_or_else(|e| panic!("DowncastBothNode Input {e}"));
			*out
		}
	}
}
impl<'n, I, O> DowncastBothNode<'n, I, O> {
	pub const fn new(node: TypeErasedPinnedRef<'n>) -> Self {
		Self {
			node,
			_i: core::marker::PhantomData,
			_o: core::marker::PhantomData,
		}
	}
}
/// Boxes the input and downcasts the output.
/// Wraps around a node taking Box<dyn DynAny> and returning Box<dyn DynAny>
#[derive(Clone, Copy)]
pub struct DowncastBothRefNode<'a, I, O> {
	node: TypeErasedPinnedRef<'a>,
	_i: PhantomData<(I, O)>,
}
impl<'n: 'input, 'input, O: 'input + StaticType, I: 'input + StaticType> Node<'input, I> for DowncastBothRefNode<'n, I, O> {
	type Output = &'input O;
	#[inline]
	fn eval<'node: 'input>(&'node self, input: I) -> Self::Output {
		{
			let input = Box::new(input);
			let out: Box<&_> = dyn_any::downcast::<&O>(self.node.eval(input)).unwrap_or_else(|e| panic!("DowncastBothRefNode Input {e}"));
			*out
		}
	}
}
impl<'n, I, O> DowncastBothRefNode<'n, I, O> {
	pub const fn new(node: TypeErasedPinnedRef<'n>) -> Self {
		Self { node, _i: core::marker::PhantomData }
	}
}

pub struct ComposeTypeErased<'a> {
	first: TypeErasedPinnedRef<'a>,
	second: TypeErasedPinnedRef<'a>,
}

impl<'i, 'a: 'i> Node<'i, Any<'i>> for ComposeTypeErased<'a> {
	type Output = Any<'i>;
	fn eval<'s: 'i>(&'s self, input: Any<'i>) -> Self::Output {
		let arg = self.first.eval(input);
		self.second.eval(arg)
	}
}

impl<'a> ComposeTypeErased<'a> {
	pub const fn new(first: TypeErasedPinnedRef<'a>, second: TypeErasedPinnedRef<'a>) -> Self {
		ComposeTypeErased { first, second }
	}
}

pub fn input_node<O: StaticType>(n: TypeErasedPinnedRef) -> DowncastBothNode<(), O> {
	DowncastBothNode::new(n)
}

#[cfg(test)]
mod test {
	use super::*;
	use graphene_core::{ops::AddNode, ops::IdNode, value::ValueNode};

	#[test]
	#[should_panic]
	pub fn dyn_input_invalid_eval_panic() {
		//let add = DynAnyNode::new(AddNode::new()).into_type_erased();
		//add.eval(Box::new(&("32", 32u32)));
		let dyn_any = DynAnyNode::<(u32, u32), u32, _>::new(ValueNode::new(AddNode::new()));
		let type_erased = dyn_any.into_type_erased();
		let _ref_type_erased = type_erased.as_ref();
		//let type_erased = Box::pin(dyn_any) as TypeErasedPinned<'_>;
		type_erased.eval(Box::new(&("32", 32u32)));
	}

	#[test]
	pub fn dyn_input_invalid_eval_panic_() {
		//let add = DynAnyNode::new(AddNode::new()).into_type_erased();
		//add.eval(Box::new(&("32", 32u32)));
		let dyn_any = DynAnyNode::<(u32, u32), u32, _>::new(ValueNode::new(AddNode::new()));
		let type_erased = Box::pin(dyn_any) as TypeErasedPinned<'_>;
		type_erased.eval(Box::new((4u32, 2u32)));
		let id_node = IdNode::new();
		let type_erased_id = Box::pin(id_node) as TypeErasedPinned;
		let type_erased = ComposeTypeErased::new(type_erased.as_ref(), type_erased_id.as_ref());
		type_erased.eval(Box::new((4u32, 2u32)));
		//let downcast: DowncastBothNode<(u32, u32), u32> = DowncastBothNode::new(type_erased.as_ref());
		//downcast.eval((4u32, 2u32));
	}

	// TODO: Fix this test
	/*
	#[test]
	pub fn dyn_input_storage_composition() {
		// todo readd test
		let node = <graphene_core::ops::IdNode>::new();
		let any: DynAnyNode<Any<'_>, Any<'_>, _> = DynAnyNode::new(ValueNode::new(node));
		any.into_type_erased();
	}
	*/
}
