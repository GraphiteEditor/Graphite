//! Handles modal dialogs that appear as floating menus in the center of the editor window.
//!
//! Dialogs are represented as structs that implement the `PropertyHolder` trait.
//!
//! To open a dialog, call the function `register_properties` on the dialog struct with `responses` and the `LayoutTarget::DialogDetails` enum variant.
//! Then dialog can be opened by sending the `FrontendMessage::DisplayDialog` message;

mod dialog_message;
mod dialog_message_handler;

pub mod export_dialog;
pub mod new_document_dialog;
pub mod simple_dialogs;

#[doc(inline)]
pub use dialog_message::{DialogMessage, DialogMessageDiscriminant};
#[doc(inline)]
pub use dialog_message_handler::DialogMessageHandler;
