#![no_std]
#![cfg_attr(target_arch = "spirv", feature(register_attr), register_attr(spirv))]

#[cfg(feature = "async")]
extern crate alloc;
#[cfg(feature = "async")]
use alloc::boxed::Box;
#[cfg(feature = "async")]
use async_trait::async_trait;

pub mod generic;
pub mod ops;
pub mod raster;
pub mod structural;
pub mod value;

pub trait Node<T> {
	type Output;

	fn eval(self, input: T) -> Self::Output;
}

trait Input<I> {
	unsafe fn input(&self, input: I);
}

#[cfg(feature = "async")]
#[async_trait]
pub trait AsyncNode<T> {
	type Output;

	async fn eval_async(self, input: T) -> Self::Output;
}

/*#[cfg(feature = "async")]
#[async_trait]
impl<'n, N: Node<T> + Send + Sync + 'n, T: Send + 'n> AsyncNode<T> for N {
	type Output = N::Output;

	async fn eval_async(self, input: T) -> Self::Output {
		Node::eval(self, input)
	}
}*/

pub trait Cache {
	fn clear(&mut self);
}

#[cfg(not(feature = "gpu"))]
extern crate alloc;
#[cfg(not(feature = "gpu"))]
impl<'n, I, O: 'n> Node<'n, I> for alloc::boxed::Box<dyn Node<'n, I, Output = O>> {
	type Output = O;

	fn eval(&'n self, input: &'n I) -> Self::Output {
		self.as_ref().eval(input)
	}
}
