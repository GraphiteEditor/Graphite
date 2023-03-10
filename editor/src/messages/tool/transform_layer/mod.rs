//! Handles Blender inspired layer transformation with the <kbd>G</kbd> <kbd>R</kbd> and <kbd>S</kbd> keys for grabbing, rotating and scaling.
//!
//! Other features include
//! - Typing a number for a precise transformation
//! - <kbd>Shift</kbd> to slow transformation
//! - <kbd>Ctrl</kbd> to snap angles to 15°
//! - Escape or right click to cancel

mod transform_layer_message;
mod transform_layer_message_handler;

#[doc(inline)]
pub use transform_layer_message::{TransformLayerMessage, TransformLayerMessageDiscriminant};
#[doc(inline)]
pub use transform_layer_message_handler::TransformLayerMessageHandler;
