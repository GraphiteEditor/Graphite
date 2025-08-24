mod navigation_message;
mod navigation_message_handler;
pub mod utility_types;

#[doc(inline)]
pub use navigation_message::{NavigationMessage, NavigationMessageDiscriminant};
#[doc(inline)]
pub use navigation_message_handler::{NavigationMessageContext, NavigationMessageHandler};
