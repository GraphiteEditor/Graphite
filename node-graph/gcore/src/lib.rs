#![no_std]

pub mod generic;
pub mod ops;
pub mod structural;
pub mod value;

#[rustfmt::skip]
pub trait Node< 'n, Input> {
    type Output : 'n;

    fn eval(&'n self, input: &'n Input) -> Self::Output;
}

pub trait Exec<'n>: Node<'n, ()> {
    fn exec(&'n self) -> Self::Output {
        self.eval(&())
    }
}
impl<'n, T: Node<'n, ()>> Exec<'n> for T {}

pub trait Cache {
    fn clear(&mut self);
}
