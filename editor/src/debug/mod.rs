pub mod debug_message;

mod debug_message_handler;

#[doc(inline)]
pub use debug_message::{DebugMessage, DebugMessageDiscriminant};
#[doc(inline)]
pub use debug_message_handler::DebugMessageHandler;
