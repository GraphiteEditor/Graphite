///! A collection of utilities for working with the HTML canvases.
///! This library is designed to be used in a WebAssembly context.
///! It doesn't expose any functionality when compiled for non-WebAssembly targets

#[cfg(target_family = "wasm")]
mod wasm;
#[cfg(target_family = "wasm")]
pub use wasm::*;
