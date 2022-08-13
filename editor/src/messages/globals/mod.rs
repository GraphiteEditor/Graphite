mod globals_message;
mod globals_message_handler;

pub mod global_variables;

#[doc(inline)]
pub use globals_message::{GlobalsMessage, GlobalsMessageDiscriminant};
#[doc(inline)]
pub use globals_message_handler::GlobalsMessageHandler;
