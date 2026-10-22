mod failed_documents_message;
mod failed_documents_message_handler;

#[doc(inline)]
pub use failed_documents_message::{FailedDocumentsMessage, FailedDocumentsMessageDiscriminant};
#[doc(inline)]
pub use failed_documents_message_handler::{FailedDocumentsMessageContext, FailedDocumentsMessageHandler};
