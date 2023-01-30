//! Bezier-rs: A Bezier Math Library for Rust
#[cfg(test)]
pub(crate) mod compare;

mod bezier;
mod consts;
mod subpath;
mod utils;

pub use bezier::*;
pub use subpath::*;
pub use utils::{SubpathTValue, TValue};
