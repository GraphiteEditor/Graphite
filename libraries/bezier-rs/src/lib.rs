//! Bezier-rs: A Bezier Math Library for Rust
pub(crate) mod compare;

mod bezier;
mod consts;
mod subpath;
mod utils;

pub use bezier::*;
pub use subpath::*;
pub use utils::{Joint, SubpathTValue, TValue};
