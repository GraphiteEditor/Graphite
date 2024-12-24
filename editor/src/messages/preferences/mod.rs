mod preference_type;
mod preferences_message;
mod preferences_message_handler;

#[doc(inline)]
pub use preference_type::SelectionMode;
#[doc(inline)]
pub use preferences_message::{PreferencesMessage, PreferencesMessageDiscriminant};
#[doc(inline)]
pub use preferences_message_handler::PreferencesMessageHandler;
