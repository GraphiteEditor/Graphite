pub mod generic;
pub mod ops;
pub mod structural;
pub mod value;

use dyn_any::{downcast_ref, DynAny, StaticType};
use std::any::Any;

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

pub type DynNode<'n, T> = &'n (dyn Node<'n, (), Output = T> + 'n);
pub type DynAnyNode<'n> = &'n (dyn Node<'n, (), Output = &'n dyn DynAny<'n>> + 'n);

pub trait DynamicInput<'n> {
    fn set_kwarg_by_name(&mut self, name: &str, value: DynAnyNode<'n>);
    fn set_arg_by_index(&mut self, index: usize, value: DynAnyNode<'n>);
}

pub trait AnyRef<'n, I: StaticType<'n>>: Node<'n, I> {
    fn any(&'n self, input: &'n dyn DynAny<'n>) -> Self::Output;
}

impl<'n, T: Node<'n, I>, I: StaticType<'n>> AnyRef<'n, I> for T {
    fn any(&'n self, input: &'n dyn DynAny<'n>) -> Self::Output {
        self.eval(downcast_ref::<I>(input).unwrap_or_else(|| {
            panic!(
                "Node was evaluated with wrong input. The input has to be of type: {}",
                std::any::type_name::<I>(),
            )
        }))
    }
}
