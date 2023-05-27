#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg_attr(feature = "log", macro_use)]
#[cfg(feature = "log")]
extern crate log;

pub mod consts;
pub mod generic;
pub mod ops;
pub mod structural;
#[cfg(feature = "std")]
pub mod text;
#[cfg(feature = "std")]
pub mod uuid;
pub mod value;

#[cfg(feature = "gpu")]
pub mod gpu;

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

pub mod quantization;

use core::any::TypeId;
pub use raster::Color;

// pub trait Node: for<'n> NodeIO<'n> {
pub trait Node<'i, Input: 'i>: 'i {
	type Output: 'i;
	fn eval(&'i self, input: Input) -> Self::Output;
	fn reset(self: Pin<&mut Self>) {}
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
	fn node_name(&self) -> &'static str {
		core::any::type_name::<Self>()
	}

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

/*impl<'i, I: 'i, O: 'i> Node<'i, I> for &'i dyn for<'n> Node<'n, I, Output = O> {
	type Output = O;

	fn eval(&'i self, input: I) -> Self::Output {
		(**self).eval(input)
	}
}*/
impl<'i, 's: 'i, I: 'i, O: 'i, N: Node<'i, I, Output = O>> Node<'i, I> for &'s N {
	type Output = O;

	fn eval(&'i self, input: I) -> Self::Output {
		(**self).eval(input)
	}
}
impl<'i, I: 'i, O: 'i> Node<'i, I> for &'i dyn for<'a> Node<'a, I, Output = O> {
	type Output = O;

	fn eval(&'i self, input: I) -> Self::Output {
		(**self).eval(input)
	}
}
use core::pin::Pin;

use dyn_any::StaticTypeSized;
#[cfg(feature = "alloc")]
impl<'i, I: 'i, O: 'i> Node<'i, I> for Pin<Box<dyn for<'a> Node<'a, I, Output = O> + 'i>> {
	type Output = O;

	fn eval(&'i self, input: I) -> Self::Output {
		(**self).eval(input)
	}
}
impl<'i, I: 'i, O: 'i> Node<'i, I> for Pin<&'i (dyn NodeIO<'i, I, Output = O> + 'i)> {
	type Output = O;

	fn eval(&'i self, input: I) -> Self::Output {
		(**self).eval(input)
	}
}

#[cfg(feature = "alloc")]
pub use crate::raster::image::{EditorApi, ExtractImageFrame};
