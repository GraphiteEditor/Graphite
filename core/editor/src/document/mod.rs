mod document_file;
mod document_message_handler;

#[doc(inline)]
pub use document_file::Document;

#[doc(inline)]
pub use document_message_handler::{DocumentActionHandler, DocumentMessage, DocumentMessageDiscriminant};
