mod resource_message;
mod resource_message_handler;

#[doc(inline)]
pub use resource_message::{ResourceMessage, ResourceMessageDiscriminant};
#[doc(inline)]
pub use resource_message_handler::{ResourceMessageContext, ResourceMessageHandler, ResourcesHandle};
