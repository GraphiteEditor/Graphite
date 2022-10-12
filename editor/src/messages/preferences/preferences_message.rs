use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, Preferences)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PreferencesMessage {
	AiArtistServerHostname { hostname: String },
}
