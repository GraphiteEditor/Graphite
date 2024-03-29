use crate::messages::prelude::*;

#[impl_message(Message, DialogMessage, PreferencesDialog)]
#[derive(Eq, PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PreferencesDialogMessage {
	Confirm,
}
