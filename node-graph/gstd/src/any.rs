use dyn_any::StaticType;
pub use graph_craft::proto::{Any, NodeContainer, TypeErasedBox, TypeErasedNode};
use graph_craft::proto::{DynFuture, FutureAny, SharedNodeContainer};
use graphene_core::Context;
use graphene_core::ContextDependencies;
use graphene_core::EditorContext;
use graphene_core::NodeIO;
use graphene_core::OwnedContextImpl;
use graphene_core::WasmNotSend;
pub use graphene_core::registry::{DowncastBothNode, DynAnyNode, FutureWrapperNode, PanicNode};
use graphene_core::uuid::SNI;
pub use graphene_core::{Node, generic, ops};
use std::sync::mpsc::Sender;

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

pub struct ComposeTypeErased {
	first: SharedNodeContainer,
	second: SharedNodeContainer,
}

impl<'i> Node<'i, Any<'i>> for ComposeTypeErased {
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

// pub struct EditorContextToContext {
// 	first: SharedNodeContainer,
// }

// impl<'i> Node<'i, Any<'i>> for EditorContextToContext {
// 	type Output = DynFuture<'i, Any<'i>>;
// 	fn eval(&'i self, input: Any<'i>) -> Self::Output {
// 		Box::pin(async move {
// 			let editor_context = dyn_any::downcast::<EditorContext>(input).unwrap();
// 			self.first.eval(Box::new(editor_context.to_context())).await
// 		})
// 	}
// }

// impl EditorContextToContext {
// 	pub const fn new(first: SharedNodeContainer) -> Self {
// 		EditorContextToContext { first }
// 	}
// }

pub struct NullificationNode {
	first: SharedNodeContainer,
	nullify: ContextDependencies,
}
impl<'i> Node<'i, Any<'i>> for NullificationNode {
	type Output = DynFuture<'i, Any<'i>>;

	fn eval(&'i self, input: Any<'i>) -> Self::Output {
		Box::pin(async move {
			let new_input = match dyn_any::try_downcast::<Context>(input) {
				Ok(context) => match *context {
					Some(context) => {
						let mut new_context: OwnedContextImpl = OwnedContextImpl::from(context);
						// log::debug!("Nullifying context: {:?} fields: {:?}", new_context, self.nullify);
						new_context.nullify(&self.nullify);
						// log::debug!("Evaluating input with: {:?}", new_context);
						Box::new(new_context.into_context()) as Any<'i>
					}
					None => {
						let none: Context = None;
						Box::new(none) as Any<'i>
					}
				},
				Err(other_input) => other_input,
			};
			self.first.eval(new_input).await
		})
	}
}

impl NullificationNode {
	pub fn new(first: SharedNodeContainer, nullify: ContextDependencies) -> Self {
		Self { first, nullify }
	}
}

pub struct ContextMonitorNode {
	sni: SNI,
	input_index: usize,
	first: SharedNodeContainer,
	sender: Sender<(SNI, usize, EditorContext)>,
}

impl<'i> Node<'i, Any<'i>> for ContextMonitorNode {
	type Output = DynFuture<'i, Any<'i>>;
	fn eval(&'i self, input: Any<'i>) -> Self::Output {
		Box::pin(async move {
			let new_input = match dyn_any::try_downcast::<Context>(input) {
				Ok(context) => match *context {
					Some(context) => {
						let editor_context = context.to_editor_context();
						let _ = self.sender.clone().send((self.sni, self.input_index, editor_context));
						Box::new(Some(context)) as Any<'i>
					}
					None => {
						let none: Context = None;
						// self.sender.clone().send((self.sni, self.input_index, editor_context));
						Box::new(none) as Any<'i>
					}
				},
				Err(other_input) => other_input,
			};
			self.first.eval(new_input).await
		})
	}
}

impl ContextMonitorNode {
	pub fn new(sni: SNI, input_index: usize, first: SharedNodeContainer, sender: Sender<(SNI, usize, EditorContext)>) -> ContextMonitorNode {
		ContextMonitorNode { sni, input_index, first, sender }
	}
}
