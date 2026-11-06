use core_types::WasmNotSend;
pub use core_types::registry::{DowncastBothNode, DynAnyNode, PanicNode};
pub use core_types::{Node, generic, ops};
use graph_craft::proto::SharedNodeContainer;
pub use graph_craft::proto::{Any, AnyInput, NodeContainer, TypeErasedBox, TypeErasedNode};

pub trait IntoTypeErasedNode<'n> {
	fn into_type_erased(self) -> TypeErasedBox<'n>;
}

impl<'n, N: 'n> IntoTypeErasedNode<'n> for N
where
	N: Node<AnyInput, Output = Any> + Sync + WasmNotSend,
{
	fn into_type_erased(self) -> TypeErasedBox<'n> {
		Box::new(self)
	}
}

pub fn input_node<O>(n: SharedNodeContainer) -> DowncastBothNode<(), O> {
	downcast_node(n)
}
pub fn downcast_node<I, O>(n: SharedNodeContainer) -> DowncastBothNode<I, O> {
	DowncastBothNode::new(n)
}
