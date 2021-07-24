mod document_file;
mod document_message_handler;

#[doc(inline)]
pub use document_file::{Document, LayerData};

#[doc(inline)]
pub use document_message_handler::{AlignAggregate, AlignAxis, DocumentMessage, DocumentMessageDiscriminant, DocumentMessageHandler};
