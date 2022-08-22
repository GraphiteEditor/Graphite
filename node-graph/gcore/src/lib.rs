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
	fn input(&self) -> &str {
		core::any::type_name::<T>()
	}
	fn output(&self) -> &str {
		core::any::type_name::<Self::Output>()
	}
}

trait Input<I> {
	unsafe fn input(&self, input: I);
}

pub trait RefNode<T> {
	type Output;

	fn eval_ref(&self, input: T) -> Self::Output;
}

impl<'n, N: 'n, I> RefNode<I> for &'n N
where
	&'n N: Node<I>,
	Self: 'n,
{
	type Output = <&'n N as Node<I>>::Output;
	fn eval_ref(&self, input: I) -> Self::Output {
		self.eval(input)
	}
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

#[cfg(feature = "async")]
impl<N, I> Node<I> for Box<N>
where
	N: Node<I>,
{
	type Output = <N as Node<I>>::Output;
	fn eval(self, input: I) -> Self::Output {
		(*self).eval(input)
	}
}
#[cfg(feature = "async")]
impl<'n, N, I> Node<I> for &'n Box<N>
where
	&'n N: Node<I>,
{
	type Output = <&'n N as Node<I>>::Output;
	fn eval(self, input: I) -> Self::Output {
		self.as_ref().eval(input)
	}
}
