pub mod value;
pub use graphene_core::{generic, ops, structural};

#[cfg(feature = "caching")]
pub mod caching;
#[cfg(feature = "memoization")]
pub mod memo;

pub use graphene_core::*;

use dyn_any::{downcast_ref, DynAny, StaticType};
pub type DynNode<'n, T> = &'n (dyn Node<'n, (), Output = T> + 'n);
pub type DynAnyNode<'n> = &'n (dyn Node<'n, (), Output = &'n dyn DynAny<'n>> + 'n);

pub trait DynamicInput<'n> {
    fn set_kwarg_by_name(&mut self, name: &str, value: DynAnyNode<'n>);
    fn set_arg_by_index(&mut self, index: usize, value: DynAnyNode<'n>);
}

pub trait AnyRef<'n, I: 'n + StaticType>: Node<'n, &'n I> {
    fn any(&'n self, input: &'n dyn DynAny<'n>) -> Self::Output;
}

impl<'n, N: Node<'n, &'n I>, I: StaticType + 'n> AnyRef<'n, I> for N {
    fn any(&'n self, input: &'n dyn DynAny<'n>) -> Self::Output {
        self.eval(downcast_ref::<I>(input).unwrap_or_else(|| {
            panic!(
                "Node was evaluated with wrong input. The input has to be of type: {}",
                std::any::type_name::<I>(),
            )
        }))
    }
}
