use dyn_any::StaticType;
use glam::DAffine2;
pub use graph_craft::proto::{Any, NodeContainer, TypeErasedBox, TypeErasedNode};
use graph_craft::proto::{DynFuture, FutureAny, SharedNodeContainer};
use graphene_core::Context;
use graphene_core::ContextDependency;
use graphene_core::NodeIO;
use graphene_core::OwnedContextImpl;
use graphene_core::WasmNotSend;
pub use graphene_core::registry::{DowncastBothNode, DynAnyNode, FutureWrapperNode, PanicNode};
use graphene_core::transform::Footprint;
pub use graphene_core::{Node, generic, ops};

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

pub struct EditorContextToContext {
	first: SharedNodeContainer,
}

impl<'i> Node<'i, Any<'i>> for EditorContextToContext {
	type Output = DynFuture<'i, Any<'i>>;
	fn eval(&'i self, input: Any<'i>) -> Self::Output {
		Box::pin(async move {
			let editor_context = dyn_any::downcast::<EditorContext>(input).unwrap();
			self.first.eval(Box::new(editor_context.to_context())).await
		})
	}
}

impl EditorContextToContext {
	pub const fn new(first: SharedNodeContainer) -> Self {
		EditorContextToContext { first }
	}
}

#[derive(Debug, Clone, Default)]
pub struct EditorContext {
	pub footprint: Option<Footprint>,
	pub downstream_transform: Option<DAffine2>,
	pub real_time: Option<f64>,
	pub animation_time: Option<f64>,
	pub index: Option<usize>,
	// #[serde(skip)]
	// pub editor_var_args: Option<(Vec<String>, Vec<Arc<Box<[dyn std::any::Any + 'static + std::panic::UnwindSafe]>>>)>,
}

unsafe impl StaticType for EditorContext {
	type Static = EditorContext;
}

// impl Default for EditorContext {
// 	fn default() -> Self {
// 		EditorContext {
// 			footprint: None,
// 			downstream_transform: None,
// 			real_time: None,
// 			animation_time: None,
// 			index: None,
// 			// editor_var_args: None,
// 		}
// 	}
// }

impl EditorContext {
	pub fn to_context(&self) -> Context {
		let mut context = OwnedContextImpl::default();
		if let Some(footprint) = self.footprint {
			context.set_footprint(footprint);
		}
		if let Some(footprint) = self.footprint {
			context.set_footprint(footprint);
		}
		// if let Some(downstream_transform) = self.downstream_transform {
		// 	context.set_downstream_transform(downstream_transform);
		// }
		if let Some(real_time) = self.real_time {
			context.set_real_time(real_time);
		}
		if let Some(animation_time) = self.animation_time {
			context.set_animation_time(animation_time);
		}
		if let Some(index) = self.index {
			context.set_index(index);
		}
		// if let Some(editor_var_args) = self.editor_var_args {
		// 	let (variable_names, values)
		// 	context.set_varargs((variable_names, values))
		// }
		context.into_context()
	}
}

pub struct NullificationNode {
	first: SharedNodeContainer,
	nullify: Vec<ContextDependency>,
}
impl<'i> Node<'i, Any<'i>> for NullificationNode {
	type Output = DynFuture<'i, Any<'i>>;

	fn eval(&'i self, input: Any<'i>) -> Self::Output {
		let new_input = match dyn_any::try_downcast::<Context>(input) {
			Ok(context) => match *context {
				Some(context) => {
					let mut new_context = OwnedContextImpl::from(context);
					new_context.nullify(&self.nullify);
					Box::new(new_context.into_context()) as Any<'i>
				}
				None => {
					let none: Context = None;
					Box::new(none) as Any<'i>
				}
			},
			Err(other_input) => other_input,
		};
		Box::pin(async move { self.first.eval(new_input).await })
	}
}

impl NullificationNode {
	pub fn new(first: SharedNodeContainer, nullify: Vec<ContextDependency>) -> Self {
		Self { first, nullify }
	}
}
