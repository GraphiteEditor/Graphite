use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, DialogMessage, PreferencesDialog)]
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PreferencesDialogMessage {
	Confirm,
}
