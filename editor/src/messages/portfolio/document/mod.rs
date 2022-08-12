mod document_message;
mod document_message_handler;

pub mod artboard;
pub mod navigation;
pub mod overlays;
pub mod properties_panel;
pub mod transform_layer;
pub mod utility_types;

#[doc(inline)]
pub use document_message::{DocumentMessage, DocumentMessageDiscriminant};
#[doc(inline)]
pub use document_message_handler::DocumentMessageHandler;
