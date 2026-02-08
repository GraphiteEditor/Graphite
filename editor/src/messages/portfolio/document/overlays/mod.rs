pub mod grid_overlays;
mod overlays_message;
mod overlays_message_handler;
pub mod utility_functions;
// Native (non‑wasm)
#[cfg(not(target_family = "wasm"))]
pub mod utility_types_native;
#[cfg(not(target_family = "wasm"))]
pub use utility_types_native as utility_types;

// WebAssembly
#[cfg(target_family = "wasm")]
pub mod utility_types_web;
#[cfg(target_family = "wasm")]
pub use utility_types_web as utility_types;

#[doc(inline)]
pub use overlays_message::{OverlaysMessage, OverlaysMessageDiscriminant};
#[doc(inline)]
pub use overlays_message_handler::{OverlaysMessageContext, OverlaysMessageHandler};
