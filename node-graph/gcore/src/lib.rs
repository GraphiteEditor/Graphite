#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg_attr(feature = "log", macro_use)]
#[cfg(feature = "log")]
extern crate log;

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
pub mod storage;

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

pub mod quantization;

use core::any::TypeId;
pub use raster::Color;
pub use types::Cow;

// pub trait Node: for<'n> NodeIO<'n> {
/// The node trait allows for defining any node. Nodes can only take one input, however they can store references to other nodes inside the struct.
/// See `node-graph/README.md` for information on how to define a new node.
pub trait Node<'i, Input: 'i>: 'i {
	type Output: 'i;
	/// Evalutes the node with the single specified input.
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

pub trait NodeMut<'i, Input: 'i>: 'i {
	type MutOutput: 'i;
	fn eval_mut(&'i mut self, input: Input) -> Self::MutOutput;
}

pub trait NodeOnce<'i, Input>
where
	Input: 'i,
{
	type OnceOutput: 'i;
	fn eval_once(self, input: Input) -> Self::OnceOutput;
}

impl<'i, T: Node<'i, I>, I: 'i> NodeOnce<'i, I> for &'i T {
	type OnceOutput = T::Output;
	fn eval_once(self, input: I) -> Self::OnceOutput {
		(self).eval(input)
	}
}
impl<'i, T: Node<'i, I> + ?Sized, I: 'i> NodeMut<'i, I> for &'i T {
	type MutOutput = T::Output;
	fn eval_mut(&'i mut self, input: I) -> Self::MutOutput {
		(*self).eval(input)
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
	fn to_node_io(&self, parameters: Vec<Type>) -> NodeIOTypes {
		NodeIOTypes {
			input: concrete!(<Input as StaticTypeSized>::Static),
			output: concrete!(<Self::Output as StaticTypeSized>::Static),
			parameters,
		}
	}
}

impl<'i, N: Node<'i, I>, I> NodeIO<'i, I> for N
where
	N::Output: 'i + StaticTypeSized,
	I: 'i + StaticTypeSized,
{
}

impl<'i, 's: 'i, I: 'i, N: Node<'i, I> + ?Sized> Node<'i, I> for &'i N {
	type Output = N::Output;
	fn eval(&'i self, input: I) -> N::Output {
		(*self).eval(input)
	}
}
#[cfg(feature = "alloc")]
impl<'i, 's: 'i, I: 'i, O: 'i, N: Node<'i, I, Output = O> + ?Sized> Node<'i, I> for Box<N> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}
#[cfg(feature = "alloc")]
impl<'i, 's: 'i, I: 'i, O: 'i, N: Node<'i, I, Output = O> + ?Sized> Node<'i, I> for alloc::sync::Arc<N> {
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
pub use crate::application_io::{ExtractImageFrame, SurfaceFrame, SurfaceId};
#[cfg(feature = "wasm")]
pub type WasmSurfaceHandle = application_io::SurfaceHandle<web_sys::HtmlCanvasElement>;
#[cfg(feature = "wasm")]
pub type WasmSurfaceHandleFrame = application_io::SurfaceHandleFrame<web_sys::HtmlCanvasElement>;
