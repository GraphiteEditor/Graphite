mod resource_message;
mod resource_message_handler;
pub mod utility_types;

#[doc(inline)]
pub use resource_message::{ResourceMessage, ResourceMessageDiscriminant};
#[doc(inline)]
pub use resource_message_handler::{ResourceMessageContext, ResourceMessageHandler};
