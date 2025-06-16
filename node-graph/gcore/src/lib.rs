extern crate alloc;

#[cfg_attr(feature = "log", macro_use)]
#[cfg(feature = "log")]
extern crate log;
pub use crate as graphene_core;
pub use num_traits;

pub use ctor;

pub mod animation;
pub mod consts;
pub mod context;
pub mod generic;
pub mod instances;
pub mod logic;
pub mod misc;
pub mod ops;
pub mod structural;
pub mod text;
pub mod uuid;
pub mod value;

pub mod memo;

pub mod raster;
pub mod transform;

mod graphic_element;
pub use graphic_element::*;
pub mod vector;

pub mod application_io;

pub mod registry;

pub use context::*;
use core::any::TypeId;
use core::future::Future;
use core::pin::Pin;
pub use dyn_any::{StaticTypeSized, WasmNotSend, WasmNotSync};
pub use memo::MemoHash;
pub use raster::Color;
pub use types::Cow;

// pub trait Node: for<'n> NodeIO<'n> {
/// The node trait allows for defining any node. Nodes can only take one call argument input, however they can store references to other nodes inside the struct.
/// See `node-graph/README.md` for information on how to define a new node.
pub trait Node<'i, Input> {
	type Output: 'i;
	/// Evaluates the node with the single specified input.
	fn eval(&'i self, input: Input) -> Self::Output;
	/// Resets the node, e.g. the LetNode's cache is set to None.
	fn reset(&self) {}
	/// Returns the name of the node for diagnostic purposes.
	fn node_name(&self) -> &'static str {
		core::any::type_name::<Self>()
	}
	/// Serialize the node which is used for the `introspect` function which can retrieve values from monitor nodes.
	fn serialize(&self) -> Option<std::sync::Arc<dyn core::any::Any + Send + Sync>> {
		log::warn!("Node::serialize not implemented for {}", core::any::type_name::<Self>());
		None
	}
}

mod types;
pub use types::*;

pub trait NodeIO<'i, Input>: Node<'i, Input>
where
	Self::Output: 'i + StaticTypeSized,
	Input: StaticTypeSized,
{
	fn input_type(&self) -> TypeId {
		TypeId::of::<Input::Static>()
	}
	fn input_type_name(&self) -> &'static str {
		core::any::type_name::<Input>()
	}
	fn output_type(&self) -> core::any::TypeId {
		TypeId::of::<<Self::Output as StaticTypeSized>::Static>()
	}
	fn output_type_name(&self) -> &'static str {
		core::any::type_name::<Self::Output>()
	}
	fn to_node_io(&self, inputs: Vec<Type>) -> NodeIOTypes {
		NodeIOTypes {
			call_argument: concrete!(<Input as StaticTypeSized>::Static),
			return_value: concrete!(<Self::Output as StaticTypeSized>::Static),
			inputs,
		}
	}
	fn to_async_node_io(&self, inputs: Vec<Type>) -> NodeIOTypes
	where
		<Self::Output as Future>::Output: StaticTypeSized,
		Self::Output: Future,
	{
		NodeIOTypes {
			call_argument: concrete!(<Input as StaticTypeSized>::Static),
			return_value: future!(<<Self::Output as Future>::Output as StaticTypeSized>::Static),
			inputs,
		}
	}
}

impl<'i, N: Node<'i, I>, I> NodeIO<'i, I> for N
where
	N::Output: 'i + StaticTypeSized,
	I: StaticTypeSized,
{
}

impl<'i, I: 'i, N: Node<'i, I> + ?Sized> Node<'i, I> for &'i N {
	type Output = N::Output;
	fn eval(&'i self, input: I) -> N::Output {
		(*self).eval(input)
	}
}
impl<'i, I: 'i, O: 'i, N: Node<'i, I, Output = O> + ?Sized> Node<'i, I> for Box<N> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}
impl<'i, I: 'i, O: 'i, N: Node<'i, I, Output = O> + ?Sized> Node<'i, I> for alloc::sync::Arc<N> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}

impl<'i, I, O: 'i> Node<'i, I> for Pin<Box<dyn Node<'i, I, Output = O> + 'i>> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}
impl<'i, I, O: 'i> Node<'i, I> for Pin<&'i (dyn NodeIO<'i, I, Output = O> + 'i)> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}

pub use crate::application_io::{SurfaceFrame, SurfaceId};
#[cfg(feature = "wasm")]
pub type WasmSurfaceHandle = application_io::SurfaceHandle<web_sys::HtmlCanvasElement>;
#[cfg(feature = "wasm")]
pub type WasmSurfaceHandleFrame = application_io::SurfaceHandleFrame<web_sys::HtmlCanvasElement>;

pub trait InputAccessorSource<'a, T>: InputAccessorSourceIdentifier + core::fmt::Debug {
	fn get_input(&'a self, index: usize) -> Option<&'a T>;
	fn set_input(&'a mut self, index: usize, value: T);
}

pub trait InputAccessorSourceIdentifier {
	fn has_identifier(&self, identifier: &str) -> bool;
}

pub trait InputAccessor<'n, Source: 'n>
where
	Self: Sized,
{
	fn new_with_source(source: &'n Source) -> Option<Self>;
}

pub trait NodeInputDecleration {
	const INDEX: usize;
	fn identifier() -> &'static str;
	type Result;
}
