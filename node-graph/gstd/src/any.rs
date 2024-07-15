pub use graph_craft::proto::{Any, NodeContainer, TypeErasedBox, TypeErasedNode};
use graph_craft::proto::{DynFuture, FutureAny, SharedNodeContainer};
use graphene_core::NodeIO;
use graphene_core::WasmNotSend;
pub use graphene_core::{generic, ops, Node};

use dyn_any::StaticType;

use std::marker::PhantomData;

pub struct DynAnyNode<I, O, Node> {
	node: Node,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}

impl<'input, _I: 'input + StaticType + WasmNotSend, _O: 'input + StaticType + WasmNotSend, N: 'input> Node<'input, Any<'input>> for DynAnyNode<_I, _O, N>
where
	N: Node<'input, _I, Output = DynFuture<'input, _O>>,
{
	type Output = FutureAny<'input>;
	#[inline]
	fn eval(&'input self, input: Any<'input>) -> Self::Output {
		let node_name = core::any::type_name::<N>();
		let output = |input| {
			let result = self.node.eval(input);
			async move { Box::new(result.await) as Any<'input> }
		};
		match dyn_any::downcast(input) {
			Ok(input) => Box::pin(output(*input)),
			// If the input type of the node is `()` and we supply an invalid type, we can still call the
			// node and just ignore the input and call it with the unit type instead.
			Err(_) if core::any::TypeId::of::<_I::Static>() == core::any::TypeId::of::<()>() => {
				assert_eq!(std::mem::size_of::<_I>(), 0);
				// Rust can't know, that `_I` and `()` are the same size, so we have to use a `transmute_copy()` here
				Box::pin(output(unsafe { std::mem::transmute_copy(&()) }))
			}
			Err(e) => panic!("DynAnyNode Input, {0} in:\n{1}", e, node_name),
		}
	}

	fn reset(&self) {
		self.node.reset();
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn core::any::Any>> {
		self.node.serialize()
	}
}
impl<'input, _I: 'input + StaticType, _O: 'input + StaticType, N: 'input> DynAnyNode<_I, _O, N>
where
	N: Node<'input, _I, Output = DynFuture<'input, _O>>,
{
	pub const fn new(node: N) -> Self {
		Self {
			node,
			_i: core::marker::PhantomData,
			_o: core::marker::PhantomData,
		}
	}
}

pub struct DynAnyRefNode<I, O, Node> {
	node: Node,
	_i: PhantomData<(I, O)>,
}
impl<'input, _I: 'input + StaticType, _O: 'input + StaticType + WasmNotSend + Sync, N: 'input> Node<'input, Any<'input>> for DynAnyRefNode<_I, _O, N>
where
	N: for<'any_input> Node<'any_input, _I, Output = &'any_input _O>,
{
	type Output = FutureAny<'input>;
	fn eval(&'input self, input: Any<'input>) -> Self::Output {
		let node_name = core::any::type_name::<N>();
		let input: Box<_I> = dyn_any::downcast(input).unwrap_or_else(|e| panic!("DynAnyRefNode Input, {e} in:\n{node_name}"));
		let result = self.node.eval(*input);
		let output = async move { Box::new(result) as Any<'input> };
		Box::pin(output)
	}
	fn reset(&self) {
		self.node.reset();
	}
	fn serialize(&self) -> Option<std::sync::Arc<dyn core::any::Any>> {
		self.node.serialize()
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
impl<'input, _I: 'input + StaticType, _O: 'input + StaticType + WasmNotSend, N: 'input> Node<'input, Any<'input>> for DynAnyInRefNode<_I, _O, N>
where
	N: for<'any_input> Node<'any_input, &'any_input _I, Output = DynFuture<'any_input, _O>>,
{
	type Output = FutureAny<'input>;
	fn eval(&'input self, input: Any<'input>) -> Self::Output {
		{
			let node_name = core::any::type_name::<N>();
			let input: Box<&_I> = dyn_any::downcast(input).unwrap_or_else(|e| panic!("DynAnyInRefNode Input, {e} in:\n{node_name}"));
			let result = self.node.eval(*input);
			Box::pin(async move { Box::new(result.await) as Any<'_> })
		}
	}
}
impl<_I, _O, S0> DynAnyInRefNode<_I, _O, S0> {
	pub const fn new(node: S0) -> Self {
		Self { node, _i: core::marker::PhantomData }
	}
}

pub struct FutureWrapperNode<Node> {
	node: Node,
}

impl<'i, T: 'i + WasmNotSend, N> Node<'i, T> for FutureWrapperNode<N>
where
	N: Node<'i, T, Output: WasmNotSend> + WasmNotSend,
{
	type Output = DynFuture<'i, N::Output>;
	fn eval(&'i self, input: T) -> Self::Output {
		let result = self.node.eval(input);
		Box::pin(async move { result })
	}
	fn reset(&self) {
		self.node.reset();
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn core::any::Any>> {
		self.node.serialize()
	}
}

impl<N> FutureWrapperNode<N> {
	pub const fn new(node: N) -> Self {
		Self { node }
	}
}

pub trait IntoTypeErasedNode<'n> {
	fn into_type_erased(self) -> TypeErasedBox<'n>;
}

impl<'n, N: 'n> IntoTypeErasedNode<'n> for N
where
	N: for<'i> NodeIO<'i, Any<'i>, Output = FutureAny<'i>> + Sync + WasmNotSend,
{
	fn into_type_erased(self) -> TypeErasedBox<'n> {
		Box::new(self)
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
fn downcast<N: 'input, _O: StaticType>(input: Any<'input>, node: &'input N) -> _O
where
	N: for<'any_input> Node<'any_input, Any<'any_input>, Output = Any<'any_input>> + 'input,
{
	let node_name = core::any::type_name::<N>();
	let out = dyn_any::downcast(node.eval(input)).unwrap_or_else(|e| panic!("DowncastNode Input {e} in:\n{node_name}"));
	*out
}

/// Boxes the input and downcasts the output.
/// Wraps around a node taking Box<dyn DynAny> and returning Box<dyn DynAny>
#[derive(Clone)]
pub struct DowncastBothNode<I, O> {
	node: SharedNodeContainer,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}
impl<'input, O: 'input + StaticType + WasmNotSend, I: 'input + StaticType + WasmNotSend> Node<'input, I> for DowncastBothNode<I, O> {
	type Output = DynFuture<'input, O>;
	#[inline]
	fn eval(&'input self, input: I) -> Self::Output {
		{
			let node_name = self.node.node_name();
			let input = Box::new(input);
			let future = self.node.eval(input);
			Box::pin(async move {
				let out = dyn_any::downcast(future.await).unwrap_or_else(|e| panic!("DowncastBothNode Input {e} in: \n{node_name}"));
				*out
			})
		}
	}
}
impl<I, O> DowncastBothNode<I, O> {
	pub const fn new(node: SharedNodeContainer) -> Self {
		Self {
			node,
			_i: core::marker::PhantomData,
			_o: core::marker::PhantomData,
		}
	}
}

pub struct ComposeTypeErased {
	first: SharedNodeContainer,
	second: SharedNodeContainer,
}

impl<'i, 'a: 'i> Node<'i, Any<'i>> for ComposeTypeErased {
	type Output = DynFuture<'i, Any<'i>>;
	fn eval(&'i self, input: Any<'i>) -> Self::Output {
		Box::pin(async move {
			let arg = self.first.eval(input).await;
			self.second.eval(arg).await
		})
	}
}

impl ComposeTypeErased {
	pub const fn new(first: SharedNodeContainer, second: SharedNodeContainer) -> Self {
		ComposeTypeErased { first, second }
	}
}

pub fn input_node<O: StaticType>(n: SharedNodeContainer) -> DowncastBothNode<(), O> {
	downcast_node(n)
}
pub fn downcast_node<I: StaticType, O: StaticType>(n: SharedNodeContainer) -> DowncastBothNode<I, O> {
	DowncastBothNode::new(n)
}

pub struct PanicNode<I: WasmNotSend, O: WasmNotSend>(PhantomData<I>, PhantomData<O>);

impl<'i, I: 'i + WasmNotSend, O: 'i + WasmNotSend> Node<'i, I> for PanicNode<I, O> {
	type Output = O;
	fn eval(&'i self, _: I) -> Self::Output {
		unimplemented!("This node should never be evaluated")
	}
}

impl<I: WasmNotSend, O: WasmNotSend> PanicNode<I, O> {
	pub const fn new() -> Self {
		Self(PhantomData, PhantomData)
	}
}

impl<I: WasmNotSend, O: WasmNotSend> Default for PanicNode<I, O> {
	fn default() -> Self {
		Self::new()
	}
}

// TODO: Evaluate safety
unsafe impl<I: WasmNotSend, O: WasmNotSend> Sync for PanicNode<I, O> {}

#[cfg(test)]
mod test {
	use super::*;

	use graphene_core::{ops::AddPairNode, ops::IdentityNode};

	#[test]
	#[should_panic]
	pub fn dyn_input_invalid_eval_panic() {
		// let add = DynAnyNode::new(AddPairNode::new()).into_type_erased();
		// add.eval(Box::new(&("32", 32_u32)));
		let dyn_any = DynAnyNode::<(u32, u32), u32, _>::new(FutureWrapperNode { node: AddPairNode::new() });
		let type_erased = Box::new(dyn_any) as TypeErasedBox;
		let _ref_type_erased = type_erased.as_ref();
		// let type_erased = Box::pin(dyn_any) as TypeErasedBox<'_>;
		futures::executor::block_on(type_erased.eval(Box::new(&("32", 32_u32))));
	}

	#[test]
	pub fn dyn_input_compose() {
		// let add = DynAnyNode::new(AddPairNode::new()).into_type_erased();
		// add.eval(Box::new(&("32", 32_u32)));
		let dyn_any = DynAnyNode::<(u32, u32), u32, _>::new(FutureWrapperNode { node: AddPairNode::new() });
		let type_erased = Box::new(dyn_any) as TypeErasedBox<'_>;
		futures::executor::block_on(type_erased.eval(Box::new((4_u32, 2_u32))));
		let id_node = FutureWrapperNode::new(IdentityNode::new());
		let any_id = DynAnyNode::<u32, u32, _>::new(id_node);
		let type_erased_id = Box::new(any_id) as TypeErasedBox;
		let type_erased = ComposeTypeErased::new(NodeContainer::new(type_erased), NodeContainer::new(type_erased_id));
		futures::executor::block_on(type_erased.eval(Box::new((4_u32, 2_u32))));
		// let downcast: DowncastBothNode<(u32, u32), u32> = DowncastBothNode::new(type_erased.as_ref());
		// downcast.eval((4_u32, 2_u32));
	}

	// TODO: Fix this test
	// #[test]
	// pub fn dyn_input_storage_composition() {
	// 	// todo readd test
	// 	let node = <graphene_core::ops::IdentityNode>::new();
	// 	let any: DynAnyNode<Any<'_>, Any<'_>, _> = DynAnyNode::new(ValueNode::new(node));
	// 	any.into_type_erased();
	// }
}
