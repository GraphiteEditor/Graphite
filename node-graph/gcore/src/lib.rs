pub mod generic;
pub mod ops;
pub mod structural;
pub mod value;

use std::any::Any;

#[rustfmt::skip]
pub trait Node< 'n, Input> {
    type Output : 'n;

    fn eval(&'n self, input: &'n Input) -> Self::Output;
}

pub trait Exec<'n> {
    type Output: 'n;
    fn exec(&'n self) -> Self::Output;
}
impl<'n, T: Exec<'n>> Node<'n, ()> for T {
    type Output = <Self as Exec<'n>>::Output;
    fn eval(&'n self, _input: &()) -> Self::Output {
        self.exec()
    }
}

pub trait DynamicInput<'n> {
    fn set_kwarg_by_name(
        &mut self,
        name: &str,
        value: &'n dyn Node<'n, (), Output = &'n (dyn Any + 'static)>,
    );
    fn set_arg_by_index(
        &mut self,
        index: usize,
        value: &'n dyn Node<'n, (), Output = &'n (dyn Any + 'static)>,
    );
}

pub trait AnyRef<'n, I>: Node<'n, I> {
    fn any(&'n self, input: &'n dyn Any) -> Self::Output
    where
        I: 'static + Copy;
}

impl<'n, T: Node<'n, I>, I> AnyRef<'n, I> for T {
    fn any(&'n self, input: &'n dyn Any) -> Self::Output
    where
        I: 'static + Copy,
    {
        self.eval(input.downcast_ref::<I>().unwrap_or_else(|| {
            panic!(
                "Node was evaluated with wrong input. The input has to be of type: {}",
                std::any::type_name::<I>(),
            )
        }))
    }
}
