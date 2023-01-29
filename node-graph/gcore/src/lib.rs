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

pub mod raster;

#[cfg(feature = "alloc")]
pub mod vector;

// pub trait Node: for<'n> NodeIO<'n> {
pub trait Node<'i, Input: 'i> {
	type Output: 'i;
	fn eval<'s: 'i>(&'s self, input: Input) -> Self::Output;
}

/*impl<'i, I: 'i, O: 'i> Node<'i, I> for &'i dyn for<'n> Node<'n, I, Output = O> {
	type Output = O;

	fn eval<'s: 'i>(&'s self, input: I) -> Self::Output {
		(**self).eval(input)
	}
}*/
impl<'i, I: 'i, O: 'i> Node<'i, I> for &dyn Node<'i, I, Output = O> {
	type Output = O;

	fn eval<'s: 'i>(&'s self, input: I) -> Self::Output {
		(**self).eval(input)
	}
}
