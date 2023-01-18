#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg_attr(feature = "log", macro_use)]
#[cfg(feature = "log")]
extern crate log;

#[cfg(feature = "async")]
use alloc::boxed::Box;
#[cfg(feature = "async")]
use async_trait::async_trait;

pub mod generic;
pub mod ops;
pub mod structural;
pub mod value;

#[cfg(feature = "gpu")]
pub mod gpu;

pub mod raster;

#[cfg(feature = "alloc")]
pub mod vector;

pub trait Node<T> {
	type Output;

	fn eval(self, input: T) -> Self::Output;
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

pub trait AsRefNode<'n, T>
where
	&'n Self: Node<T>,
	Self: 'n,
{
	type Output;
	fn eval_box(&'n self, input: T) -> <Self>::Output;
}

impl<'n, N: 'n, I> AsRefNode<'n, I> for N
where
	&'n N: Node<I>,
	N: Node<I>,
	Self: 'n,
{
	type Output = <&'n N as Node<I>>::Output;
	fn eval_box(&'n self, input: I) -> <Self>::Output {
		self.eval(input)
	}
}

impl<'n, T> Node<T> for &'n (dyn AsRefNode<'n, T, Output = T> + 'n) {
	type Output = T;
	fn eval(self, input: T) -> Self::Output {
		self.eval_box(input)
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
