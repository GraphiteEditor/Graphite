#![no_std]
#![cfg_attr(target_arch = "spirv", feature(register_attr), register_attr(spirv))]

pub mod generic;
pub mod ops;
//pub mod structural;
pub mod value;

#[rustfmt::skip]
pub trait Node<'n> {
    type Output: 'n; // TODO: replace with generic associated type

    fn eval(&'n self) -> Self::Output;
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
