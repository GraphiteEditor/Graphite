mod resource_storage_message;
mod resource_storage_message_handler;

#[doc(inline)]
pub use resource_storage_message::{ResourceStorageMessage, ResourceStorageMessageDiscriminant};
#[doc(inline)]
pub use resource_storage_message_handler::{ResourceStorageMessageContext, ResourceStorageMessageHandler, ResourcesHandle};
