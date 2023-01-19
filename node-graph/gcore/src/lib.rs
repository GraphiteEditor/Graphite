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
//pub mod ops;
pub mod structural;
pub mod value;

#[cfg(feature = "gpu")]
pub mod gpu;

//pub mod raster;

#[cfg(feature = "alloc")]
//pub mod vector;

pub trait NodeIO<'a, _WhereSelfUsableWithinA = &'a Self> {
	type Input;
	type Output;
}

pub trait Node: for<'n> NodeIO<'n> {
	fn eval<'i, 's: 'i>(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output;
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

#[cfg(feature = "async")]
impl<'n, N> NodeIO<'n> for Box<N>
where
	N: NodeIO<'n>,
{
	type Input = <N as NodeIO<'n>>::Input;
	type Output = <N as NodeIO<'n>>::Output;
}
#[cfg(feature = "async")]
impl<'n: 'i, 'i, N> NodeIO<'i> for &'n Box<N>
where
	for<'a> &'a N: NodeIO<'a>,
{
	type Input = <&'i N as NodeIO<'i>>::Input;
	type Output = <&'i N as NodeIO<'i>>::Output;
}
