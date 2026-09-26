mod sync_message;
mod sync_message_handler;

#[doc(inline)]
pub use sync_message::{SyncMessage, SyncMessageDiscriminant};
#[doc(inline)]
pub use sync_message_handler::{PRESENCE_OVERLAY_PROVIDER, SyncMessageContext, SyncMessageHandler};
