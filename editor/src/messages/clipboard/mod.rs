mod clipboard_message;
pub mod clipboard_message_handler;

#[doc(inline)]
pub use clipboard_message::{ClipboardMessage, ClipboardMessageDiscriminant};
#[doc(inline)]
pub use clipboard_message_handler::{ClipboardMessageContext, ClipboardMessageHandler};
