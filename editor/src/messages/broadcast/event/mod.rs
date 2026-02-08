mod event_message;
mod event_message_handler;

#[doc(inline)]
pub use event_message::{EventMessage, EventMessageDiscriminant};
#[doc(inline)]
pub use event_message_handler::{EventMessageContext, EventMessageHandler};
