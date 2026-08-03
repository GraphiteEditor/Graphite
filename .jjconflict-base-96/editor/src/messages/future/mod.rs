mod future_message;
mod future_message_handler;

#[doc(inline)]
pub use future_message::{FutureMessage, FutureMessageDiscriminant};
#[doc(inline)]
pub use future_message_handler::{FutureMessageContext, FutureMessageHandler, MessageFuture, MessageSpawner, Wake};
