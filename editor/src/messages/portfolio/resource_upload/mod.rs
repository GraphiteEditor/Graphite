mod resource_upload_message;
mod resource_upload_message_handler;

pub mod utility_types;

#[doc(inline)]
pub use resource_upload_message::{ResourceUploadMessage, ResourceUploadMessageDiscriminant};
#[doc(inline)]
pub use resource_upload_message_handler::{ResourceUploadMessageContext, ResourceUploadMessageHandler};
