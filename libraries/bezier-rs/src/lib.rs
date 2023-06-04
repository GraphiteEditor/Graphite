#![doc = include_str!("../README.md")]

pub(crate) mod compare;

mod bezier;
mod consts;
mod subpath;
mod utils;

pub use bezier::*;
pub use subpath::*;
pub use utils::{Cap, Join, SubpathTValue, TValue, TValueType};
