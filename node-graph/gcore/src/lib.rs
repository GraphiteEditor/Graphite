#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use core::future::Future;

#[cfg_attr(feature = "log", macro_use)]
#[cfg(feature = "log")]
extern crate log;
pub use crate as graphene_core;

#[cfg(feature = "reflections")]
pub use ctor;

pub mod consts;
pub mod generic;
pub mod logic;
pub mod ops;
pub mod structural;
#[cfg(feature = "std")]
pub mod text;
#[cfg(feature = "std")]
pub mod uuid;
pub mod value;

#[cfg(feature = "gpu")]
pub mod gpu;

#[cfg(feature = "alloc")]
pub mod memo;

pub mod raster;
#[cfg(feature = "alloc")]
pub mod transform;

#[cfg(feature = "alloc")]
mod graphic_element;
#[cfg(feature = "alloc")]
pub use graphic_element::*;
#[cfg(feature = "alloc")]
pub mod vector;

#[cfg(feature = "alloc")]
pub mod application_io;

#[cfg(feature = "reflections")]
pub mod registry;

use core::any::TypeId;
pub use memo::MemoHash;
pub use raster::Color;
pub use types::Cow;

// pub trait Node: for<'n> NodeIO<'n> {
/// The node trait allows for defining any node. Nodes can only take one input, however they can store references to other nodes inside the struct.
/// See `node-graph/README.md` for information on how to define a new node.
pub trait Node<'i, Input: 'i>: 'i {
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
	#[cfg(feature = "std")]
	fn serialize(&self) -> Option<std::sync::Arc<dyn core::any::Any>> {
		log::warn!("Node::serialize not implemented for {}", core::any::type_name::<Self>());
		None
	}
}

#[cfg(feature = "alloc")]
mod types;
#[cfg(feature = "alloc")]
pub use types::*;

pub trait NodeIO<'i, Input: 'i>: 'i + Node<'i, Input>
where
	Self::Output: 'i + StaticTypeSized,
	Input: 'i + StaticTypeSized,
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
	#[cfg(feature = "alloc")]
	fn to_node_io(&self, inputs: Vec<Type>) -> NodeIOTypes {
		NodeIOTypes {
			call_argument: concrete!(<Input as StaticTypeSized>::Static),
			return_value: concrete!(<Self::Output as StaticTypeSized>::Static),
			inputs,
		}
	}
	#[cfg(feature = "alloc")]
	fn to_async_node_io(&self, inputs: Vec<Type>) -> NodeIOTypes
	where
		<Self::Output as Future>::Output: StaticTypeSized,
		Self::Output: Future,
	{
		NodeIOTypes {
			call_argument: concrete!(<Input as StaticTypeSized>::Static),
			// TODO return actual future type
			return_value: concrete!(<<Self::Output as Future>::Output as StaticTypeSized>::Static),
			inputs,
		}
	}
}

impl<'i, N: Node<'i, I>, I> NodeIO<'i, I> for N
where
	N::Output: 'i + StaticTypeSized,
	I: 'i + StaticTypeSized,
{
}

impl<'i, I: 'i, N: Node<'i, I> + ?Sized> Node<'i, I> for &'i N {
	type Output = N::Output;
	fn eval(&'i self, input: I) -> N::Output {
		(*self).eval(input)
	}
}
#[cfg(feature = "alloc")]
impl<'i, I: 'i, O: 'i, N: Node<'i, I, Output = O> + ?Sized> Node<'i, I> for Box<N> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}
#[cfg(feature = "alloc")]
impl<'i, I: 'i, O: 'i, N: Node<'i, I, Output = O> + ?Sized> Node<'i, I> for alloc::sync::Arc<N> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}

use dyn_any::StaticTypeSized;

use core::pin::Pin;

#[cfg(feature = "alloc")]
impl<'i, I: 'i, O: 'i> Node<'i, I> for Pin<Box<dyn Node<'i, I, Output = O> + 'i>> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}
impl<'i, I: 'i, O: 'i> Node<'i, I> for Pin<&'i (dyn NodeIO<'i, I, Output = O> + 'i)> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}

#[cfg(feature = "alloc")]
pub use crate::application_io::{SurfaceFrame, SurfaceId};
#[cfg(feature = "wasm")]
pub type WasmSurfaceHandle = application_io::SurfaceHandle<web_sys::HtmlCanvasElement>;
#[cfg(feature = "wasm")]
pub type WasmSurfaceHandleFrame = application_io::SurfaceHandleFrame<web_sys::HtmlCanvasElement>;

pub use dyn_any::{WasmNotSend, WasmNotSync};
