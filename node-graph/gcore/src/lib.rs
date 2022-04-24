#![no_std]
#![cfg_attr(target_arch = "spirv", feature(register_attr), register_attr(spirv))]

pub mod generic;
pub mod ops;
pub mod structural;
pub mod value;

#[rustfmt::skip]
pub trait Node< 'n, Input> {
    type Output : 'n;

    fn eval(&'n self, input: Input) -> Self::Output;
}

// TODO: Fix exec trait
pub trait Exec<'n>: Node<'n, ()> {
    fn exec(&'n self) -> Self::Output {
        self.eval(())
    }
}
impl<'n, T: Node<'n, ()>> Exec<'n> for T {}

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
