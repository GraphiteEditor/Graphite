//! Handles dialogs/modals/popups that appear as boxes in the centre of the editor.
//!
//! Dialogs are represented as structs that implement the [`crate::layout::widgets::PropertyHolder`] trait.
//!
//! To open a dialog, call the function `register_properties` on the dialog struct with `responses` and the `LayoutTarget::DialogDetails`
//! and then you can open the dialog with [`crate::message_prelude::FrontendMessage::DisplayDialog`]

mod dialog_message;
mod dialog_message_handler;
mod dialogs;

pub mod messages {
	pub use super::dialog_message::{DialogMessage, DialogMessageDiscriminant};
	pub use super::dialog_message_handler::DialogMessageHandler;
}

pub use dialogs::*;
