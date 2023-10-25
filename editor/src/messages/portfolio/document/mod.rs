mod document_message;
mod document_message_handler;

pub mod navigation;
pub mod node_graph;
pub mod overlays;
pub mod properties_panel;
pub mod utility_types;

#[doc(inline)]
pub use document_message::{DocumentMessage, DocumentMessageDiscriminant};
#[doc(inline)]
pub use document_message_handler::{DocumentInputs, DocumentMessageHandler};
