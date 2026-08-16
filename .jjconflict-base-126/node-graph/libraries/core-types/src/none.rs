use dyn_any::DynAny;
use graphene_hash::CacheHash;

/// An artist's declaration that there is no content here, distinct from the `()` type's "nothing was wired".
/// Visually represented as a red slash over a white background. Akin to the CSS `none` keyword.
///
/// Because its name matches the Rust prelude's `Option::None` variant, we always reference this as `none::None`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, CacheHash, DynAny)]
pub struct None;
