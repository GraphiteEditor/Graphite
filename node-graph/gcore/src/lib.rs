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
pub mod uuid;
pub mod value;

#[cfg(feature = "gpu")]
pub mod gpu;

pub mod raster;

#[cfg(feature = "alloc")]
pub mod vector;

pub mod quantization;

use core::any::TypeId;
pub use raster::Color;

// pub trait Node: for<'n> NodeIO<'n> {
pub trait Node<'i, Input: 'i>: 'i {
	type Output: 'i;
	fn eval<'s: 'i>(&'s self, input: Input) -> Self::Output;
}

#[cfg(feature = "alloc")]
mod types;
#[cfg(feature = "alloc")]
pub use types::*;

pub trait NodeIO<'i, Input: 'i>: 'i + Node<'i, Input>
where
	Self::Output: 'i + StaticType,
	Input: 'i + StaticType,
{
	fn input_type(&self) -> TypeId {
		TypeId::of::<Input::Static>()
	}
	fn input_type_name(&self) -> &'static str {
		core::any::type_name::<Input>()
	}
	fn output_type(&self) -> core::any::TypeId {
		TypeId::of::<<Self::Output as StaticType>::Static>()
	}
	fn output_type_name(&self) -> &'static str {
		core::any::type_name::<Self::Output>()
	}
	#[cfg(feature = "alloc")]
	fn to_node_io(&self, parameters: Vec<(Type, Type)>) -> NodeIOTypes {
		NodeIOTypes {
			input: concrete!(<Input as StaticType>::Static),
			output: concrete!(<Self::Output as StaticType>::Static),
			parameters,
		}
	}
}

impl<'i, N: Node<'i, I>, I> NodeIO<'i, I> for N
where
	N::Output: 'i + StaticType,
	I: 'i + StaticType,
{
}

/*impl<'i, I: 'i, O: 'i> Node<'i, I> for &'i dyn for<'n> Node<'n, I, Output = O> {
	type Output = O;

	fn eval<'s: 'i>(&'s self, input: I) -> Self::Output {
		(**self).eval(input)
	}
}*/
impl<'i, 'n: 'i, I: 'i, O: 'i> Node<'i, I> for &'n dyn for<'a> Node<'a, I, Output = O> {
	type Output = O;

	fn eval<'s: 'i>(&'s self, input: I) -> Self::Output {
		(**self).eval(input)
	}
}
use core::pin::Pin;

use dyn_any::StaticType;
#[cfg(feature = "alloc")]
impl<'i, I: 'i, O: 'i> Node<'i, I> for Pin<Box<dyn for<'a> Node<'a, I, Output = O> + 'i>> {
	type Output = O;

	fn eval<'s: 'i>(&'s self, input: I) -> Self::Output {
		(**self).eval(input)
	}
}
