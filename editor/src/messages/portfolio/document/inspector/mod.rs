mod inspector_message;
pub mod inspector_message_handler;

#[doc(inline)]
pub use inspector_message::{InspectorMessage, InspectorMessageDiscriminant};
#[doc(inline)]
pub use inspector_message_handler::InspectorMessageHandler;
