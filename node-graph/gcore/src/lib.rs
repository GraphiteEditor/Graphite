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
//pub mod structural;
pub mod value;

pub trait Node<'n> {
    type Output: 'n; // TODO: replace with generic associated type

    fn eval(&'n self) -> Self::Output;
}

impl<'n, N: Node<'n>> Node<'n> for &'n N {
    type Output = N::Output;

    fn eval(&'n self) -> Self::Output {
        Node::eval(*self)
    }
}

#[cfg(feature = "async")]
#[async_trait]
pub trait AsyncNode<'n> {
    type Output: 'n; // TODO: replace with generic associated type

    async fn eval(&'n self) -> Self::Output;
}

#[cfg(feature = "async")]
#[async_trait]
impl<'n, N: Node<'n> + Sync> AsyncNode<'n> for N {
    type Output = N::Output;

    async fn eval(&'n self) -> Self::Output {
        Node::eval(self)
    }
}

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
