pub mod value;
pub use graphene_core::{generic, ops /*, structural*/};

#[cfg(feature = "caching")]
pub mod caching;
#[cfg(feature = "memoization")]
pub mod memo;

pub use graphene_core::*;

use dyn_any::{downcast_ref, DynAny, StaticType};
pub type DynNode<'n, T> = &'n (dyn Node<'n, Output = T> + 'n);
pub type DynAnyNode<'n> = &'n (dyn Node<'n, Output = &'n dyn DynAny<'n>> + 'n);

pub trait DynamicInput<'n> {
    fn set_kwarg_by_name(&mut self, name: &str, value: DynAnyNode<'n>);
    fn set_arg_by_index(&mut self, index: usize, value: DynAnyNode<'n>);
}
