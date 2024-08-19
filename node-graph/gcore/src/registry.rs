use crate::{ProtoNodeIdentifier, Type};
use std::collections::HashMap;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::{LazyLock, Mutex};

use dyn_any::DynAny;

use crate::NodeIO;
use crate::NodeIOTypes;

#[derive(Clone)]
pub struct NodeMetadata {
	pub identifier: ProtoNodeIdentifier,
	pub category: Option<&'static str>,
	pub input_type: Type,
	pub output_type: Type,
	pub fields: Vec<FieldMetadata>,
}

#[derive(Clone)]
pub struct FieldMetadata {
	pub name: String,
	pub default_value: Option<&'static str>,
}
pub static NODE_REGISTRY: LazyLock<Mutex<HashMap<ProtoNodeIdentifier, Vec<(NodeConstructor, NodeIOTypes)>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub static NODE_METADATA: LazyLock<Mutex<HashMap<ProtoNodeIdentifier, NodeMetadata>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(not(target_arch = "wasm32"))]
pub type DynFuture<'n, T> = Pin<Box<dyn core::future::Future<Output = T> + 'n + Send>>;
#[cfg(target_arch = "wasm32")]
pub type DynFuture<'n, T> = Pin<Box<dyn core::future::Future<Output = T> + 'n>>;
pub type LocalFuture<'n, T> = Pin<Box<dyn core::future::Future<Output = T> + 'n>>;
#[cfg(not(target_arch = "wasm32"))]
pub type Any<'n> = Box<dyn DynAny<'n> + 'n + Send>;
#[cfg(target_arch = "wasm32")]
pub type Any<'n> = Box<dyn DynAny<'n> + 'n>;
pub type FutureAny<'n> = DynFuture<'n, Any<'n>>;
// TODO: is this safe? This is assumed to be send+sync.
#[cfg(not(target_arch = "wasm32"))]
pub type TypeErasedNode<'n> = dyn for<'i> NodeIO<'i, Any<'i>, Output = FutureAny<'i>> + 'n + Send + Sync;
#[cfg(target_arch = "wasm32")]
pub type TypeErasedNode<'n> = dyn for<'i> NodeIO<'i, Any<'i>, Output = FutureAny<'i>> + 'n;
pub type TypeErasedPinnedRef<'n> = Pin<&'n TypeErasedNode<'n>>;
pub type TypeErasedRef<'n> = &'n TypeErasedNode<'n>;
pub type TypeErasedBox<'n> = Box<TypeErasedNode<'n>>;
pub type TypeErasedPinned<'n> = Pin<Box<TypeErasedNode<'n>>>;

pub type SharedNodeContainer = std::sync::Arc<NodeContainer>;

pub type NodeConstructor = fn(Vec<SharedNodeContainer>) -> DynFuture<'static, TypeErasedBox<'static>>;

#[derive(Clone)]
pub struct NodeContainer {
	#[cfg(feature = "dealloc_nodes")]
	pub node: *const TypeErasedNode<'static>,
	#[cfg(not(feature = "dealloc_nodes"))]
	pub node: TypeErasedRef<'static>,
}

impl Deref for NodeContainer {
	type Target = TypeErasedNode<'static>;

	#[cfg(feature = "dealloc_nodes")]
	fn deref(&self) -> &Self::Target {
		unsafe { &*(self.node) }
		#[cfg(not(feature = "dealloc_nodes"))]
		self.node
	}
	#[cfg(not(feature = "dealloc_nodes"))]
	fn deref(&self) -> &Self::Target {
		self.node
	}
}

/// #Safety
/// Marks NodeContainer as Sync. This dissallows the use of threadlocal stroage for nodes as this would invalidate references to them.
// TODO: implement this on a higher level wrapper to avoid missuse
#[cfg(feature = "dealloc_nodes")]
unsafe impl Send for NodeContainer {}
#[cfg(feature = "dealloc_nodes")]
unsafe impl Sync for NodeContainer {}

#[cfg(feature = "dealloc_nodes")]
impl Drop for NodeContainer {
	fn drop(&mut self) {
		unsafe { self.dealloc_unchecked() }
	}
}

impl core::fmt::Debug for NodeContainer {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NodeContainer").finish()
	}
}

impl NodeContainer {
	pub fn new(node: TypeErasedBox<'static>) -> SharedNodeContainer {
		let node = Box::leak(node);
		Self { node }.into()
	}

	#[cfg(feature = "dealloc_nodes")]
	unsafe fn dealloc_unchecked(&mut self) {
		std::mem::drop(Box::from_raw(self.node as *mut TypeErasedNode));
	}
}

use crate::Node;
use crate::WasmNotSend;
use dyn_any::StaticType;
use std::marker::PhantomData;

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
	fn reset(&self) {
		self.node.reset();
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn core::any::Any>> {
		self.node.serialize()
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

mod construct_vector2 {
	use super::*;
	use crate::registry::{DowncastBothNode, DynAnyNode, FieldMetadata, FutureWrapperNode, NodeMetadata, NODE_METADATA, NODE_REGISTRY};
	use crate::{concrete, fn_type, Node, NodeIOTypes, ProtoNodeIdentifier};
	use core::future::Future;
	use ctor::ctor;
	pub struct ConstructVector2<Node0, Node1, Node2> {
		x: Node0,
		y: Node1,
		c: Node2,
	}
	#[allow(non_snake_case)]
	async fn construct_vector2<'n, IY: Into<f64>>(_: (), x: f64, y: IY, c: impl Node<'n, (), Output: Future<Output = u32>>) -> glam::DVec2 {
		glam::DVec2::new(x, y.into())
	}
	impl<'n, IY: Into<f64>, Node0, Node1, Node2> Node<'n, ()> for ConstructVector2<Node0, Node1, Node2>
	where
		Node0: Node<'n, (), Output = f64>,
		Node1: Node<'n, (), Output = IY>,
		Node2: Node<'n, (), Output: Future<Output = u32>>,
	{
		type Output = Pin<Box<dyn Future<Output = glam::DVec2> + 'n>>;
		fn eval(&'n self, input: ()) -> Self::Output {
			Box::pin(async move {
				let x = self.x.eval(());
				let y = self.y.eval(());
				let c = &self.c;
				construct_vector2(input, x, y, c).await
			})
		}
	}
	impl<'n, Node0, Node1, Node2> ConstructVector2<Node0, Node1, Node2> {
		pub fn new(x: Node0, y: Node1, c: Node2) -> Self {
			Self { x, y, c }
		}
	}
	#[ctor]
	fn register_node() {
		let mut registry = NODE_REGISTRY.lock().unwrap();
		registry.insert(
			ProtoNodeIdentifier::new(concat![std::module_path!(), "::", stringify!(ConstructVector2)]),
			vec![
				(
					|args| {
						Box::pin(async move {
							let x: DowncastBothNode<(), f64> = DowncastBothNode::new(args[0usize].clone());
							let y: DowncastBothNode<(), f32> = DowncastBothNode::new(args[1usize].clone());
							let c: DowncastBothNode<(), u32> = DowncastBothNode::new(args[2usize].clone());
							let node = ConstructVector2::new(x, y, c);
							let any: DynAnyNode<(), _, _> = DynAnyNode::new(node);
							any.into_type_erased()
						})
					},
					NodeIOTypes::new(concrete!(()), concrete!(glam::DVec2), vec![fn_type!((), f64), fn_type!((), f32), fn_type!((), u32),],)
				);
				(
					|args| {
						Box::pin(async move {
							let x: DowncastBothNode<(), f64> = DowncastBothNode::new(args[0usize].clone());
							let y: DowncastBothNode<(), f64> = DowncastBothNode::new(args[1usize].clone());
							let c: DowncastBothNode<(), u64> = DowncastBothNode::new(args[2usize].clone());
							let node = ConstructVector2::new(x, y, c);
							let any: DynAnyNode<(), _, _> = DynAnyNode::new(node);
							any.into_type_erased()
						})
					},
					NodeIOTypes::new(concrete!(()), concrete!(glam::DVec2), vec![fn_type!((), f64), fn_type!((), f64), fn_type!((), u64),],)
				)
			],
		);
	}
	#[ctor]
	fn register_metadata() {
		let metadata = NodeMetadata {
			identifier: ProtoNodeIdentifier::new(concat![std::module_path!(), "::", stringify!(ConstructVector2)]),
			category: Some("Value"),
			input_type: concrete!(()),
			output_type: concrete!(glam::DVec2),
			fields: vec![
				FieldMetadata {
					name: stringify!(x).to_string(),
					default_value: Some(stringify!(1.3)),
				},
				FieldMetadata {
					name: stringify!(y).to_string(),
					default_value: None,
				},
				FieldMetadata {
					name: stringify!(c).to_string(),
					default_value: None,
				},
			],
		};
		NODE_METADATA
			.lock()
			.unwrap()
			.insert(ProtoNodeIdentifier::new(concat![std::module_path!(), "::", stringify!(ConstructVector2)]), metadata);
	}
}
