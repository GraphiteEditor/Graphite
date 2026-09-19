mod ingest_message;
mod ingest_message_handler;

pub mod utility_types;

#[doc(inline)]
pub use ingest_message::{IngestMessage, IngestMessageDiscriminant};
#[doc(inline)]
pub use ingest_message_handler::{IngestMessageContext, IngestMessageHandler};
