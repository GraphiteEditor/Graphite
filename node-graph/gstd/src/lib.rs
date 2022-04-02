#![feature(generic_associated_types)]

#[cfg(feature = "caching")]
pub mod caching;
pub mod generic;
#[cfg(feature = "memoization")]
pub mod memo;
pub mod ops;
pub mod structural;
pub mod value;
