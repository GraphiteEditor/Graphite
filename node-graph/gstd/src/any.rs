pub use graph_craft::proto::{Any, NodeContainer, TypeErasedBox, TypeErasedNode};
use graph_craft::proto::{DynFuture, FutureAny, SharedNodeContainer};
use graphene_core::NodeIO;
use graphene_core::WasmNotSend;
pub use graphene_core::{generic, ops, Node};

use dyn_any::StaticType;

use std::marker::PhantomData;

pub use graphene_core::registry::{DowncastBothNode, DynAnyNode, FutureWrapperNode, PanicNode};

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
