#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg_attr(feature = "log", macro_use)]
#[cfg(feature = "log")]
extern crate log;

pub mod generic;
pub mod ops;
pub mod structural;
pub mod value;

#[cfg(feature = "gpu")]
pub mod gpu;

//pub mod raster;

#[cfg(feature = "alloc")]
//pub mod vector;

pub trait NodeIO<'a, _WhereSelfUsableWithinA = &'a Self>: 'a {
	type Input;
	type Output;
}

//pub trait Node: for<'n> NodeIO<'n> {
pub trait Node<'i, 's: 'i>: NodeIO<'i> {
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output;
}
