//! Container backend implementations.

pub mod memory;

#[cfg(not(target_family = "wasm"))]
pub mod folder;

#[cfg(target_family = "wasm")]
pub mod opfs;
