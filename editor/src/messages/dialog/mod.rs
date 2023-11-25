//! Handles dialogs that appear as floating menus in the center of the editor window.
//!
//! Dialogs are represented as structs that implement the `DialogLayoutHolder` trait.
//!
//! To open a dialog, call the function `send_dialog_to_frontend()` on the dialog struct.
//! Then dialog can be opened by sending the `FrontendMessage::DisplayDialog` message;

mod dialog_message;
mod dialog_message_handler;

pub mod export_dialog;
pub mod new_document_dialog;
pub mod preferences_dialog;
pub mod simple_dialogs;

#[doc(inline)]
pub use dialog_message::{DialogMessage, DialogMessageDiscriminant};
#[doc(inline)]
pub use dialog_message_handler::*;
